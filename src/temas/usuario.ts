// Tema de usuario (RP28, «Parte B» de RP17): un tema que arma el USUARIO.
// A diferencia de los 5 built-in (cada uno un bloque [data-tema] en
// tokens.css), sus valores NO viven en el CSS: son un objeto guardado en
// localStorage que se aplica como variables INLINE sobre :root
// (document.documentElement.style.setProperty). Decisión (BITACORA RP28):
// inline-vars > un bloque CSS editado por JS, porque el estilo inline gana
// la cascada sin tocar tokens.css ni pelear especificidad, y el preview en
// vivo es un simple setProperty. Contracara: esas vars inline taparían a
// CUALQUIER otro tema, así que aplicarTema() las limpia al salir de
// «usuario» (ver index.ts → limpiarVarsUsuario).
//
// Default = copia EXACTA del «Educativo» (DEFAULT_USUARIO). Para no duplicar
// valores a ciegas, usuario.test.ts verifica DEFAULT_USUARIO === el bloque
// educativo de tokens.css (fuente única, sin drift).

export const CLAVE_TEMA_USUARIO = "emurv.tema-usuario.v1";

// Tokens editables por el usuario. El orden = el orden en el editor. Cada
// nombre mapea a la variable CSS --<nombre>. Cubre superficies, texto,
// acento/estados y la sintaxis del editor (--sx-*) — así el código también
// recolorea con el tema propio, y el aviso de contraste puede vigilarlos.
export const TOKENS_USUARIO = [
  "fondo",
  "fondo-panel",
  "fondo-claro",
  "borde",
  "texto",
  "texto-suave",
  "texto-sobre-acento",
  "acento",
  "ok",
  "error",
  "cambio",
  "sx-com",
  "sx-dir",
  "sx-lbl",
  "sx-ins",
  "sx-str",
  "sx-num",
] as const;

export type TokenUsuario = (typeof TOKENS_USUARIO)[number];
export type TemaUsuario = Record<TokenUsuario, string>;

// Default = «Educativo». Verificado === al bloque [data-tema="educativo"] de
// tokens.css por usuario.test.ts (si alguien cambia un valor del Educativo,
// el test obliga a actualizar acá → no hay drift silencioso).
export const DEFAULT_USUARIO: TemaUsuario = {
  fondo: "#151612",
  "fondo-panel": "#20211c",
  "fondo-claro": "#292b24",
  borde: "#44473d",
  texto: "#f7f4e6",
  "texto-suave": "#aaa99b",
  "texto-sobre-acento": "#16122b",
  acento: "#9b8cff",
  ok: "#6fcf6f",
  error: "#ff7a7a",
  cambio: "#ffd166",
  "sx-com": "#aaa99b",
  "sx-dir": "#7fcf8f",
  "sx-lbl": "#a99bff",
  "sx-ins": "#c4b6ff",
  "sx-str": "#9ec97f",
  "sx-num": "#e0a36b",
};

/// ¿`v` es un color `#rrggbb` (6 hex)? Validación pura para parsear lo
/// guardado / lo importado.
export function esHex(v: unknown): v is string {
  return typeof v === "string" && /^#[0-9a-fA-F]{6}$/.test(v);
}

/// Una copia nueva del default (no exponer la constante a mutación).
export function temaUsuarioPorDefecto(): TemaUsuario {
  return { ...DEFAULT_USUARIO };
}

/// Normaliza un objeto cualquiera a un TemaUsuario: arranca del default y
/// pisa solo las claves conocidas con valor `#hex` válido. Así un objeto
/// parcial, viejo o con basura igual carga (lo que falte hereda del
/// Educativo) en vez de descartarse entero.
export function normalizarTema(v: unknown): TemaUsuario {
  const base = temaUsuarioPorDefecto();
  if (v && typeof v === "object") {
    const obj = v as Record<string, unknown>;
    for (const k of TOKENS_USUARIO) {
      if (esHex(obj[k])) base[k] = obj[k] as string;
    }
  }
  return base;
}

/// Tema de usuario guardado (normalizado), o una copia del Educativo si no
/// hay nada / es inválido. Nunca tira.
export function leerTemaUsuario(): TemaUsuario {
  try {
    const crudo = localStorage.getItem(CLAVE_TEMA_USUARIO);
    if (crudo) return normalizarTema(JSON.parse(crudo));
  } catch {
    // sin localStorage o JSON corrupto: default
  }
  return temaUsuarioPorDefecto();
}

/// Persiste el tema de usuario. Sin localStorage: vale para la sesión.
export function guardarTemaUsuario(tema: TemaUsuario): void {
  try {
    localStorage.setItem(CLAVE_TEMA_USUARIO, JSON.stringify(tema));
  } catch {
    // sin localStorage: no persiste, no rompe
  }
}

/// Aplica el tema de usuario como variables CSS inline sobre :root (preview
/// en vivo y selección). El estilo inline gana la cascada sobre los bloques
/// [data-tema] de tokens.css.
export function aplicarVarsUsuario(tema: TemaUsuario): void {
  const el = document.documentElement;
  for (const k of TOKENS_USUARIO) el.style.setProperty(`--${k}`, tema[k]);
}

/// Quita las variables inline del tema de usuario. Imprescindible al cambiar
/// a otro tema: si no, estas vars inline taparían sus valores de tokens.css.
export function limpiarVarsUsuario(): void {
  const el = document.documentElement;
  for (const k of TOKENS_USUARIO) el.style.removeProperty(`--${k}`);
}
