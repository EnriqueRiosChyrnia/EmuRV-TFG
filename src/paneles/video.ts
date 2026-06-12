// Pantalla gráfica (framebuffer VGA 13h, 320×240): canvas con volcado por
// putImageData sobre un ImageData reusado (RB-1) y timer de ~30 Hz con flag
// sucio por número de secuencia (RB-2): el backend conserva el último frame
// y responde solo si hay uno más nuevo que el que este consumidor ya pintó,
// así el panel embebido y la ventana desacoplada pueden convivir.
// El panel está oculto hasta que el programa usa la VRAM o el usuario lo
// muestra con el botón de la toolbar (RB-4).
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { api } from "../api";
import { t } from "../i18n";
import { mapearTecla } from "../teclado";

const ANCHO = 320;
const ALTO = 240;
const PERIODO_MS = 33; // ~30 Hz
const VENTANA = "pantalla"; // label de la ventana desacoplada

export class Video {
  private seccion: HTMLElement;
  private canvas: HTMLCanvasElement;
  /** Aviso de «teclado capturado» visible mientras el canvas tiene foco. */
  private indicador: HTMLElement;
  private ctx: CanvasRenderingContext2D;
  private imageData: ImageData; // reusado entre frames (RB-1)
  private timer: number | null = null;
  private leyendo = false;
  // Llegó un pedido de refresco mientras otro estaba en vuelo (p. ej. el
  // volcado final al detenerse): se repite al terminar para no perderlo.
  private repetirAlTerminar = false;
  private seq = 0n; // última publicación pintada por ESTE consumidor
  private desacoplada = false;

  constructor(seccion: HTMLElement, canvas: HTMLCanvasElement, botonDesacoplar: HTMLElement) {
    this.seccion = seccion;
    this.canvas = canvas;
    canvas.width = ANCHO;
    canvas.height = ALTO;
    this.ctx = canvas.getContext("2d")!;
    this.imageData = this.ctx.createImageData(ANCHO, ALTO);
    botonDesacoplar.onclick = () => void this.abrirVentanaAparte();

    // Captura de teclado (TEC4): SOLO tras un click explícito del usuario
    // en el canvas, nunca robándole el foco al editor. Los listeners viven
    // en el propio canvas (no en window/document), así que con el panel
    // oculto o desacoplado el canvas no puede tener foco y no captura nada;
    // no hay listener global que limpiar en el ciclo de vida del panel.
    canvas.tabIndex = 0; // hace al canvas enfocable
    this.indicador = document.createElement("div");
    this.indicador.className = "video-indicador";
    this.indicador.textContent = t("video.tecladoCapturado");
    this.indicador.hidden = true;
    canvas.insertAdjacentElement("afterend", this.indicador);
    canvas.addEventListener("click", () => canvas.focus());
    canvas.addEventListener("focus", () => {
      this.indicador.hidden = false;
    });
    canvas.addEventListener("blur", () => {
      this.indicador.hidden = true;
    });
    canvas.addEventListener("keydown", (ev) => {
      const code = mapearTecla(ev);
      if (code === null) return; // tecla no mapeada: que la maneje el navegador
      ev.preventDefault(); // imprescindible en flechas/espacio: no scrollear
      api.pushKey(code);
    });
  }

  get visible(): boolean {
    return !this.seccion.hidden;
  }

  /** Muestra el panel embebido (salvo que la pantalla esté desacoplada). */
  mostrar(): void {
    if (this.desacoplada || !this.seccion.hidden) return;
    this.seccion.hidden = false;
    void this.refrescar();
  }

  alternar(): void {
    if (this.desacoplada) {
      // La pantalla vive en la ventana aparte: traerla al frente.
      void WebviewWindow.getByLabel(VENTANA).then((w) => w?.setFocus());
      return;
    }
    if (this.seccion.hidden) this.mostrar();
    else {
      this.seccion.hidden = true;
      this.detenerTimer();
      this.canvas.blur(); // soltar la captura de teclado al ocultar
    }
  }

  /** Arranca el volcado periódico (mientras el programa corre). */
  iniciarTimer(): void {
    if (this.timer !== null || this.desacoplada) return;
    this.timer = window.setInterval(() => void this.refrescar(), PERIODO_MS);
  }

  /** Frena el timer y hace un último volcado (estado final en pantalla). */
  detenerTimerYRefrescar(): void {
    this.detenerTimer();
    if (this.visible) void this.refrescar();
  }

  detenerTimer(): void {
    if (this.timer !== null) {
      window.clearInterval(this.timer);
      this.timer = null;
    }
  }

  /** Pide un frame más nuevo que el último pintado; 0 bytes = sin cambios. */
  async refrescar(): Promise<void> {
    if (!this.visible) return;
    if (this.leyendo) {
      this.repetirAlTerminar = true;
      return;
    }
    this.leyendo = true;
    try {
      const buf = await api.readFramebuffer(this.seq);
      if (buf.byteLength === 8 + ANCHO * ALTO * 4) {
        this.seq = new DataView(buf).getBigUint64(0, true);
        this.imageData.data.set(new Uint8ClampedArray(buf, 8));
        this.ctx.putImageData(this.imageData, 0, 0);
      }
    } catch {
      // sin sesión de emulación: no hay nada que pintar
    } finally {
      this.leyendo = false;
      if (this.repetirAlTerminar) {
        this.repetirAlTerminar = false;
        void this.refrescar();
      }
    }
  }

  /** Desacopla la pantalla a una ventana propia (Tauri multi-ventana).
   *  El panel embebido se oculta mientras la ventana exista; al cerrarla,
   *  el panel vuelve. La ventana corre su propio timer (src/pantalla.ts). */
  async abrirVentanaAparte(): Promise<void> {
    const existente = await WebviewWindow.getByLabel(VENTANA);
    if (existente) {
      await existente.setFocus();
      return;
    }
    // Ocultar el panel embebido YA, antes de crear la ventana: esperar al
    // evento tauri://created es una carrera perdida (la ventana se crea
    // antes de que el listener termine de registrarse por IPC) y el panel
    // quedaba duplicado. Si la creación falla, tauri://error lo restaura.
    this.desacoplada = true;
    this.seccion.hidden = true;
    this.detenerTimer();
    this.canvas.blur(); // la captura de teclado pasa a la ventana aparte
    const w = new WebviewWindow(VENTANA, {
      url: "pantalla.html",
      title: t("video.ventana"),
      width: 648,
      height: 484,
      minWidth: 328,
      minHeight: 248,
    });
    void w.once("tauri://error", () => {
      this.desacoplada = false;
      this.mostrar();
      this.iniciarTimer();
    });
    void w.once("tauri://destroyed", () => {
      this.desacoplada = false;
      this.seq = 0n; // re-pedir el frame vigente al volver al panel
      this.mostrar();
      this.iniciarTimer();
    });
  }
}
