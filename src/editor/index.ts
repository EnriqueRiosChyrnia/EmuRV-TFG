// Editor de código (RF-01): CodeMirror 6 con números de línea y
// resaltado RV32. Breakpoints y resaltado de línea actual se agregan
// en la fase F4 sobre esta misma base.
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { EditorState } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import {
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { riscvLanguage } from "./riscv";

export function createEditor(parent: HTMLElement, doc: string): EditorView {
  return new EditorView({
    parent,
    state: makeState(doc),
  });
}

function makeState(doc: string): EditorState {
  return EditorState.create({
    doc,
    extensions: [
      lineNumbers(),
      highlightActiveLine(),
      highlightActiveLineGutter(),
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
      riscvLanguage,
      oneDark,
    ],
  });
}

/** Reemplaza todo el contenido (carga de ejemplos). */
export function setDoc(view: EditorView, doc: string): void {
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: doc } });
}

export function getDoc(view: EditorView): string {
  return view.state.doc.toString();
}
