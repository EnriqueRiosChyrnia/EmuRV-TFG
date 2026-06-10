// Vista de registros (RF-05): x0–x31 + pc con nombres ABI, valores en
// hex y decimal, y resaltado de los registros modificados desde la
// última actualización.
import type { MachineState } from "../api";
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

export class Registros {
  private celdas: { hex: HTMLElement; dec: HTMLElement; fila: HTMLElement }[] = [];
  private pcHex: HTMLElement;
  private anterior: number[] | null = null;
  private pcAnterior: number | null = null;

  constructor(root: HTMLElement) {
    const tabla = document.createElement("table");
    tabla.className = "registros-tabla";
    tabla.innerHTML = `<thead><tr>
        <th>${t("registros.nombre")}</th>
        <th>${t("registros.hex")}</th>
        <th>${t("registros.dec")}</th>
      </tr></thead>`;
    const cuerpo = document.createElement("tbody");

    const filaPc = document.createElement("tr");
    filaPc.className = "registros-pc";
    filaPc.innerHTML = `<td>pc</td><td class="mono"></td><td></td>`;
    this.pcHex = filaPc.children[1] as HTMLElement;
    cuerpo.appendChild(filaPc);

    for (let i = 0; i < 32; i++) {
      const tr = document.createElement("tr");
      tr.innerHTML = `<td>${ABI[i]} <span class="registros-xn">x${i}</span></td>
        <td class="mono"></td><td class="mono"></td>`;
      cuerpo.appendChild(tr);
      this.celdas.push({
        fila: tr,
        hex: tr.children[1] as HTMLElement,
        dec: tr.children[2] as HTMLElement,
      });
    }
    tabla.appendChild(cuerpo);
    root.appendChild(tabla);
    this.limpiar();
  }

  /** Refresca valores y marca los registros que cambiaron (RF-05). */
  actualizar(estado: MachineState): void {
    const regs = estado.registers;
    for (let i = 0; i < 32; i++) {
      const c = this.celdas[i];
      c.hex.textContent = hex(regs[i]);
      c.dec.textContent = dec(regs[i]);
      const cambio = this.anterior !== null && this.anterior[i] !== regs[i];
      c.fila.classList.toggle("cambiado", cambio);
    }
    this.pcHex.textContent = hex(estado.pc);
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
    for (const c of this.celdas) {
      c.hex.textContent = "—";
      c.dec.textContent = "—";
    }
  }
}
