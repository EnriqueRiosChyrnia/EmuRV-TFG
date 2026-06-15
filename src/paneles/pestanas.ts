// Barra de pestañas (RP30): una fila entre la toolbar y el editor con una
// pestaña por documento abierto, su nombre, el indicador «•» de cambios sin
// guardar (RP25, ahora POR pestaña) y un botón cerrar; al final, un «+» para
// abrir una pestaña nueva. Solo UI: la app le pasa la vista (qué pintar) y
// recibe los clicks por callbacks. No toca el backend ni el editor.
import { t } from "../i18n";

/** Lo que la app le pide pintar a la barra por cada pestaña. */
export interface PestanaVista {
  /** Identidad de la pestaña (para `data-id`, depuración). */
  id: string;
  /** Nombre visible (basename o «Sin título»). */
  nombre: string;
  /** Hay cambios sin guardar en ese documento (pinta «•», RP25). */
  sucio: boolean;
  /** Se puede cerrar (no se muestra «×» cuando es la única pestaña). */
  cerrable: boolean;
}

export interface BarraCallbacks {
  /** Activar (traer al frente) la pestaña `idx`. */
  onActivar: (idx: number) => void;
  /** Cerrar la pestaña `idx`. */
  onCerrar: (idx: number) => void;
  /** Abrir una pestaña nueva (botón «+»). */
  onNueva: () => void;
}

export class BarraPestanas {
  private readonly cont: HTMLElement;
  private readonly cbs: BarraCallbacks;

  constructor(contenedor: HTMLElement, cbs: BarraCallbacks) {
    this.cont = contenedor;
    this.cbs = cbs;
    this.cont.setAttribute("role", "tablist");
  }

  /** Repinta la barra entera (idempotente). Se llama ante cualquier cambio de
   *  nombre/«•»/cantidad/activa. Pocas pestañas: re-render completo es barato. */
  render(pestanas: PestanaVista[], activaIdx: number): void {
    this.cont.replaceChildren();
    pestanas.forEach((p, i) => {
      const activa = i === activaIdx;
      const tab = document.createElement("div");
      tab.className = activa ? "pestana activa" : "pestana";
      tab.dataset.id = p.id;

      const etiqueta = document.createElement("button");
      etiqueta.type = "button";
      etiqueta.className = "pestana-label";
      etiqueta.setAttribute("role", "tab");
      etiqueta.setAttribute("aria-selected", String(activa));
      // «•» como prefijo (mismo signo que el título de ventana, RP25).
      etiqueta.textContent = (p.sucio ? "• " : "") + p.nombre;
      etiqueta.title = p.nombre;
      etiqueta.onclick = () => this.cbs.onActivar(i);
      tab.appendChild(etiqueta);

      if (p.cerrable) {
        const cerrar = document.createElement("button");
        cerrar.type = "button";
        cerrar.className = "pestana-cerrar";
        cerrar.textContent = "×";
        cerrar.title = t("pestana.cerrar");
        cerrar.setAttribute("aria-label", t("pestana.cerrar"));
        cerrar.onclick = () => this.cbs.onCerrar(i);
        tab.appendChild(cerrar);
      }
      this.cont.appendChild(tab);
    });

    const mas = document.createElement("button");
    mas.type = "button";
    mas.className = "pestana-nueva";
    mas.textContent = "+";
    mas.title = t("pestana.nueva");
    mas.setAttribute("aria-label", t("pestana.nueva"));
    mas.onclick = () => this.cbs.onNueva();
    this.cont.appendChild(mas);
  }
}
