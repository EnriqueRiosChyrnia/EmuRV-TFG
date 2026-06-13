// Progreso del currículo y navegación (RP15-F3). Funciones PURAS sobre un
// estado (Set de números completados) + un wrapper de localStorage. El
// estado vive solo en el webview (frontend puro, sin tocar el backend).

/// Clave versionada en localStorage (si el formato cambia, se sube la v).
export const CLAVE_PROGRESO = "emurv.lecciones.v1";

/// Números de las lecciones marcadas como completadas.
export type Progreso = Set<number>;

/// Parsea el progreso guardado (array JSON de números) de forma tolerante:
/// cualquier basura → conjunto vacío (nunca rompe la UI).
export function parsearProgreso(crudo: string | null): Progreso {
  if (!crudo) return new Set();
  try {
    const v: unknown = JSON.parse(crudo);
    if (Array.isArray(v)) {
      return new Set(v.filter((n): n is number => typeof n === "number"));
    }
  } catch {
    // dato corrupto: empezar limpio
  }
  return new Set();
}

/// Serializa el progreso a JSON (array ordenado, estable).
export function serializarProgreso(p: Progreso): string {
  return JSON.stringify([...p].sort((a, b) => a - b));
}

/// Marca/desmarca una lección como completada (devuelve un NUEVO set).
export function alternarCompletada(p: Progreso, numero: number): Progreso {
  const nuevo = new Set(p);
  if (nuevo.has(numero)) nuevo.delete(numero);
  else nuevo.add(numero);
  return nuevo;
}

/// La lección siguiente/anterior en orden de currículo, acotada a [1, n].
/// `delta` = +1 (siguiente) o -1 (anterior). null si se sale del rango.
export function vecina(numero: number, delta: number, total: number): number | null {
  const v = numero + delta;
  return v >= 1 && v <= total ? v : null;
}

// --- wrapper de localStorage (la capa con efecto, mínima) ---

export function leerProgreso(): Progreso {
  try {
    return parsearProgreso(localStorage.getItem(CLAVE_PROGRESO));
  } catch {
    return new Set();
  }
}

export function guardarProgreso(p: Progreso): void {
  try {
    localStorage.setItem(CLAVE_PROGRESO, serializarProgreso(p));
  } catch {
    // sin localStorage (modo privado, etc.): el progreso no persiste, pero
    // la app sigue funcionando.
  }
}

export function resetearProgreso(): void {
  try {
    localStorage.removeItem(CLAVE_PROGRESO);
  } catch {
    // ignorar
  }
}
