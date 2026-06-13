// i18n (RNF-05, RP16): textos externalizados, multi-idioma con español
// primario e inglés como alcance. El español es la lengua de la tesis; el
// inglés cubre SOLO la UI cáscara (los diagnósticos del ensamblador y las
// lecciones siguen en español en v1 — limitación documentada).
import en from "./en.json";
import es from "./es.json";

export type Lang = "es" | "en";
type Dict = Record<string, string>;

const DICTS: Record<Lang, Dict> = { es, en };
/// Clave versionada en localStorage (patrón RP-15).
export const CLAVE_LANG = "emurv.lang.v1";

/// Resolución PURA del texto: idioma activo → fallback a español → la
/// propia clave. Interpola `{params}`. Separada para poder testearla sin
/// localStorage ni los JSON reales (anti-drift y lógica en un solo lugar).
export function traducir(
  dicts: Record<string, Dict>,
  lang: string,
  key: string,
  params?: Record<string, string | number>,
): string {
  let text = dicts[lang]?.[key] ?? dicts.es?.[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}

/// Idioma persistido (o español por defecto / si no hay localStorage).
function leerLang(): Lang {
  try {
    const v = localStorage.getItem(CLAVE_LANG);
    if (v === "es" || v === "en") return v;
  } catch {
    // sin localStorage (tests/modo privado): español
  }
  return "es";
}

let activo: Lang = leerLang();

/// Idioma activo actual.
export function getLang(): Lang {
  return activo;
}

/// Cambia y persiste el idioma. NO re-renderiza: la UI se construye de
/// forma imperativa (toolbar/paneles leen t() al crearse), así que el
/// cambio se aplica recargando la ventana tras persistir (ver main.ts) —
/// simple y robusto, sin estado a medio traducir (BITACORA RP16-F1).
export function setLang(lang: Lang): void {
  activo = lang;
  try {
    localStorage.setItem(CLAVE_LANG, lang);
  } catch {
    // sin localStorage: el idioma vale para esta sesión, no persiste
  }
}

/// Texto traducido al idioma activo, interpolando `{claves}` con `params`.
export function t(key: string, params?: Record<string, string | number>): string {
  return traducir(DICTS, activo, key, params);
}
