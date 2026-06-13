// Mecanismo de temas (RP17-F3). Un tema = un valor de `data-tema` en
// <html> que selecciona un bloque de tokens (temas/tokens.css). Cambiar de
// tema NO recarga: las variables CSS aplican en vivo (ventaja sobre el
// idioma, que sí recarga). Persistencia en localStorage, patrón RP-15/16.

export type Tema = "educativo" | "retro" | "pergamino" | "kawaii" | "disquete";
// Orden = orden del selector. «educativo» (default) primero. Los 3 últimos
// son los temas del diseño aprobado (RP17B). Un tema = un bloque de tokens
// en tokens.css; agregar uno acá basta (selector e i18n lo derivan).
export const TEMAS: Tema[] = ["educativo", "retro", "pergamino", "kawaii", "disquete"];
export const CLAVE_TEMA = "emurv.tema.v1";

/// ¿`v` es un tema válido? (validación PURA, para parsear lo persistido).
export function esTema(v: unknown): v is Tema {
  return (TEMAS as string[]).includes(v as string);
}

/// Tema persistido, o «educativo» (default) si no hay nada / es inválido.
export function leerTema(): Tema {
  try {
    const v = localStorage.getItem(CLAVE_TEMA);
    if (esTema(v)) return v;
  } catch {
    // sin localStorage: default
  }
  return "educativo";
}

/// Aplica el tema al documento: el default «educativo» = sin atributo
/// (:root); cualquier otro = data-tema. Devuelve el tema aplicado.
export function aplicarTema(tema: Tema): Tema {
  const t = esTema(tema) ? tema : "educativo";
  if (t === "educativo") {
    document.documentElement.removeAttribute("data-tema");
  } else {
    document.documentElement.setAttribute("data-tema", t);
  }
  return t;
}

/// Cambia el tema: aplica EN VIVO (sin recargar) y persiste.
export function setTema(tema: Tema): void {
  const t = aplicarTema(tema);
  try {
    localStorage.setItem(CLAVE_TEMA, t);
  } catch {
    // sin localStorage: vale para la sesión, no persiste
  }
}

/// Aplica el tema guardado al arrancar (llamar una vez en el bootstrap).
export function inicializarTema(): Tema {
  return aplicarTema(leerTema());
}
