// Navegador del currículo de lecciones (RP15): overlay modal con la lista
// ordenada, los objetivos/paneles de la lección seleccionada ANTES de
// abrirla, progreso (completadas) y navegación siguiente/anterior. Patrón
// de overlay tomado de paneles/ayuda.ts. Separado del selector «Ejemplos».
import { t } from "../i18n";
import { LECCIONES, type Leccion } from "../lecciones";
import {
  alternarCompletada,
  guardarProgreso,
  leerProgreso,
  resetearProgreso,
  vecina,
  type Progreso,
} from "../lecciones/progreso";

export interface CallbacksLecciones {
  /// Abrir una lección normal (carga su .s al editor).
  onAbrir: (leccion: Leccion) => void;
  /// Abrir el caso multi-archivo (08): guardar a carpeta + abrir main.s con
  /// base_dir, o aviso si el usuario cancela. La maneja la app (RP-5).
  onAbrirMultiarchivo: (leccion: Leccion) => void;
}

export class Lecciones {
  private overlay: HTMLElement;
  private lista: HTMLElement;
  private detalle: HTMLElement;
  private seleccion = 0; // índice en LECCIONES
  private progreso: Progreso;

  constructor(private cb: CallbacksLecciones) {
    this.progreso = leerProgreso();
    this.overlay = document.createElement("div");
    this.overlay.id = "lecciones-overlay";
    this.overlay.hidden = true;
    this.overlay.setAttribute("role", "dialog");
    this.overlay.setAttribute("aria-modal", "true");
    this.overlay.innerHTML = `
      <div class="lecciones-caja">
        <header>
          <h2>${t("lecciones.titulo")}</h2>
          <button class="lecciones-cerrar" aria-label="${t("lecciones.cerrar")}">✕</button>
        </header>
        <div class="lecciones-cuerpo">
          <nav class="lecciones-lista" data-rol="lista"></nav>
          <section class="lecciones-detalle" data-rol="detalle"></section>
        </div>
        <footer class="lecciones-pie">
          <button class="lecciones-reset" data-rol="reset">${t("lecciones.reset")}</button>
        </footer>
      </div>`;
    document.body.appendChild(this.overlay);
    this.lista = this.overlay.querySelector("[data-rol=lista]")!;
    this.detalle = this.overlay.querySelector("[data-rol=detalle]")!;
    this.overlay.querySelector<HTMLButtonElement>(".lecciones-cerrar")!.onclick = () =>
      this.cerrar();
    this.overlay.querySelector<HTMLButtonElement>("[data-rol=reset]")!.onclick = () => {
      resetearProgreso();
      this.progreso = new Set();
      this.render();
    };
    this.overlay.addEventListener("click", (e) => {
      if (e.target === this.overlay) this.cerrar();
    });
    this.render();
  }

  private onKeydown = (e: KeyboardEvent): void => {
    if (e.key === "Escape") this.cerrar();
  };

  abrir(): void {
    this.progreso = leerProgreso();
    this.render();
    this.overlay.hidden = false;
    document.addEventListener("keydown", this.onKeydown);
  }

  cerrar(): void {
    this.overlay.hidden = true;
    document.removeEventListener("keydown", this.onKeydown);
  }

  /// Marca la lección como completada (la llama la app tras abrirla) y
  /// persiste. No abre el overlay.
  marcarCompletada(numero: number): void {
    if (!this.progreso.has(numero)) {
      this.progreso = alternarCompletada(this.progreso, numero);
      guardarProgreso(this.progreso);
    }
  }

  // ----- render -----

  private render(): void {
    this.lista.innerHTML = "";
    LECCIONES.forEach((l, i) => {
      const item = document.createElement("button");
      item.className = "lecciones-item" + (i === this.seleccion ? " activa" : "");
      const hecha = this.progreso.has(l.numero);
      item.innerHTML = `<span class="lecciones-check">${hecha ? "✓" : ""}</span>
        <span class="lecciones-num">${String(l.numero).padStart(2, "0")}</span>
        <span class="lecciones-tit">${escapar(l.titulo)}</span>`;
      item.onclick = () => {
        this.seleccion = i;
        this.render();
      };
      this.lista.appendChild(item);
    });
    this.renderDetalle();
  }

  private renderDetalle(): void {
    const l = LECCIONES[this.seleccion];
    const objetivos = l.objetivos.map((o) => `<li>${escapar(o)}</li>`).join("");
    const hecha = this.progreso.has(l.numero);
    const prev = vecina(l.numero, -1, LECCIONES.length);
    const next = vecina(l.numero, +1, LECCIONES.length);
    this.detalle.innerHTML = `
      <h3>${t("lecciones.leccion", { n: String(l.numero).padStart(2, "0") })}: ${escapar(l.titulo)}</h3>
      <h4>${t("lecciones.objetivos")}</h4>
      <ul class="lecciones-objetivos">${objetivos}</ul>
      <h4>${t("lecciones.paneles")}</h4>
      <p class="lecciones-paneles">${escapar(l.paneles)}</p>
      ${l.multiarchivo ? `<p class="lecciones-aviso">${t("lecciones.multiarchivoNota")}</p>` : ""}
      <div class="lecciones-acciones">
        <button data-rol="abrir" class="principal">${t("lecciones.abrir")}</button>
        <button data-rol="completada">${hecha ? t("lecciones.desmarcar") : t("lecciones.marcar")}</button>
      </div>
      <div class="lecciones-nav">
        <button data-rol="prev" ${prev === null ? "disabled" : ""}>${t("lecciones.anterior")}</button>
        <button data-rol="next" ${next === null ? "disabled" : ""}>${t("lecciones.siguiente")}</button>
      </div>`;

    this.detalle.querySelector<HTMLButtonElement>("[data-rol=abrir]")!.onclick = () => {
      if (l.multiarchivo) this.cb.onAbrirMultiarchivo(l);
      else this.cb.onAbrir(l);
      this.cerrar();
    };
    this.detalle.querySelector<HTMLButtonElement>("[data-rol=completada]")!.onclick = () => {
      this.progreso = alternarCompletada(this.progreso, l.numero);
      guardarProgreso(this.progreso);
      this.render();
    };
    const irA = (num: number | null) => {
      if (num === null) return;
      this.seleccion = LECCIONES.findIndex((x) => x.numero === num);
      this.render();
    };
    this.detalle.querySelector<HTMLButtonElement>("[data-rol=prev]")!.onclick = () => irA(prev);
    this.detalle.querySelector<HTMLButtonElement>("[data-rol=next]")!.onclick = () => irA(next);
  }
}

/// Escape mínimo para interpolar texto de las lecciones en HTML.
function escapar(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
