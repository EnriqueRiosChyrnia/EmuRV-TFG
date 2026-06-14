// Mecanismo de temas (RP17-F3). Un tema = un valor de `data-tema` en
// <html> que selecciona un bloque de tokens (temas/tokens.css). Cambiar de
// tema NO recarga: las variables CSS aplican en vivo (ventaja sobre el
// idioma, que sí recarga). Persistencia en localStorage, patrón RP-15/16.

import { aplicarVarsUsuario, leerTemaUsuario, limpiarVarsUsuario } from "./usuario";

export type Tema =
  | "educativo"
  | "retro"
  | "pergamino"
  | "kawaii"
  | "disquete"
  | "usuario";
// Orden = orden del selector. «educativo» (default) primero; «usuario»
// (Personalizado, RP28) último. Los 3 del medio son los temas del diseño
// aprobado (RP17B). Cada built-in = un bloque de tokens en tokens.css;
// «usuario» NO tiene bloque: sus valores son vars inline desde localStorage
// (ver usuario.ts). Agregar un tema acá basta (selector e i18n lo derivan).
export const TEMAS: Tema[] = [
  "educativo",
  "retro",
  "pergamino",
  "kawaii",
  "disquete",
  "usuario",
];
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
/// (:root); cualquier otro = data-tema. El tema «usuario» (RP28) además
/// pinta sus variables inline desde localStorage. Devuelve el tema aplicado.
export function aplicarTema(tema: Tema): Tema {
  const t = esTema(tema) ? tema : "educativo";
  // SIEMPRE limpiar las vars inline del usuario primero: si quedaran,
  // taparían los valores de tokens.css de cualquier otro tema.
  limpiarVarsUsuario();
  if (t === "educativo") {
    document.documentElement.removeAttribute("data-tema");
  } else {
    document.documentElement.setAttribute("data-tema", t);
    if (t === "usuario") aplicarVarsUsuario(leerTemaUsuario());
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
