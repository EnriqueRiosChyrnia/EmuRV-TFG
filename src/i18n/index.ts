// i18n mínimo (RNF-05): textos externalizados en archivos JSON.
// El español es la lengua primaria; agregar otro idioma = otro JSON.
import es from "./es.json";

type Dict = Record<string, string>;
const dict: Dict = es;

/** Devuelve el texto traducido, interpolando {claves} con `params`. */
export function t(key: string, params?: Record<string, string | number>): string {
  let text = dict[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}
