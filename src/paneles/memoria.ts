// Vista de memoria (RF-06): hexdump navegable por dirección con accesos
// rápidos a .text, .data y la zona de pila (sp). Doble click en una
// palabra del dump la edita en vivo (RP7, 4 bytes little-endian).
import { api, type AssembleOk } from "../api";
import { t } from "../i18n";
import {
  buscarAdelante,
  buscarAtras,
  parsearPatron,
  type TipoBusqueda,
} from "./memoria-buscar";

// Región de datos mapeada (espejo del backend src-tauri/core/mod.rs:
// DATA_REGION_BASE/SIZE): la búsqueda escanea aquí — .data, .bss y el heap.
const DATA_REGION_BASE = 0x1000_0000;
const DATA_REGION_FIN = DATA_REGION_BASE + 0x0040_0000; // 4 MiB

const FILAS = 16;
const BYTES_POR_FILA = 16;

const hex32 = (v: number) => `0x${(v >>> 0).toString(16).padStart(8, "0")}`;

/** Acepta decimal (42, -7) o hexadecimal (0x2a, -0x10); null si no parsea
 *  o se sale del rango de 32 bits. (Mismo criterio que registros.ts.) */
function parsearValor(texto: string): number | null {
  const v = texto.trim();
  let n: number;
  if (/^-?\d+$/.test(v)) n = Number.parseInt(v, 10);
  else if (/^-?0x[0-9a-f]+$/i.test(v)) n = Number.parseInt(v, 16);
  else return null;
  if (!Number.isSafeInteger(n) || n < -0x8000_0000 || n > 0xffff_ffff) return null;
  return n;
}

