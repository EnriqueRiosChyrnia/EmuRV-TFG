// Navegador de lecciones (RP15) en un modal de DOS COLUMNAS (RP21): a la
// izquierda la lista (con check/num + barra de progreso, de RP-20), a la
// derecha el DETALLE de la lección elegida — objetivos, qué paneles mirar,
// «Abrir lección» y navegación siguiente/anterior. RP-20 había perdido esa
// vista previa (regresión de RP-15); RP-21 la recupera con la estética del
// rediseño (mismo lenguaje que la Ayuda 2-col).
//
// La DATA (objetivos[]/paneles) sale del parser; la LÓGICA y el PROGRESO son
// los de RP-15 (src/lecciones/: vecina, progreso) — acá SOLO se presentan.
// Elegir un item en la lista MUESTRA su detalle (no abre); «Abrir lección»
// carga al editor y cierra; el caso multiarchivo (08) se dispara recién ahí.
import { t } from "../i18n";
import { Modal, ICONO_CERRAR } from "./modal";
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
  /// Abrir el caso multi-archivo (08): la app pide carpeta y resuelve .include.
  onAbrirMultiarchivo: (leccion: Leccion) => void;
}

const ICONO_CHECK = `<svg class="ico modal-check" viewBox="0 0 24 24" width="20" height="20"><path d="M20 6 9 17l-5-5"/></svg>`;

export class Lecciones {
  private modal: Modal;
  private progreso: Progreso;
  private seleccion = 0; // índice en LECCIONES de la lección mostrada en el detalle
  private lista!: HTMLElement;
  private detalle!: HTMLElement;

  constructor(private cb: CallbacksLecciones) {
    this.progreso = leerProgreso();
    this.modal = new Modal("modal-cuerpo-lecciones");

    // Un solo handler delegado en la card: reset, abrir, sig/ant, y elegir
    // un item de la lista (que MUESTRA el detalle, no abre).
    this.modal.card.addEventListener("click", (e) => {
      const el = e.target as HTMLElement;
      if (el.closest("[data-rol=reset]")) {
        resetearProgreso();
        this.progreso = new Set();
        this.render();
        return;
      }
      if (el.closest("[data-rol=abrir]")) {
        this.abrirSeleccion();
        return;
      }
      if (el.closest("[data-rol=prev]")) return this.navegar(-1);
      if (el.closest("[data-rol=next]")) return this.navegar(+1);
      const item = el.closest<HTMLElement>("button.modal-item[data-i]");
      if (item) {
        this.seleccion = Number(item.dataset.i);
        this.marcarActiva();
        this.renderDetalle();
      }
    });

    this.render();
  }

  /// Índice de la lección «en curso» (primera no completada) o 0.
  private indiceEnCurso(): number {
    const i = LECCIONES.findIndex((l) => !this.progreso.has(l.numero));
    return i >= 0 ? i : 0;
  }

  private abrirSeleccion(): void {
    const l = LECCIONES[this.seleccion];
    if (l.multiarchivo) this.cb.onAbrirMultiarchivo(l);
    else this.cb.onAbrir(l);
    this.cerrar();
  }

  /// Mueve la SELECCIÓN del detalle a la lección vecina (sig/ant), acotada
  /// por `vecina` (RP-15). No abre nada: solo cambia el detalle.
  private navegar(delta: number): void {
    const num = vecina(LECCIONES[this.seleccion].numero, delta, LECCIONES.length);
    if (num === null) return;
    this.seleccion = LECCIONES.findIndex((x) => x.numero === num);
    this.marcarActiva();
    this.renderDetalle();
  }

  private marcarActiva(): void {
    this.lista.querySelectorAll<HTMLElement>(".modal-item").forEach((b) => {
      b.classList.toggle("activa", Number(b.dataset.i) === this.seleccion);
    });
  }

  /// Reconstruye la card entera (lista + barra + contenedor de detalle).
  private render(): void {
    const total = LECCIONES.length;
    const hechas = LECCIONES.filter((l) => this.progreso.has(l.numero)).length;
    const pct = total === 0 ? 0 : Math.round((hechas / total) * 100);
    const enCurso = LECCIONES.find((l) => !this.progreso.has(l.numero))?.numero ?? -1;

    const items = LECCIONES.map((l, i) => {
      const hecha = this.progreso.has(l.numero);
      const marca = hecha ? ICONO_CHECK : `<span class="modal-num">${l.numero}</span>`;
      const nota =
        l.numero === enCurso ? ` <span class="mh-sub">· ${t("lecciones.enCurso")}</span>` : "";
      const activa = i === this.seleccion ? " activa" : "";
      return (
        `<button class="modal-item${activa}" data-i="${i}">${marca}<div>` +
        `<div class="mi-titulo">${escapar(l.titulo)}${nota}</div></div></button>`
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
      <div class="lec-body">
        <nav class="lec-lista" data-rol="lista">${items}</nav>
        <div class="lec-detalle" data-rol="detalle"></div>
      </div>`;
    this.lista = this.modal.card.querySelector("[data-rol=lista]")!;
    this.detalle = this.modal.card.querySelector("[data-rol=detalle]")!;
    this.renderDetalle();
  }

  /// Llena el panel derecho con el detalle de la lección seleccionada.
  private renderDetalle(): void {
    const l = LECCIONES[this.seleccion];
    const objetivos = l.objetivos.map((o) => `<li>${escapar(o)}</li>`).join("");
    const prev = vecina(l.numero, -1, LECCIONES.length);
    const next = vecina(l.numero, +1, LECCIONES.length);
    this.detalle.innerHTML = `
      <h3>${t("lecciones.leccion", { n: String(l.numero).padStart(2, "0") })}: ${escapar(l.titulo)}</h3>
      <h4>${t("lecciones.objetivos")}</h4>
      <ul class="lec-objetivos">${objetivos}</ul>
      <h4>${t("lecciones.paneles")}</h4>
      <p>${escapar(l.paneles)}</p>
      ${
        l.multiarchivo
          ? `<div class="ay-nota"><span class="et">.include</span>${t("lecciones.multiarchivoNota")}</div>`
          : ""
      }
      <div class="lec-acciones">
        <button class="principal" data-rol="abrir">${t("lecciones.abrir")}</button>
      </div>
      <div class="lec-nav">
        <button data-rol="prev"${prev === null ? " disabled" : ""}>${t("lecciones.anterior")}</button>
        <button data-rol="next"${next === null ? " disabled" : ""}>${t("lecciones.siguiente")}</button>
      </div>`;
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
    this.seleccion = this.indiceEnCurso(); // primera apertura: «en curso»
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
