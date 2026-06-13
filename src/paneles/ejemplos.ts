// Modal de Ejemplos (RP20-F3): la «cara» del mock (lista de .modal-item)
// sobre la lógica existente — al elegir un ejemplo, la app lo carga al
// editor (igual que el viejo <select>). Antes era un selector en la toolbar.
import { t } from "../i18n";
import { Modal, ICONO_CERRAR } from "./modal";

export interface ItemEjemplo {
  /** Clave i18n del nombre; la descripción es `${clave}.desc`. */
  clave: string;
}

export class Ejemplos {
  private modal: Modal;

  constructor(items: ItemEjemplo[], onElegir: (indice: number) => void) {
    this.modal = new Modal("modal-cuerpo-ejemplos");
    const filas = items
      .map(
        (e, i) =>
          `<button class="modal-item" data-i="${i}"><div>` +
          `<div class="mi-titulo">${t(e.clave)}</div>` +
          `<div class="mi-desc">${t(`${e.clave}.desc`)}</div></div></button>`,
      )
      .join("");
    this.modal.card.innerHTML = `
      <header class="modal-head">
        <h2>${t("toolbar.ejemplos")}<span class="mh-sub">${t("ejemplos.sub")}</span></h2>
        <button class="modal-x" aria-label="${t("ayuda.cerrar")}">${ICONO_CERRAR}</button>
      </header>
      <div class="modal-scroll">${filas}</div>`;

    this.modal.card.querySelector(".modal-scroll")!.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest<HTMLElement>("button[data-i]");
      if (!btn) return;
      onElegir(Number(btn.dataset.i));
      this.modal.cerrar();
    });
  }

  abrir(disparador?: HTMLElement | null): void {
    this.modal.abrir(disparador);
  }
}
