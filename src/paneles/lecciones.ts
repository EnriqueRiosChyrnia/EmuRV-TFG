// Navegador de lecciones (RP15) con la CARA del mock (RP20-F3): un modal
// con barra de progreso + lista de .modal-item (check si está completada,
// número si está pendiente). La LÓGICA y la PERSISTENCIA son las de RP-15
// (src/lecciones/: datos, parser, progreso) — acá SOLO cambia la
// presentación. Al elegir una lección se abre directo (un click), igual que
// «Abrir lección» del navegador viejo, reusando los callbacks.
//
// Nota (RP20): la vista de detalle del navegador de RP-15 (objetivos +
// «qué paneles mirar» en una 2ª columna) NO existe en el mock — el mock es
// una lista de un click. Acá se respeta el mock: el primer objetivo de cada
// lección va como descripción del item; abrir es directo. La persistencia
// de progreso no cambia.
import { t } from "../i18n";
import { Modal, ICONO_CERRAR } from "./modal";
import { LECCIONES, type Leccion } from "../lecciones";
import {
  alternarCompletada,
  guardarProgreso,
  leerProgreso,
  resetearProgreso,
  type Progreso,
} from "../lecciones/progreso";

export interface CallbacksLecciones {
  /// Abrir una lección normal (carga su .s al editor).
  onAbrir: (leccion: Leccion) => void;
  /// Abrir el caso multi-archivo (08): la app pide carpeta y resuelve .include.
  onAbrirMultiarchivo: (leccion: Leccion) => void;
}

const ICONO_CHECK = `<svg class="ico modal-check" viewBox="0 0 24 24" width="20" height="20"><path d="M20 6 9 17l-5-5"/></svg>`;

export class Lecciones {
  private modal: Modal;
  private progreso: Progreso;

  constructor(private cb: CallbacksLecciones) {
    this.progreso = leerProgreso();
    this.modal = new Modal("modal-cuerpo-lecciones");

    // Delegación: elegir una lección la abre; el botón reset reinicia.
    this.modal.card.addEventListener("click", (e) => {
      const objetivo = e.target as HTMLElement;
      if (objetivo.closest("[data-rol=reset]")) {
        resetearProgreso();
        this.progreso = new Set();
        this.render();
        return;
      }
      const item = objetivo.closest<HTMLElement>("button.modal-item[data-i]");
      if (!item) return;
      const l = LECCIONES[Number(item.dataset.i)];
      if (l.multiarchivo) this.cb.onAbrirMultiarchivo(l);
      else this.cb.onAbrir(l);
      this.cerrar();
    });

    this.render();
  }

  private render(): void {
    const total = LECCIONES.length;
    const hechas = LECCIONES.filter((l) => this.progreso.has(l.numero)).length;
    const pct = total === 0 ? 0 : Math.round((hechas / total) * 100);
    // «en curso» = la primera lección no completada (la que toca seguir).
    const enCurso = LECCIONES.find((l) => !this.progreso.has(l.numero))?.numero ?? -1;

    const filas = LECCIONES.map((l, i) => {
      const hecha = this.progreso.has(l.numero);
      const marca = hecha
        ? ICONO_CHECK
        : `<span class="modal-num">${l.numero}</span>`;
      const nota =
        l.numero === enCurso ? ` <span class="mh-sub">· ${t("lecciones.enCurso")}</span>` : "";
      const desc = escapar(l.objetivos[0] ?? l.paneles);
      return (
        `<button class="modal-item" data-i="${i}">${marca}<div>` +
        `<div class="mi-titulo">${escapar(l.titulo)}${nota}</div>` +
        `<div class="mi-desc">${desc}</div></div></button>`
      );
    }).join("");

    this.modal.card.innerHTML = `
      <header class="modal-head">
        <h2>${t("lecciones.titulo")}<span class="mh-sub">${t("lecciones.progresoSub", {
          hechas: String(hechas),
          total: String(total),
        })}</span></h2>
        <button class="modal-x" aria-label="${t("lecciones.cerrar")}">${ICONO_CERRAR}</button>
      </header>
      <div class="modal-progreso">
        <div class="etq">${t("lecciones.tuProgreso")}
          <button class="ml-reset" data-rol="reset">${t("lecciones.reset")}</button></div>
        <div class="barra"><i style="width:${pct}%"></i></div>
      </div>
      <div class="modal-scroll">${filas}</div>`;
  }

  /// Marca la lección como completada (la llama la app tras abrirla) y
  /// persiste. No abre el modal. La lógica/persistencia son las de RP-15.
  marcarCompletada(numero: number): void {
    if (!this.progreso.has(numero)) {
      this.progreso = alternarCompletada(this.progreso, numero);
      guardarProgreso(this.progreso);
    }
  }

  abrir(disparador?: HTMLElement | null): void {
    this.progreso = leerProgreso();
    this.render();
    this.modal.abrir(disparador);
  }

  cerrar(): void {
    this.modal.cerrar();
  }
}

/// Escape mínimo para interpolar texto de las lecciones en HTML.
function escapar(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
