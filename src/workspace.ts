// Persistencia del workspace (RP27 → RP30 v2): recuerda TODAS las pestañas
// abiertas (contenido + ruta) y cuál es la activa, para retomar la sesión.
// Mismo patrón del repo: clave versionada en localStorage, parseo PURO
// (testeable sin DOM) + wrappers tolerantes a entornos sin localStorage.
//
// v1 era un solo buffer `{contenido, rutaArchivo}`. v2 es `{pestanas, activa}`.
// `parsearWorkspace` devuelve SIEMPRE la forma v2: lee un objeto v2 tal cual y
// MIGRA un objeto v1 a una sola pestaña — así la sesión guardada con la versión
// anterior se recupera sin pérdida.

/// Clave del formato actual (v2).
export const CLAVE_WORKSPACE = "emurv.workspace.v2";
/// Clave del formato anterior (v1), solo para migrar al leer.
export const CLAVE_WORKSPACE_V1 = "emurv.workspace.v1";

/** Un documento persistido: solo el fuente editable (igual que v1, NO se
 *  persisten programa ni modo binario: el listado desensamblado es derivado). */
export interface DocPersistido {
  contenido: string;
  rutaArchivo: string | null;
}

export interface Workspace {
  /** Pestañas abiertas, en orden. Al menos una. */
  pestanas: DocPersistido[];
  /** Índice de la pestaña activa, acotado a [0, pestanas.length - 1]. */
  activa: number;
}

/// Sanea un documento crudo: exige `contenido` string; `rutaArchivo` string o
/// null. Devuelve null si no cumple (se descarta esa pestaña).
function sanearDoc(x: unknown): DocPersistido | null {
  if (typeof x !== "object" || x === null) return null;
  const o = x as Record<string, unknown>;
  if (typeof o.contenido !== "string") return null;
  return {
    contenido: o.contenido,
    rutaArchivo: typeof o.rutaArchivo === "string" ? o.rutaArchivo : null,
  };
}

/// Parseo PURO del JSON crudo. Devuelve la forma v2 normalizada, o null ante
/// ausencia/JSON corrupto/forma inesperada (tolerante: nunca lanza).
///   - v2 `{pestanas:[...], activa}`: se lee, saneando cada pestaña y acotando
///     `activa`; si no queda ninguna pestaña válida, null.
///   - v1 `{contenido, rutaArchivo}`: se MIGRA a una sola pestaña activa.
export function parsearWorkspace(crudo: string | null): Workspace | null {
  if (!crudo) return null;
  let o: unknown;
  try {
    o = JSON.parse(crudo);
  } catch {
    return null;
  }
  if (typeof o !== "object" || o === null) return null;
  const obj = o as Record<string, unknown>;

  // v2: lista de pestañas + activa.
  if (Array.isArray(obj.pestanas)) {
    const pestanas = obj.pestanas
      .map(sanearDoc)
      .filter((d): d is DocPersistido => d !== null);
    if (pestanas.length === 0) return null;
    const cruda = typeof obj.activa === "number" ? Math.trunc(obj.activa) : 0;
    const activa = Math.min(Math.max(cruda, 0), pestanas.length - 1);
    return { pestanas, activa };
  }

  // v1: un solo buffer → migrar a una pestaña.
  const doc = sanearDoc(obj);
  if (doc) return { pestanas: [doc], activa: 0 };
  return null;
}

/// Lee el workspace persistido (v2; si no hay v2 pero sí v1, lo migra), o null.
export function leerWorkspace(): Workspace | null {
  try {
    const v2 = localStorage.getItem(CLAVE_WORKSPACE);
    if (v2) return parsearWorkspace(v2);
    const v1 = localStorage.getItem(CLAVE_WORKSPACE_V1);
    if (v1) return parsearWorkspace(v1); // migración v1 → v2
  } catch {
    // sin localStorage: arranca con el default
  }
  return null;
}

/// Persiste el workspace (v2). Sin localStorage: vale para la sesión, no rompe.
export function guardarWorkspace(w: Workspace): void {
  try {
    localStorage.setItem(CLAVE_WORKSPACE, JSON.stringify(w));
  } catch {
    // no-op
  }
}

/// Olvida el workspace persistido (ambas versiones). Útil para un futuro «reset».
export function olvidarWorkspace(): void {
  try {
    localStorage.removeItem(CLAVE_WORKSPACE);
    localStorage.removeItem(CLAVE_WORKSPACE_V1);
  } catch {
    // no-op
  }
}
