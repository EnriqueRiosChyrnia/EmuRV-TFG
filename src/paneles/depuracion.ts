// Panel de depuración (RP7-F4): breakpoints con condición opcional y
// watchpoints sobre datos. El panel no toca el editor: la lista de
// breakpoints (línea → dirección) llega desde main.ts, que es quien conoce
// getBreakpointLines y line_to_addr; las condiciones y los watchpoints
// viven acá. Los errores didácticos del backend (condición mal escrita,
// región inválida, máquina corriendo) se muestran en el div de error.
import { api, type WatchKind, type Watchpoint } from "../api";
import { t } from "../i18n";

const hex32 = (v: number) => `0x${(v >>> 0).toString(16).padStart(8, "0")}`;

/** Breakpoint como lo conoce la app: línea del editor + dirección
 *  (null si no hay programa ensamblado vigente). */
export interface FilaBreakpoint {
  linea: number;
  addr: number | null;
}

export interface DepuracionCallbacks {
  /** El usuario editó una condición: la app re-manda set_breakpoints. */
  onCondicionCambiada: () => void;
  /** El usuario quitó un breakpoint desde el panel (la app lo saca del margen). */
  onQuitarBreakpoint: (linea: number) => void;
}

export class Depuracion {
  private listaBp: HTMLElement;
  private listaWp: HTMLElement;
  private inputDir: HTMLInputElement;
  private selLen: HTMLSelectElement;
  private selKind: HTMLSelectElement;
  private errorEl: HTMLElement;
  private cb: DepuracionCallbacks;
  /** Condición textual por línea; se manda tal cual y el backend la parsea. */
  private condiciones = new Map<number, string>();
  private filas: FilaBreakpoint[] = [];
  /** Último juego de watchpoints ACEPTADO por el backend (más el pendiente
   *  en vuelo): semántica reemplazar-todo, ver aplicarWatchpoints. */
  private watchpoints: Watchpoint[] = [];

  constructor(root: HTMLElement, cb: DepuracionCallbacks) {
    this.cb = cb;
    root.innerHTML = `
      <div class="depuracion-seccion">
        <h3>${t("depuracion.breakpoints")}</h3>
        <div data-rol="bps"></div>
      </div>
      <details class="depuracion-watchpoints">
        <summary>${t("depuracion.watchpoints")}</summary>
        <div class="wp-ayuda">${t("depuracion.watchpointsAyuda")}</div>
        <div data-rol="wps"></div>
        <form class="depuracion-alta" data-rol="alta">
          <input type="text" class="mono" data-rol="dir" spellcheck="false"
                 placeholder="0x10010000" aria-label="${t("depuracion.direccion")}" />
          <select data-rol="len" aria-label="${t("depuracion.tamano")}">
            <option value="1">1</option>
            <option value="2">2</option>
            <option value="4" selected>4</option>
          </select>
          <select data-rol="kind" aria-label="${t("depuracion.tipo")}">
            <option value="read">${t("depuracion.lectura")}</option>
            <option value="write" selected>${t("depuracion.escritura")}</option>
            <option value="both">${t("depuracion.ambos")}</option>
          </select>
          <button type="submit">${t("depuracion.agregar")}</button>
        </form>
      </details>
      <div class="panel-error" data-rol="error" hidden></div>`;
    this.listaBp = root.querySelector("[data-rol=bps]")!;
    this.listaWp = root.querySelector("[data-rol=wps]")!;
    this.inputDir = root.querySelector("[data-rol=dir]")!;
    this.selLen = root.querySelector("[data-rol=len]")!;
    this.selKind = root.querySelector("[data-rol=kind]")!;
    this.errorEl = root.querySelector("[data-rol=error]")!;

    root.querySelector<HTMLFormElement>("[data-rol=alta]")!.addEventListener("submit", (e) => {
      e.preventDefault();
      this.agregarWatchpoint();
    });

    // Panel colapsable: click en el título alterna el cuerpo.
    const panel = root.closest<HTMLElement>(".panel");
    const titulo = panel?.querySelector<HTMLElement>("h2");
    if (panel && titulo) {
      titulo.classList.add("panel-titulo-clickeable");
      titulo.addEventListener("click", () => panel.classList.toggle("colapsado"));
    }

    this.renderBreakpoints();
    this.renderWatchpoints();
  }

  // ----- breakpoints -----

  /** Sincroniza la lista con los puntos del margen del editor (main.ts). */
  setFilasBreakpoints(filas: FilaBreakpoint[]): void {
    this.filas = filas;
    // Condiciones de líneas que ya no tienen breakpoint: se descartan.
    const vivas = new Set(filas.map((f) => f.linea));
    for (const linea of [...this.condiciones.keys()]) {
      if (!vivas.has(linea)) this.condiciones.delete(linea);
    }
    this.renderBreakpoints();
  }

  /** Condición vigente de una línea (null si está vacía). */
  condicionDe(linea: number): string | null {
    const c = this.condiciones.get(linea)?.trim();
    return c ? c : null;
  }

