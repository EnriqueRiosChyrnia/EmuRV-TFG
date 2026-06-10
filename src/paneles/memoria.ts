// Vista de memoria (RF-06): hexdump navegable por dirección con accesos
// rápidos a .text, .data y la zona de pila (sp).
import { api, type AssembleOk } from "../api";
import { t } from "../i18n";

const FILAS = 16;
const BYTES_POR_FILA = 16;

const hex32 = (v: number) => `0x${(v >>> 0).toString(16).padStart(8, "0")}`;

export class Memoria {
  private dump: HTMLElement;
  private inputDir: HTMLInputElement;
  private direccion = 0x1001_0000;
  private programa: AssembleOk | null = null;
  private aviso: HTMLElement;

  constructor(root: HTMLElement) {
    root.innerHTML = `
      <div class="memoria-controles">
        <input type="text" class="mono" data-rol="dir" spellcheck="false"
               aria-label="${t("memoria.direccion")}" />
        <button data-rol="ir">${t("memoria.ir")}</button>
        <button data-rol="text">${t("memoria.texto")}</button>
        <button data-rol="data">${t("memoria.datos")}</button>
        <button data-rol="pila">${t("memoria.pila")}</button>
      </div>
      <div class="memoria-aviso" hidden></div>
      <pre class="memoria-dump mono" data-rol="dump"></pre>`;
    this.dump = root.querySelector("[data-rol=dump]")!;
    this.aviso = root.querySelector(".memoria-aviso")!;
    this.inputDir = root.querySelector("[data-rol=dir]")!;
    this.inputDir.value = hex32(this.direccion);

    const irA = (addr: number) => void this.irA(addr);
    root.querySelector<HTMLButtonElement>("[data-rol=ir]")!.onclick = () =>
      irA(this.parsearDir());
    root.querySelector<HTMLButtonElement>("[data-rol=text]")!.onclick = () =>
      irA(this.programa?.text_base ?? 0x0040_0000);
    root.querySelector<HTMLButtonElement>("[data-rol=data]")!.onclick = () =>
      irA(this.programa?.data_base ?? 0x1001_0000);
    root.querySelector<HTMLButtonElement>("[data-rol=pila]")!.onclick = () =>
      irA(((this.programa?.sp_init ?? 0x7fff_fff0) - 0x40) >>> 0);
    this.inputDir.addEventListener("keydown", (e) => {
      if (e.key === "Enter") irA(this.parsearDir());
    });
  }

  setPrograma(p: AssembleOk | null): void {
    this.programa = p;
  }

  private parsearDir(): number {
    const v = Number.parseInt(this.inputDir.value.trim(), 16);
    return Number.isNaN(v) ? this.direccion : v >>> 0;
  }

  async irA(addr: number): Promise<void> {
    // alinear la fila inicial a 16 bytes
    this.direccion = (addr - (addr % BYTES_POR_FILA)) >>> 0;
    this.inputDir.value = hex32(this.direccion);
    await this.refrescar();
  }

  /** Relee la zona visible (al pausar/terminar/breakpoint, nunca por instrucción). */
  async refrescar(): Promise<void> {
    if (!this.programa) {
      this.dump.textContent = "";
      return;
    }
    try {
      const bytes = await api.readMem(this.direccion, FILAS * BYTES_POR_FILA);
      this.aviso.hidden = true;
      this.dump.textContent = this.formatear(bytes);
    } catch {
      this.dump.textContent = "";
      this.aviso.textContent = t("memoria.error");
      this.aviso.hidden = false;
    }
  }

  private formatear(bytes: number[]): string {
    const lineas: string[] = [];
    for (let f = 0; f < FILAS; f++) {
      const base = this.direccion + f * BYTES_POR_FILA;
      const fila = bytes.slice(f * BYTES_POR_FILA, (f + 1) * BYTES_POR_FILA);
      const hexes = fila
        .map((b, i) => b.toString(16).padStart(2, "0") + (i === 7 ? " " : ""))
        .join(" ");
      const ascii = fila
        .map((b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : "·"))
        .join("");
      lineas.push(`${hex32(base)}  ${hexes}  ${ascii}`);
    }
    return lineas.join("\n");
  }
}
