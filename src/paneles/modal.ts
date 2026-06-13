// Shell común de los modales (RP20): la «cara» del mock
// (propuestas-ui.html ~356-392). Un `.emurv-modal` (overlay fijo) con un
// `.modal-backdrop` (cierra al click, anim emurv-fade) y una `.modal-card`
// (anim emurv-pop). Escape cierra; el foco entra al abrir y vuelve al
// disparador al cerrar (accesibilidad). El contenido de la card lo arma el
// llamador (Ayuda / Ejemplos / Lecciones); este shell solo da el marco,
// el cierre y el foco.
//
// Adaptación mock→app: el mock usa UN overlay con `data-modal` que conmuta
// tres cards; acá cada modal es su propia instancia (un overlay con una
// card), más simple. `position:fixed` (el mock usa absolute porque vive
// dentro de un contenedor de demo; la app real cubre toda la ventana).

export class Modal {
  readonly overlay: HTMLElement;
  readonly card: HTMLElement;
  private disparador: HTMLElement | null = null;

  /** `claseCard` = clase del cuerpo (p. ej. "modal-cuerpo-ayuda"). */
  constructor(claseCard: string) {
    this.overlay = document.createElement("div");
    this.overlay.className = "emurv-modal";
    this.overlay.hidden = true;
    this.overlay.setAttribute("role", "dialog");
    this.overlay.setAttribute("aria-modal", "true");

    const backdrop = document.createElement("div");
    backdrop.className = "modal-backdrop";
    backdrop.addEventListener("click", () => this.cerrar());

    this.card = document.createElement("section");
    this.card.className = `modal-card ${claseCard}`;
    this.card.tabIndex = -1; // enfocable como fallback

    this.overlay.append(backdrop, this.card);
    document.body.appendChild(this.overlay);

    // El botón de cerrar (.modal-x) lo arma el llamador dentro de la card;
    // se cablea por delegación, sin importar cuándo se setea el contenido.
    this.overlay.addEventListener("click", (e) => {
      if ((e.target as HTMLElement).closest?.(".modal-x")) this.cerrar();
    });
  }

  private onKeydown = (e: KeyboardEvent): void => {
    if (e.key === "Escape") this.cerrar();
  };

  abrir(disparador?: HTMLElement | null): void {
    this.disparador = disparador ?? null;
    this.overlay.hidden = false;
    document.addEventListener("keydown", this.onKeydown);
    // Foco al primer control de la card (o la card misma).
    const foco = this.card.querySelector<HTMLElement>(
      "button:not([disabled]), [href], input, select, textarea, [tabindex]:not([tabindex='-1'])",
    );
    (foco ?? this.card).focus();
  }

  cerrar(): void {
    if (this.overlay.hidden) return;
    this.overlay.hidden = true;
    document.removeEventListener("keydown", this.onKeydown);
    this.disparador?.focus();
    this.disparador = null;
  }

  get abierto(): boolean {
    return !this.overlay.hidden;
  }
}