  private renderBreakpoints(): void {
    this.listaBp.textContent = "";
    if (this.filas.length === 0) {
      const vacio = document.createElement("div");
      vacio.className = "depuracion-vacio";
      vacio.textContent = t("depuracion.sinBreakpoints");
      this.listaBp.appendChild(vacio);
      return;
    }
    for (const f of this.filas) {
      const fila = document.createElement("div");
      fila.className = "depuracion-fila";

      const linea = document.createElement("span");
      linea.className = "depuracion-etiqueta";
      linea.textContent = t("depuracion.linea", { linea: f.linea });

      const dir = document.createElement("span");
      dir.className = "mono depuracion-dir";
      dir.textContent = f.addr !== null ? hex32(f.addr) : "—";

      const cond = document.createElement("input");
      cond.type = "text";
      cond.className = "mono depuracion-cond";
      cond.placeholder = t("depuracion.condicionPlaceholder");
      cond.spellcheck = false;
      cond.setAttribute("aria-label", t("depuracion.condicion"));
      cond.value = this.condiciones.get(f.linea) ?? "";
      // change (Enter o blur): guardar el texto tal cual —si el backend lo
      // rechaza, queda en el input para corregirlo— y reenviar.
      cond.addEventListener("change", () => {
        this.condiciones.set(f.linea, cond.value);
        this.cb.onCondicionCambiada();
      });

      fila.append(linea, dir, cond, this.botonQuitar(() => this.cb.onQuitarBreakpoint(f.linea)));
      this.listaBp.appendChild(fila);
    }
  }

  // ----- watchpoints -----

  private agregarWatchpoint(): void {
    const dir = Number.parseInt(this.inputDir.value.trim(), 16);
    if (Number.isNaN(dir)) {
      this.mostrarError(t("depuracion.direccionInvalida"));
      return;
    }
    const previos = [...this.watchpoints];
    this.watchpoints.push({
      addr: dir >>> 0,
      len: Number(this.selLen.value) as 1 | 2 | 4,
      kind: this.selKind.value as WatchKind,
    });
    void this.aplicarWatchpoints(previos, () => {
      this.inputDir.value = "";
    });
  }

  /** Re-manda los watchpoints vigentes (sesión nueva tras re-ensamblar). */
  async reaplicarWatchpoints(): Promise<void> {
    if (this.watchpoints.length === 0) return;
    try {
      await api.setWatchpoints([...this.watchpoints]);
      this.limpiarError();
    } catch (e) {
      this.mostrarError(String(e));
    }
  }

  /** Manda el juego completo al backend (reemplazar-todo). Si lo rechaza,
   *  vuelve al último juego aceptado: dejar la entrada inválida en la lista
   *  envenenaría todos los envíos siguientes. */
  private async aplicarWatchpoints(previos: Watchpoint[], alAceptar?: () => void): Promise<void> {
    try {
      await api.setWatchpoints([...this.watchpoints]);
      this.limpiarError();
      alAceptar?.();
    } catch (e) {
      this.watchpoints = previos;
      this.mostrarError(String(e));
    }
    this.renderWatchpoints();
  }

  private renderWatchpoints(): void {
    this.listaWp.textContent = "";
    if (this.watchpoints.length === 0) {
      const vacio = document.createElement("div");
      vacio.className = "depuracion-vacio";
      vacio.textContent = t("depuracion.sinWatchpoints");
      this.listaWp.appendChild(vacio);
      return;
    }
    this.watchpoints.forEach((w, i) => {
      const fila = document.createElement("div");
      fila.className = "depuracion-fila";

      const dir = document.createElement("span");
      dir.className = "mono depuracion-dir";
      dir.textContent = hex32(w.addr);

      const len = this.selector(
        [
          ["1", "1"],
          ["2", "2"],
          ["4", "4"],
        ],
        String(w.len),
        t("depuracion.tamano"),
      );
      len.onchange = () => {
        const previos = [...this.watchpoints];
        this.watchpoints[i] = { ...w, len: Number(len.value) as 1 | 2 | 4 };
        void this.aplicarWatchpoints(previos);
      };

      const kind = this.selector(
        [
          ["read", t("depuracion.lectura")],
          ["write", t("depuracion.escritura")],
          ["both", t("depuracion.ambos")],
        ],
        w.kind,
        t("depuracion.tipo"),
      );
      kind.onchange = () => {
        const previos = [...this.watchpoints];
        this.watchpoints[i] = { ...w, kind: kind.value as WatchKind };
        void this.aplicarWatchpoints(previos);
      };

      const quitar = this.botonQuitar(() => {
        const previos = [...this.watchpoints];
        this.watchpoints.splice(i, 1);
        void this.aplicarWatchpoints(previos);
      });

      fila.append(dir, len, kind, quitar);
      this.listaWp.appendChild(fila);
    });
  }

  // ----- comunes -----

  private selector(opciones: [string, string][], valor: string, etiqueta: string): HTMLSelectElement {
    const sel = document.createElement("select");
    sel.setAttribute("aria-label", etiqueta);
    for (const [v, texto] of opciones) {
      const o = document.createElement("option");
      o.value = v;
      o.textContent = texto;
      sel.appendChild(o);
    }
    sel.value = valor;
    return sel;
  }

  private botonQuitar(onClick: () => void): HTMLButtonElement {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "depuracion-quitar";
    b.textContent = "✕";
    b.title = t("depuracion.quitar");
    b.setAttribute("aria-label", t("depuracion.quitar"));
    b.onclick = onClick;
    return b;
  }

  mostrarError(mensaje: string): void {
    this.errorEl.textContent = mensaje;
    this.errorEl.hidden = false;
  }

  limpiarError(): void {
    this.errorEl.hidden = true;
  }
}
