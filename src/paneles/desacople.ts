// Mecanismo GENÉRICO de desacople de paneles (RP22): un solo módulo abre
// cualquier panel (Registros/Memoria/Depuración) en su propia ventana
// (`panel.html?panel=…`), en modo MOVER — el panel se va del lateral al
// desacoplar y vuelve al cerrar la ventana. Mismo patrón que video.ts, pero
// sin lógica por-panel: la ventana host (panel.ts) instancia la clase del
// panel pedido. No hay 3 implementaciones ad-hoc.
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

export interface PanelDesacoplable {
  /** id corto: "registros" | "memoria" | "depuracion" (query + label). */
  id: string;
  /** id de la <section> del lateral a ocultar/restaurar (modo MOVER). */
  seccionId: string;
  /** título de la ventana (i18n YA resuelto por el llamador). */
  titulo: string;
  width: number;
  height: number;
  /** Al DESACOPLAR (ventana creada): p. ej. marcar un flag de delegación
   *  (Depuración). Opcional. */
  alDesacoplar?: () => void;
  /** Al REACOPLAR (ventana cerrada): refrescar el panel embebido con el
   *  estado vivo, así recoge ediciones hechas en la ventana (write_* no
   *  emiten evento). Opcional. */
  alReacoplar?: () => void;
}

/** Botón ⧉ para el `<h2>` de un panel. `stopPropagation` evita disparar el
 *  plegado del panel (Depuración) al hacer click. */
export function botonDesacople(titulo: string, onClick: () => void): HTMLButtonElement {
  const b = document.createElement("button");
  b.className = "panel-desacoplar";
  b.innerHTML = "⧉";
  b.title = titulo;
  b.setAttribute("aria-label", titulo);
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    onClick();
  });
  return b;
}

/** Desacopla el panel a una ventana propia. Si ya existe, la enfoca. Oculta
 *  la sección del lateral YA (antes de crear la ventana: esperar el evento
 *  de creación es una carrera perdida, ver video.ts). Al cerrarse la ventana
 *  (o si falla), la sección reaparece y se refresca. */
export async function desacoplarPanel(p: PanelDesacoplable): Promise<void> {
  const label = `panel-${p.id}`;
  const existente = await WebviewWindow.getByLabel(label);
  if (existente) {
    await existente.setFocus();
    return;
  }
  const seccion = document.getElementById(p.seccionId);
  if (seccion) seccion.hidden = true;
  p.alDesacoplar?.();

  const w = new WebviewWindow(label, {
    url: `panel.html?panel=${p.id}`,
    title: p.titulo,
    width: p.width,
    height: p.height,
    minWidth: 280,
    minHeight: 220,
  });

  const restaurar = (): void => {
    if (seccion) seccion.hidden = false;
    p.alReacoplar?.();
  };
  void w.once("tauri://error", restaurar);
  void w.once("tauri://destroyed", restaurar);
}
