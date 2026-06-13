// Tema del editor que SIGUE los tokens de la app (RP17B-F2). Antes el
// editor usaba oneDark FIJO, así que el código no cambiaba de color con el
// tema. Acá:
//   1. Un theme base neutro cuyo chrome (fondo, cursor, selección, gutter,
//      línea activa) sale de las variables CSS de la app (temas/tokens.css).
//   2. Un HighlightStyle que, en vez de colores inline, emite CLASES
//      (.cm-sx-*). Esas clases viven en styles.css y pintan con var(--sx-*).
// Resultado: cambiar de tema recolorea el chrome Y la sintaxis EN VIVO, sin
// recrear el editor — las variables CSS aplican al instante.
//
// Punto técnico: CodeMirror NO lee CSS vars de forma nativa en el
// HighlightStyle, pero un TagStyle admite `class` (toma precedencia sobre
// las props de color); esa clase sí puede usar var(--…) desde CSS. Por eso
// el highlight se hace por clase, no por color fijo.
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";
import type { Extension } from "@codemirror/state";

// Chrome del editor por tokens de la app. Todo es var(--…), así que recolorea
// con el tema sin tocar JS. El fondo es --fondo (sobre el que contraste.test
// verifica los --sx-*). La línea en ejecución (.cm-linea-actual) y los
// breakpoints se estilan en styles.css y mandan sobre esto (no se tocan acá).
const chromeEditor = EditorView.theme({
  "&": {
    color: "var(--texto)",
    backgroundColor: "var(--fondo)",
  },
  ".cm-content": {
    caretColor: "var(--acento)",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--acento)",
  },
  // Selección: sin drawSelection() activo, el resaltado real es el ::selection
  // nativo del contenido. Se tinta con un token (alfa del acento) para que
  // siga el tema. Se cubren ambas capas por robustez.
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
    backgroundColor: "var(--resaltado-linea)",
  },
  ".cm-content ::selection": {
    backgroundColor: "var(--resaltado-linea)",
  },
  // Gutters (números de línea y margen de breakpoints) sobre el panel.
  ".cm-gutters": {
    backgroundColor: "var(--fondo-panel)",
    color: "var(--texto-suave)",
    border: "none",
  },
  ".cm-activeLine": {
    backgroundColor: "var(--fila-par)",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "var(--fila-par)",
    color: "var(--texto)",
  },
});

// Mapa tag→clase. Las clases corresponden a los tokens del StreamLanguage
// de riscv.ts: comment, meta (directiva), labelName, keyword (mnemónico),
// string, number. Registros (variableName.special) y nombres sueltos quedan
// con el color de texto por defecto, igual que en el diseño.
const resaltadoSintaxis = HighlightStyle.define([
  { tag: t.comment, class: "cm-sx-com" },
  { tag: t.meta, class: "cm-sx-dir" },
  { tag: t.labelName, class: "cm-sx-lbl" },
  { tag: t.keyword, class: "cm-sx-ins" },
  { tag: t.string, class: "cm-sx-str" },
  { tag: t.number, class: "cm-sx-num" },
]);

/// Extensión a montar en el editor en lugar de oneDark.
export const temaEditor: Extension = [chromeEditor, syntaxHighlighting(resaltadoSintaxis)];
