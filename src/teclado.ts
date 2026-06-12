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
// Con Ctrl o Cmd apretados no se mapea nada: esas combinaciones son atajos
// del sistema o de la app y no hay que robárselos al usuario. Alt/AltGr en
// cambio NO descalifica por sí solo: en los teclados español/LATAM (el
// público de esta herramienta) signos esenciales como @, #, [ o ] se
// tipean con AltGr (Option en macOS); si la combinación produjo un
// carácter ASCII imprimible, se acepta tal cual.

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
  // En Windows, AltGr llega como Ctrl+Alt simultáneos: getModifierState
  // lo distingue del Ctrl real, que sí hay que respetar.
  const altGr = ev.getModifierState("AltGraph");
  // Ctrl y Cmd/Win se bloquean SIEMPRE (atajos reales del sistema o de la
  // app); la única excepción es el pseudo-Ctrl del AltGr de Windows.
  if ((ev.ctrlKey && !altGr) || ev.metaKey) return null;
  // Las teclas especiales solo se mapean sin Alt: Alt+flecha, Alt+Enter,
  // etc. siguen siendo atajos del sistema.
  if (!ev.altKey) {
    const especial = ESPECIALES[ev.key];
    if (especial !== undefined) return especial;
  }
  // Imprimibles: ev.key trae el carácter YA resuelto por el layout
  // ("a", "5", " ", "@"…). Alt/AltGr apretado NO descalifica: en los
  // teclados español/LATAM @, #, [, ] y compañía se tipean con AltGr
  // (Option en macOS), y rechazarlos los volvía imposibles de escribir.
  // Solo se acepta el rango ASCII imprimible (32–126); "ñ", "á", etc.
  // quedan afuera (no entran en un byte del dispositivo).
  if (ev.key.length === 1) {
    const cp = ev.key.codePointAt(0)!;
    if (cp >= 32 && cp <= 126) return cp;
  }
  return null;
}
