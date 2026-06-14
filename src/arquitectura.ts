// Selector de arquitectura RV32/RV64 (RP32), persistido en localStorage.
// Mismo patrón que el idioma (i18n) y el tema: clave versionada `.v1`,
// lectura/escritura tolerantes a entornos sin localStorage (modo privado o
// tests). RV32 es el default histórico.

/** Ancho de la arquitectura. Coincide con `core::Xlen` del backend (serde
 *  rename_all="lowercase"): viaja tal cual al comando `assemble`. */
export type Xlen = "rv32" | "rv64";

export const CLAVE_XLEN = "emurv.xlen.v1";

/** Ancho elegido (persistido); RV32 por defecto. */
export function leerXlen(): Xlen {
  try {
    const v = localStorage.getItem(CLAVE_XLEN);
    if (v === "rv32" || v === "rv64") return v;
  } catch {
    // sin localStorage (modo privado / tests): vale el default
  }
  return "rv32";
}

/** Persiste el ancho elegido. No recarga ni reensambla: aplica al PRÓXIMO
 *  ensamblado (el usuario puede cambiarlo sin perder la sesión actual). */
export function setXlen(xlen: Xlen): void {
  try {
    localStorage.setItem(CLAVE_XLEN, xlen);
  } catch {
    // sin localStorage: vale solo para esta sesión
  }
}