export class Memoria {
  private dump: HTMLElement;
  private inputDir: HTMLInputElement;
  private direccion = 0x1001_0000;
  private programa: AssembleOk | null = null;
  /** dirección → instrucción decodificada (RP11); null = sin listado. */
  private disasm: Record<string, number | string> | null = null;
  private aviso: HTMLElement;
  // Búsqueda en memoria (RP26).
  private busqInput: HTMLInputElement;
  private tipoSelect: HTMLSelectElement;
  /** Rango [ini, fin) de la coincidencia actual a resaltar; null = sin marca. */
  private coincidencia: { ini: number; fin: number } | null = null;
  /** Dirección de la última coincidencia (ancla de siguiente/anterior). */
  private ultimaCoincidencia: number | null = null;
  /** Evita relanzar un escaneo mientras hay otro en curso. */
  private buscando = false;

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
      <div class="memoria-busqueda">
        <input type="text" class="mono" data-rol="buscar" spellcheck="false"
               placeholder="${t("memoria.busqueda.placeholder")}"
               aria-label="${t("memoria.busqueda.placeholder")}" />
        <select data-rol="tipo" aria-label="${t("memoria.busqueda.tipoAria")}">
          <option value="byte">${t("memoria.busqueda.tipoByte")}</option>
          <option value="word">${t("memoria.busqueda.tipoWord")}</option>
          <option value="ascii">${t("memoria.busqueda.tipoAscii")}</option>
        </select>
        <button data-rol="buscar-btn">${t("memoria.busqueda.buscar")}</button>
        <button data-rol="buscar-prev" aria-label="${t("memoria.busqueda.anteriorAria")}">‹</button>
        <button data-rol="buscar-next" aria-label="${t("memoria.busqueda.siguienteAria")}">›</button>
      </div>
      <div class="memoria-aviso" hidden></div>
      <pre class="memoria-dump mono" data-rol="dump"></pre>`;
    this.dump = root.querySelector("[data-rol=dump]")!;
    this.aviso = root.querySelector(".memoria-aviso")!;
    this.inputDir = root.querySelector("[data-rol=dir]")!;
    this.busqInput = root.querySelector("[data-rol=buscar]")!;
    this.tipoSelect = root.querySelector("[data-rol=tipo]")!;
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
    // Búsqueda en memoria (RP26): Buscar/Enter arranca desde el inicio del
    // rango; ‹/› recorren coincidencias; editar el patrón reinicia el ancla.
    root.querySelector<HTMLButtonElement>("[data-rol=buscar-btn]")!.onclick = () =>
      void this.buscarPrimera();
    root.querySelector<HTMLButtonElement>("[data-rol=buscar-next]")!.onclick = () =>
      void this.buscarSiguiente();
    root.querySelector<HTMLButtonElement>("[data-rol=buscar-prev]")!.onclick = () =>
      void this.buscarAnterior();
    this.busqInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") void (e.shiftKey ? this.buscarAnterior() : this.buscarSiguiente());
    });
    this.busqInput.addEventListener("input", () => {
      this.ultimaCoincidencia = null; // patrón nuevo → siguiente empieza de cero
      this.aviso.hidden = true;
    });
    this.tipoSelect.addEventListener("change", () => {
      this.ultimaCoincidencia = null;
    });
    // Edición en vivo (RP7): doble click sobre una palabra del dump.
    this.dump.addEventListener("dblclick", (e) => {
      const palabra = (e.target as HTMLElement).closest?.(".memoria-palabra");
      if (palabra instanceof HTMLElement) this.editarPalabra(palabra);
    });
  }

  setPrograma(p: AssembleOk | null): void {
    this.programa = p;
    if (!p) this.disasm = null;
  }

  /** Columna de instrucción (RP11): se refresca al (re)ensamblar o
   *  importar; el render la muestra solo para filas dentro de .text. */
  setDisasm(d: Record<string, string> | null): void {
    this.disasm = d;
    void this.refrescar();
  }

  private parsearDir(): number {
    const v = Number.parseInt(this.inputDir.value.trim(), 16);
    return Number.isNaN(v) ? this.direccion : v >>> 0;
  }

  async irA(addr: number): Promise<void> {
    // Navegación manual (botones/Enter/eventos): limpia la marca de búsqueda.
    this.coincidencia = null;
    await this.saltar(addr);
  }

  /** Posiciona el volcado en `addr` (fila alineada a 16) y lo redibuja, sin
   *  tocar la marca de coincidencia (la búsqueda la fija antes de saltar). */
  private async saltar(addr: number): Promise<void> {
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
      this.render(bytes);
    } catch {
      this.dump.textContent = "";
      this.aviso.textContent = t("memoria.error");
      this.aviso.hidden = false;
    }
  }

  /** Arma el dump con un span por palabra de 4 bytes (editable con doble
   *  click); el texto resultante es idéntico al hexdump plano anterior. */
  private render(bytes: number[]): void {
    this.dump.textContent = "";
    for (let f = 0; f < FILAS; f++) {
      const base = (this.direccion + f * BYTES_POR_FILA) >>> 0;
      const fila = bytes.slice(f * BYTES_POR_FILA, (f + 1) * BYTES_POR_FILA);
      this.dump.append(`${hex32(base)}  `);
      for (let w = 0; w < 4; w++) {
        const off = w * 4;
        const palabra = document.createElement("span");
        palabra.className = "memoria-palabra";
        palabra.dataset.addr = String((base + off) >>> 0);
        palabra.dataset.valor = String(
          (fila[off] | (fila[off + 1] << 8) | (fila[off + 2] << 16) | (fila[off + 3] << 24)) >>> 0,
        );
        // Bytes en cero atenuados (RP19-F3, «ruido visual»): un span por
        // byte, clase .cero en los 00 → el dato real resalta. El textContent
        // resultante sigue siendo "00 1a 2f 00" (la edición lee dataset +
        // textContent, así que no se rompe; ver editarPalabra).
        fila.slice(off, off + 4).forEach((b, i) => {
          const bs = document.createElement("span");
          bs.textContent = b.toString(16).padStart(2, "0");
          if (b === 0) bs.className = "cero";
          // Resaltado de la coincidencia de búsqueda (RP26): por byte, así
          // funciona aunque el patrón cruce palabras o filas.
          const baddr = (base + off + i) >>> 0;
          if (this.coincidencia && baddr >= this.coincidencia.ini && baddr < this.coincidencia.fin) {
            bs.classList.add("memoria-coincidencia");
          }
          palabra.appendChild(bs);
          if (i < 3) palabra.append(" ");
        });
        this.dump.appendChild(palabra);
        // separación de palabras: doble espacio en la mitad de la fila
        if (w < 3) this.dump.append(w === 1 ? "  " : " ");
      }
      const ascii = fila
        .map((b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : "·"))
        .join("");
      this.dump.append(`  ${ascii}`);
      // Columna de instrucción decodificada (RP11): solo cuando la fila
      // cae en .text y hay listado de la sesión.
      const enText =
        this.programa !== null &&
        base >= this.programa.text_base &&
        base < this.programa.text_base + this.programa.text_len;
      if (enText && this.disasm) {
        const instrs: string[] = [];
        for (let w = 0; w < 4; w++) {
          const texto = this.disasm[String((base + w * 4) >>> 0)];
          if (texto !== undefined) instrs.push(String(texto));
        }
        if (instrs.length > 0) {
          const span = document.createElement("span");
          span.className = "memoria-disasm";
          span.textContent = `   ${instrs.join(" ; ")}`;
          this.dump.appendChild(span);
        }
      }
      this.dump.append("\n");
    }
  }

  // ----- búsqueda en memoria (RP26) -----

  /** Rango [ini, fin) que escanea la búsqueda: la región de datos mapeada
   *  (.data/.bss/heap). Acotado a propósito para no recorrer toda la memoria
   *  ni cruzar el hueco hasta la pila (ver BITACORA RP26). */
  private rangoBusqueda(): [number, number] {
    return [this.programa?.data_base ?? 0x1001_0000, DATA_REGION_FIN];
  }

  /** Buscar desde el principio del rango (botón Buscar). */
  private buscarPrimera(): Promise<void> {
    this.ultimaCoincidencia = null;
    return this.escanear(this.rangoBusqueda()[0], false);
  }

  /** Siguiente coincidencia tras la última (o desde el inicio si no hay). */
  private buscarSiguiente(): Promise<void> {
    const ini = this.rangoBusqueda()[0];
    const ancla = this.ultimaCoincidencia === null ? ini : this.ultimaCoincidencia + 1;
    return this.escanear(ancla, false);
  }

  /** Coincidencia anterior a la última (o la última del rango si no hay). */
  private buscarAnterior(): Promise<void> {
    const ancla = this.ultimaCoincidencia ?? this.rangoBusqueda()[1];
    return this.escanear(ancla, true);
  }

  /** Núcleo: parsea el patrón, escanea (adelante/atrás), salta y resalta.
   *  Sin coincidencia (o patrón inválido / sin programa) muestra un aviso. */
  private async escanear(ancla: number, haciaAtras: boolean): Promise<void> {
    if (this.buscando) return;
    if (!this.programa) {
      this.mostrarAviso(t("memoria.busqueda.sinPrograma"));
      return;
    }
    const tipo = this.tipoSelect.value as TipoBusqueda;
    const patron = parsearPatron(this.busqInput.value, tipo);
    if (patron === null) {
      this.mostrarAviso(t("memoria.busqueda.invalido"));
      return;
    }
    const [ini, fin] = this.rangoBusqueda();
    this.buscando = true;
    try {
      const dir = haciaAtras
        ? await buscarAtras(api.readMem, ini, fin, patron, ancla)
        : await buscarAdelante(api.readMem, ini, fin, patron, ancla);
      if (dir === null) {
        this.coincidencia = null;
        this.mostrarAviso(t("memoria.busqueda.noEncontrado", { patron: this.busqInput.value.trim() }));
        return;
      }
      this.ultimaCoincidencia = dir;
      this.coincidencia = { ini: dir, fin: (dir + patron.length) >>> 0 };
      await this.saltar(dir); // saltar (no irA) conserva la marca
    } finally {
      this.buscando = false;
    }
  }

  // ----- edición en vivo (RP7) -----

  /** Input inline sobre la palabra: Enter escribe 4 bytes little-endian,
   *  Esc o blur cancelan. La vista solo cambia tras un write_mem aceptado. */
  private editarPalabra(palabra: HTMLElement): void {
    if (palabra.querySelector("input")) return;
    const addr = Number(palabra.dataset.addr);
    const valor = Number(palabra.dataset.valor);
    const original = palabra.textContent ?? "";
    const input = document.createElement("input");
    input.type = "text";
    input.className = "edicion-inline";
    input.value = hex32(valor);
    input.spellcheck = false;
    palabra.textContent = "";
    palabra.appendChild(input);
    input.focus();
    input.select();
    let cerrado = false;
    const cerrar = () => {
      if (cerrado) return;
      cerrado = true;
      palabra.textContent = original;
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        cerrar();
        return;
      }
      if (e.key !== "Enter") return;
      const v = parsearValor(input.value);
      if (v === null) {
        // Valor ilegible: se avisa y el input queda para corregirlo.
        this.mostrarAviso(t("edicion.valorInvalido"));
        return;
      }
      void this.escribirPalabra(addr, v, cerrar);
    });
    input.addEventListener("blur", () => cerrar());
  }

  private async escribirPalabra(addr: number, valor: number, cerrar: () => void): Promise<void> {
    const u = valor >>> 0;
    const bytes = [u & 0xff, (u >>> 8) & 0xff, (u >>> 16) & 0xff, (u >>> 24) & 0xff];
    try {
      await api.writeMem(addr, bytes);
      cerrar();
      await this.refrescar(); // muestra el valor nuevo y oculta el aviso
    } catch (e) {
      cerrar(); // rechazo (corriendo, zona inválida, …): la vista no cambia
      this.mostrarAviso(String(e));
    }
  }

  private mostrarAviso(mensaje: string): void {
    this.aviso.textContent = mensaje;
    this.aviso.hidden = false;
  }
}
