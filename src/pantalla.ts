// Ventana de pantalla desacoplada: solo el canvas 320×240, alimentado por
// el mismo comando `read_framebuffer` que el panel embebido. El protocolo
// de seq permite que ambos consuman frames en paralelo sin robárselos:
// esta ventana arranca con seq=0, así recibe el frame vigente al abrirse.
import { api } from "./api";

const ANCHO = 320;
const ALTO = 240;
const PERIODO_MS = 33; // ~30 Hz (RF2)

const canvas = document.getElementById("pantalla-canvas") as HTMLCanvasElement;
const ctx = canvas.getContext("2d")!;
const imageData = ctx.createImageData(ANCHO, ALTO); // reusado (RF1)

let seq = 0n;
let leyendo = false;

async function refrescar(): Promise<void> {
  if (leyendo) return;
  leyendo = true;
  try {
    const buf = await api.readFramebuffer(seq);
    if (buf.byteLength === 8 + ANCHO * ALTO * 4) {
      seq = new DataView(buf).getBigUint64(0, true);
      imageData.data.set(new Uint8ClampedArray(buf, 8));
      ctx.putImageData(imageData, 0, 0);
    }
  } catch {
    // sin sesión de emulación todavía: no hay nada que pintar
  } finally {
    leyendo = false;
  }
}

void refrescar();
window.setInterval(() => void refrescar(), PERIODO_MS);
