// Vista de registros (RF-05): x0–x31 + pc con nombres ABI, valores en
// hex y decimal, y resaltado de los registros modificados desde la
// última actualización. Doble click en un valor lo edita en vivo (RP7).
// Al pie, una sección plegable muestra los CSRs de máquina (RP8-F4).
import { api, type MachineState } from "../api";
import { t } from "../i18n";

const ABI: string[] = [
  "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1",
  "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7",
  "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
  "t3", "t4", "t5", "t6",
];

/** CSRs que expone get_csrs(), en el orden fijo del backend (RP8). */
const CSRS: string[] = [
  "mstatus", "mie", "mtvec", "mscratch", "mepc", "mcause", "mtval", "mip",
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
  private csrDetalles: HTMLDetailsElement;
  private csrCeldas: { hex: HTMLElement; dec: HTMLElement }[] = [];

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

    // Sección plegable de CSRs (RP8-F4), cerrada por defecto: recién al
    // abrirla se piden valores (cerrada no genera tráfico con el backend).
    this.csrDetalles = document.createElement("details");
    this.csrDetalles.className = "registros-csrs";
    const resumen = document.createElement("summary");
    resumen.textContent = t("registros.csrs.titulo");
    this.csrDetalles.appendChild(resumen);
    const tablaCsr = document.createElement("table");
    tablaCsr.className = "registros-tabla";
    tablaCsr.title = t("registros.editarHint");
    const cuerpoCsr = document.createElement("tbody");
    CSRS.forEach((nombre, i) => {
      const tr = document.createElement("tr");
      tr.innerHTML = `<td>${nombre}</td><td class="mono"></td><td class="mono"></td>`;
      const hexEl = tr.children[1] as HTMLElement;
      const decEl = tr.children[2] as HTMLElement;
      this.hacerEditableCsr(hexEl, i);
      this.hacerEditableCsr(decEl, i);
      cuerpoCsr.appendChild(tr);
      this.csrCeldas.push({ hex: hexEl, dec: decEl });
    });
    tablaCsr.appendChild(cuerpoCsr);
    this.csrDetalles.appendChild(tablaCsr);
    this.csrDetalles.addEventListener("toggle", () => {
      if (this.csrDetalles.open) void this.refrescarCsrs();
    });
    root.appendChild(this.csrDetalles);

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
    void this.refrescarCsrs();
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
    for (const c of this.csrCeldas) {
      c.hex.textContent = "—";
      c.dec.textContent = "—";
    }
  }

  // ----- CSRs (RP8-F4) -----

  /** Vuelca los CSRs en la sección plegable. Solo si está abierta (cerrada
   *  = cero tráfico extra, importante con actualizar() a ~10 Hz durante una
   *  ejecución larga) y sin pisar una edición inline en curso (mismo guard
   *  que RP7). Sin sesión de emulación el rechazo se ignora en silencio. */
  private async refrescarCsrs(): Promise<void> {
    if (!this.csrDetalles.open) return;
    if (this.csrDetalles.querySelector("input")) return;
    let valores: number[];
    try {
      valores = await api.getCsrs();
    } catch {
      return; // sin sesión: la tabla conserva los «—»
    }
    for (let i = 0; i < this.csrCeldas.length && i < valores.length; i++) {
      const c = this.csrCeldas[i];
      // Un input abierto mientras esperábamos la respuesta no se pisa.
      if (!c.hex.querySelector("input")) c.hex.textContent = hex(valores[i]);
      if (!c.dec.querySelector("input")) c.dec.textContent = dec(valores[i]);
    }
  }

  // ----- edición en vivo (RP7; los CSRs de RP8 reusan el mismo flujo) -----

  private hacerEditable(celda: HTMLElement, reg: string): void {
    celda.addEventListener("dblclick", () =>
      this.editar(
        celda,
        (val) => api.writeReg(reg, val),
        async () => this.actualizar(await api.getState()),
      ),
    );
  }

  private hacerEditableCsr(celda: HTMLElement, indice: number): void {
    celda.addEventListener("dblclick", () =>
      this.editar(
        celda,
        (val) => api.writeCsr(indice, val),
        () => this.refrescarCsrs(),
      ),
    );
  }

  /** Reemplaza la celda por un input inline prellenado: Enter aplica,
   *  Esc o blur cancelan. El valor mostrado solo cambia vía `refrescar`
   *  tras una escritura aceptada. */
  private editar(
    celda: HTMLElement,
    escribir: (val: number) => Promise<void>,
    refrescar: () => Promise<void>,
  ): void {
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
      void this.aplicar(val, cerrar, escribir, refrescar);
    });
    input.addEventListener("blur", () => cerrar());
  }

  private async aplicar(
    val: number,
    cerrar: () => void,
    escribir: (val: number) => Promise<void>,
    refrescar: () => Promise<void>,
  ): Promise<void> {
    try {
      await escribir(val);
      cerrar();
      this.errorEl.hidden = true;
      // Refresco por el flujo normal: el resaltado de cambios (RF-05)
      // marca el registro recién editado.
      await refrescar();
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
