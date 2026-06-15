// Persistencia del workspace (RP27): recuerda el último buffer del editor y su
// ruta entre sesiones, para retomar donde se dejó. Mismo patrón que progreso.ts
// / i18n / temas: clave versionada en localStorage, una función de parseo PURA
// (testeable sin DOM) y wrappers de lectura/escritura tolerantes a entornos sin
// localStorage (modo privado, tests).

/// Clave versionada (si el formato cambia, se sube la v).
export const CLAVE_WORKSPACE = "emurv.workspace.v1";

export interface Workspace {
  /** Contenido del editor al cerrar. */
  contenido: string;
  /** Ruta del archivo abierto (base para `.include`), o null si es un buffer
   *  sin guardar / ejemplo embebido. */
  rutaArchivo: string | null;
}

/// Parseo PURO del JSON crudo de localStorage. Devuelve null ante ausencia,
/// JSON corrupto o forma inesperada (tolerante: nunca lanza).
export function parsearWorkspace(crudo: string | null): Workspace | null {
  if (!crudo) return null;
  try {
    const o: unknown = JSON.parse(crudo);
    if (typeof o !== "object" || o === null) return null;
    const w = o as Record<string, unknown>;
    if (typeof w.contenido !== "string") return null;
    return {
      contenido: w.contenido,
      rutaArchivo: typeof w.rutaArchivo === "string" ? w.rutaArchivo : null,
    };
  } catch {
    return null;
  }
}

/// Lee el workspace persistido, o null si no hay / no se puede.
export function leerWorkspace(): Workspace | null {
  try {
    return parsearWorkspace(localStorage.getItem(CLAVE_WORKSPACE));
  } catch {
    // sin localStorage: arranca con el default
    return null;
  }
}

/// Persiste el workspace. Sin localStorage: vale para la sesión, no rompe.
export function guardarWorkspace(w: Workspace): void {
  try {
    localStorage.setItem(CLAVE_WORKSPACE, JSON.stringify(w));
  } catch {
    // no-op
  }
}

/// Olvida el workspace persistido (no se usa hoy; útil para un futuro «reset»).
export function olvidarWorkspace(): void {
  try {
    localStorage.removeItem(CLAVE_WORKSPACE);
  } catch {
    // no-op
  }
}
