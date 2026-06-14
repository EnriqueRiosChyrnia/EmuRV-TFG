// Búsqueda/reemplazo del editor (RP-24): traducción del panel de
// `@codemirror/search`. CodeMirror resuelve cada etiqueta del panel con
// `state.phrase(<frase fuente en inglés>)`; este módulo mapea esas frases
// fuente a su traducción i18n para que el panel hable el idioma activo.
//
// Las frases con `$` son plantillas que CodeMirror sustituye en vivo con el
// conteo de coincidencias o el número de línea: el `$` se conserva literal en
// la traducción (no es un parámetro i18n `{...}`).
import { t } from "../i18n";

/// Frases fuente que usa el panel de `@codemirror/search` (extraídas de su
/// `dist`), mapeadas a su clave i18n. La clave del registro DEBE coincidir
/// carácter por carácter con la frase que pasa CodeMirror a `state.phrase`.
const FRASES: Record<string, string> = {
  Find: "editor.buscar.find",
  Replace: "editor.buscar.replace",
  next: "editor.buscar.next",
  previous: "editor.buscar.previous",
  all: "editor.buscar.all",
  "match case": "editor.buscar.matchCase",
  "by word": "editor.buscar.byWord",
  regexp: "editor.buscar.regexp",
  replace: "editor.buscar.replaceBtn",
  "replace all": "editor.buscar.replaceAll",
  close: "editor.buscar.close",
  "current match": "editor.buscar.currentMatch",
  "replaced $ matches": "editor.buscar.replacedMatches",
  "replaced match on line $": "editor.buscar.replacedOnLine",
  "on line": "editor.buscar.onLine",
  "Go to line": "editor.buscar.gotoLine",
  go: "editor.buscar.go",
};

/// Mapa `frase-fuente → traducción` para `EditorState.phrases.of(...)`, en el
/// idioma activo. Se evalúa al construir el editor; el cambio de idioma
/// recarga la ventana (ver i18n/setLang), así que no hace falta reactividad.
export function frasesBusqueda(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [fuente, clave] of Object.entries(FRASES)) {
    out[fuente] = t(clave);
  }
  return out;
}
