// Mapeo de teclas del navegador → códigos del dispositivo de teclado MMIO
// (TEC4). Módulo único compartido por el panel embebido (paneles/video.ts)
// y la ventana de pantalla desacoplada (pantalla.ts).
//
// Tabla de códigos que recibe el programa emulado por RECEIVER_DATA:
//
//   tecla                código
//   ──────────────────   ─────────────────────────────────────────────
//   imprimibles ASCII    su byte (letras, dígitos, espacio, signos…)
//   Enter                10  (LF, igual que la consola de RARS)
//   Escape               27
//   Backspace            8
//   ArrowLeft            0x80 ┐ fuera del rango ASCII, así el
//   ArrowRight           0x81 │ programa las distingue de los
//   ArrowUp              0x82 │ caracteres imprimibles
//   ArrowDown            0x83 ┘
//   todo lo demás        null (F1–F12, modificadores solos, etc.)
//
// Con Ctrl, Cmd o Alt apretados no se mapea nada: esas combinaciones son
// atajos del sistema o de la app y no hay que robárselos al usuario.

/** Teclas especiales (no imprimibles) con código propio. */
const ESPECIALES: Readonly<Record<string, number>> = {
  Enter: 10,
  Escape: 27,
  Backspace: 8,
  ArrowLeft: 0x80,
  ArrowRight: 0x81,
  ArrowUp: 0x82,
  ArrowDown: 0x83,
};

/** Traduce un KeyboardEvent al código que espera el dispositivo de teclado,
 *  o null si la tecla no se captura (queda para el navegador/sistema).
 *  Si devuelve un código, quien captura debe llamar a preventDefault():
 *  flechas y espacio scrollearían la página si llegaran al navegador. */
export function mapearTecla(ev: KeyboardEvent): number | null {
  if (ev.ctrlKey || ev.metaKey || ev.altKey) return null;
  const especial = ESPECIALES[ev.key];
  if (especial !== undefined) return especial;
  // Imprimibles: ev.key es el carácter en sí ("a", "5", " ", …). Solo se
  // aceptan los que entran en un byte ASCII; "ñ", "á", etc. quedan afuera.
  if (ev.key.length === 1) {
    const cp = ev.key.codePointAt(0)!;
    if (cp < 128) return cp;
  }
  return null;
}
