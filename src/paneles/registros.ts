// Vista de registros (RF-05): x0–x31 + pc con nombres ABI, valores en
// hex y decimal, y resaltado de los registros modificados desde la
// última actualización. Doble click en un valor lo edita en vivo (RP7).
import { api, type MachineState } from "../api";
import { t } from "../i18n";

const ABI: string[] = [
  "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1",
  "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7",
  "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
  "t3", "t4", "t5", "t6",
];

const hex = (v: number) => `0x${(v >>> 0).toString(16).padStart(8, "0")}`;
/** Valor decimal con signo (los registros se muestran como i32). */
const dec = (v: number) => String(v | 0);

/** Acepta decimal (42, -7) o hexadecimal (0x2a, -0x10); null si no parsea
 *  o se sale del rango de 32 bits. */
function parsearValor(texto: string): number | null {
  const v = texto.trim();
  let n: number;
  if (/^-?\d+$/.test(v)) n = Number.parseInt(v, 10);
  else if (/^-?0x[0-9a-f]+$/i.test(v)) n = Number.parseInt(v, 16);
  else return null;
  if (!Number.isSafeInteger(n) || n < -0x8000_0000 || n > 0xffff_ffff) return null;
  return n;
}

export class Registros {
  private celdas: { hex: HTMLElement; dec: HTMLElement; fila: HTMLElement }[] = [];
  private pcHex: HTMLElement;
  private pcDec: HTMLElement;
  private anterior: number[] | null = null;
  private pcAnterior: number | null = null;
  private errorEl: HTMLElement;

  constructor(root: HTMLElement) {
    const tabla = document.createElement("table");
    tabla.className = "registros-tabla";
    tabla.title = t("registros.editarHint");
    tabla.innerHTML = `<thead><tr>
        <th>${t("registros.nombre")}</th>
        <th>${t("registros.hex")}</th>
        <th>${t("registros.dec")}</th>
      </tr></thead>`;
    const cuerpo = document.createElement("tbody");

    const filaPc = document.createElement("tr");
    filaPc.className = "registros-pc";
    filaPc.innerHTML = `<td>pc</td><td class="mono"></td><td class="mono"></td>`;
    this.pcHex = filaPc.children[1] as HTMLElement;
    this.pcDec = filaPc.children[2] as HTMLElement;
    this.hacerEditable(this.pcHex, "pc");
    this.hacerEditable(this.pcDec, "pc");
    cuerpo.appendChild(filaPc);

    for (let i = 0; i < 32; i++) {
      const tr = document.createElement("tr");
      tr.innerHTML = `<td>${ABI[i]} <span class="registros-xn">x${i}</span></td>
        <td class="mono"></td><td class="mono"></td>`;
      cuerpo.appendChild(tr);
      const hexEl = tr.children[1] as HTMLElement;
      const decEl = tr.children[2] as HTMLElement;
      // x0 también se cablea: el rechazo didáctico del backend lo explica.
      this.hacerEditable(hexEl, ABI[i]);
      this.hacerEditable(decEl, ABI[i]);
      this.celdas.push({ fila: tr, hex: hexEl, dec: decEl });
    }
    tabla.appendChild(cuerpo);
    root.appendChild(tabla);
    this.errorEl = document.createElement("div");
    this.errorEl.className = "panel-error";
    this.errorEl.hidden = true;
    root.appendChild(this.errorEl);
    this.limpiar();
  }

  /** Refresca valores y marca los registros que cambiaron (RF-05).
   *  Una celda con edición inline en curso no se pisa (el refresco
   *  periódico durante una ejecución larga destruiría el input). */
  actualizar(estado: MachineState): void {
    const poner = (el: HTMLElement, texto: string) => {
      if (!el.querySelector("input")) el.textContent = texto;
    };
    const regs = estado.registers;
    for (let i = 0; i < 32; i++) {
      const c = this.celdas[i];
      poner(c.hex, hex(regs[i]));
      poner(c.dec, dec(regs[i]));
      const cambio = this.anterior !== null && this.anterior[i] !== regs[i];
      c.fila.classList.toggle("cambiado", cambio);
    }
    poner(this.pcHex, hex(estado.pc));
    poner(this.pcDec, dec(estado.pc));
    this.pcHex.parentElement!.classList.toggle(
      "cambiado",
      this.pcAnterior !== null && this.pcAnterior !== estado.pc,
    );
    this.anterior = [...regs];
    this.pcAnterior = estado.pc;
  }

  /** Estado inicial sin resaltados (tras ensamblar/reiniciar). */
  reiniciar(estado: MachineState): void {
    this.anterior = null;
    this.pcAnterior = null;
    this.actualizar(estado);
  }

  private limpiar(): void {
    this.pcHex.textContent = "—";
    this.pcDec.textContent = "—";
    for (const c of this.celdas) {
      c.hex.textContent = "—";
      c.dec.textContent = "—";
    }
  }

  // ----- edición en vivo (RP7) -----

  private hacerEditable(celda: HTMLElement, reg: string): void {
    celda.addEventListener("dblclick", () => this.editar(celda, reg));
  }

  /** Reemplaza la celda por un input inline prellenado: Enter aplica,
   *  Esc o blur cancelan. El valor mostrado solo cambia vía actualizar()
   *  tras un write_reg aceptado. */
  private editar(celda: HTMLElement, reg: string): void {
    if (celda.querySelector("input")) return;
    const original = celda.textContent ?? "";
    if (original === "—") return; // sin máquina todavía: nada que editar
    const input = document.createElement("input");
    input.type = "text";
    input.className = "edicion-inline";
    input.value = original;
    input.spellcheck = false;
    celda.textContent = "";
    celda.appendChild(input);
    input.focus();
    input.select();
    let cerrado = false;
    const cerrar = () => {
      if (cerrado) return;
      cerrado = true;
      celda.textContent = original;
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        cerrar();
        return;
      }
      if (e.key !== "Enter") return;
      const val = parsearValor(input.value);
      if (val === null) {
        // Valor ilegible: se avisa y el input queda para corregirlo.
        this.mostrarError(t("edicion.valorInvalido"));
        return;
      }
      void this.aplicar(reg, val, cerrar);
    });
    input.addEventListener("blur", () => cerrar());
  }

  private async aplicar(reg: string, val: number, cerrar: () => void): Promise<void> {
    try {
      await api.writeReg(reg, val);
      cerrar();
      this.errorEl.hidden = true;
      // Refresco por el flujo normal: el resaltado de cambios (RF-05)
      // marca el registro recién editado.
      this.actualizar(await api.getState());
    } catch (e) {
      cerrar(); // rechazo (corriendo, x0, …): el valor no cambia
      this.mostrarError(String(e));
    }
  }

  private mostrarError(mensaje: string): void {
    this.errorEl.textContent = mensaje;
    this.errorEl.hidden = false;
  }
}
