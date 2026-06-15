// Archivos recientes (RP30): lista de rutas abiertas/guardadas recientemente,
// accesible desde el menú Archivo. Mismo patrón del repo (progreso.ts /
// workspace.ts): clave versionada en localStorage, parseo PURO testeable +
// wrappers tolerantes a entornos sin localStorage. La lista va de más reciente
// a más antiguo, sin duplicados, acotada a MAX_RECIENTES.

/// Clave versionada (si el formato cambia, se sube la v).
export const CLAVE_RECIENTES = "emurv.recientes.v1";

/// Máximo de rutas recordadas.
export const MAX_RECIENTES = 10;

/// Parseo PURO del JSON crudo: array de strings, filtrando lo no-string y
/// acotando a MAX_RECIENTES. Tolerante: ausencia/corrupto/forma inesperada → [].
export function parsearRecientes(crudo: string | null): string[] {
  if (!crudo) return [];
  try {
    const v: unknown = JSON.parse(crudo);
    if (Array.isArray(v)) {
      return v.filter((s): s is string => typeof s === "string").slice(0, MAX_RECIENTES);
    }
  } catch {
    // dato corrupto: lista vacía
  }
  return [];
}

/// Serializa la lista a JSON (acotada a MAX_RECIENTES).
export function serializarRecientes(lista: string[]): string {
  return JSON.stringify(lista.slice(0, MAX_RECIENTES));
}

/// Agrega `ruta` al frente, sin duplicados (si ya estaba, sube al frente), y
/// acota a MAX_RECIENTES. Devuelve una NUEVA lista (no muta la entrada).
export function agregarReciente(lista: string[], ruta: string): string[] {
  return [ruta, ...lista.filter((r) => r !== ruta)].slice(0, MAX_RECIENTES);
}

// --- wrapper de localStorage (la capa con efecto, mínima) ---

export function leerRecientes(): string[] {
  try {
    return parsearRecientes(localStorage.getItem(CLAVE_RECIENTES));
  } catch {
    return [];
  }
}

export function guardarRecientes(lista: string[]): void {
  try {
    localStorage.setItem(CLAVE_RECIENTES, serializarRecientes(lista));
  } catch {
    // sin localStorage (modo privado, tests): no persiste, no rompe
  }
}
