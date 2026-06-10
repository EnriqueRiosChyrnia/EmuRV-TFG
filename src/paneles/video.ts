// Pantalla gráfica (framebuffer VGA 13h, 320×240): canvas con volcado por
// putImageData sobre un ImageData reusado (RF1) y timer de ~30 Hz con flag
// sucio resuelto en el backend (RF2: si la VRAM no cambió llegan 0 bytes).
// El panel está oculto hasta que el programa usa la VRAM o el usuario lo
// muestra con el botón de la toolbar (RF4).
import { api } from "../api";

const ANCHO = 320;
const ALTO = 240;
const PERIODO_MS = 33; // ~30 Hz

export class Video {
  private seccion: HTMLElement;
  private ctx: CanvasRenderingContext2D;
  private imageData: ImageData; // reusado entre frames (RF1)
  private timer: number | null = null;
  private leyendo = false;

  constructor(seccion: HTMLElement, canvas: HTMLCanvasElement) {
    this.seccion = seccion;
    canvas.width = ANCHO;
    canvas.height = ALTO;
    this.ctx = canvas.getContext("2d")!;
    this.imageData = this.ctx.createImageData(ANCHO, ALTO);
  }

  get visible(): boolean {
    return !this.seccion.hidden;
  }

  mostrar(): void {
    if (!this.seccion.hidden) return;
    this.seccion.hidden = false;
    void this.refrescar();
  }

  alternar(): void {
    if (this.seccion.hidden) this.mostrar();
    else {
      this.seccion.hidden = true;
      this.detenerTimer();
    }
  }

  /** Arranca el volcado periódico (mientras el programa corre). */
  iniciarTimer(): void {
    if (this.timer !== null) return;
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

  /** Pide el frame RGBA; 0 bytes = sin cambios (flag sucio del backend). */
  async refrescar(): Promise<void> {
    if (!this.visible || this.leyendo) return;
    this.leyendo = true;
    try {
      const buf = await api.readFramebuffer();
      if (buf.byteLength === ANCHO * ALTO * 4) {
        this.imageData.data.set(new Uint8ClampedArray(buf));
        this.ctx.putImageData(this.imageData, 0, 0);
      }
    } catch {
      // sin sesión de emulación: no hay nada que pintar
    } finally {
      this.leyendo = false;
    }
  }
}
