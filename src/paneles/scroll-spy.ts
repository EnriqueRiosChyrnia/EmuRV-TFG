// Scroll-spy de la Ayuda (RP20): qué sección del índice marcar como activa
// según el scroll. Lógica PURA (sin DOM) para poder testearla; la usa
// ayuda.ts tanto al hacer click en el nav (scrollTo + activa) como al
// scrollear a mano (actualiza la activa).

export interface SeccionPos {
  id: string;
  /** offsetTop de la sección relativo al contenedor scrolleable. */
  top: number;
}

/**
 * Devuelve la id de la sección «en vista»: la ÚLTIMA cuya cabecera ya pasó
 * el tope del scroll (con un pequeño margen). Asume `secciones` en orden de
 * aparición. Si ninguna pasó aún, devuelve la primera; null si no hay.
 */
export function seccionActiva(
  scrollTop: number,
  secciones: SeccionPos[],
  margen = 12,
): string | null {
  if (secciones.length === 0) return null;
  let activa = secciones[0].id;
  for (const s of secciones) {
    if (s.top - margen <= scrollTop) activa = s.id;
    else break;
  }
  return activa;
}
