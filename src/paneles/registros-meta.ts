// Orden y metadatos de los registros enteros (RP18-F2). Módulo PURO (sin
// DOM ni api) a propósito: así un test puede FIJAR el orden x0..x31 — la
// convención estándar de RISC-V (RARS, gdb, manuales) — y cazar cualquier
// reordenamiento accidental, que el plan PROHÍBE.

/** Nombre ABI de cada registro entero, índice = número (x0..x31). NO
 *  reordenar: el orden ABI no es contiguo por tipo a propósito (t0–t2=x5–x7,
 *  s0–s1=x8–x9, a0–a7=x10–x17, s2–s11=x18–x27, t3–t6=x28–x31), igual que en
 *  toda la documentación real que el estudiante va a usar. */
export const ABI: readonly string[] = [
  "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1",
  "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7",
  "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
  "t3", "t4", "t5", "t6",
];

/** Índices ABI donde empieza un BLOQUE funcional → ahí va un separador
 *  visual sutil (línea). Solo cosmético; el orden no cambia.
 *  ra=x1 (especiales), t0=x5 (temporales), s0=x8 (guardados), a0=x10
 *  (argumentos), s2=x18 (guardados, 2ª tanda), t3=x28 (temporales, 2ª). */
export const GRUPOS_ABI: ReadonlySet<number> = new Set([1, 5, 8, 10, 18, 28]);

/** Clave i18n del rol de un registro (tooltip de la celda del nombre).
 *  Los roles se agrupan: t* = temporal, s* = guardado, a* = argumento. */
export function claveRol(abi: string): string {
  switch (abi) {
    case "zero": return "registros.rol.zero";
    case "ra": return "registros.rol.ra";
    case "sp": return "registros.rol.sp";
    case "gp": return "registros.rol.gp";
    case "tp": return "registros.rol.tp";
  }
  if (/^t\d+$/.test(abi)) return "registros.rol.temporal";
  if (/^s\d+$/.test(abi)) return "registros.rol.guardado";
  if (/^a\d+$/.test(abi)) return "registros.rol.argumento";
  return "registros.rol.otro";
}
