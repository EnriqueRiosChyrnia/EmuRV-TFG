// Editor de código (RF-01, RF-02, RF-04): CodeMirror 6 con números de
// línea, resaltado RV32, breakpoints en el margen, línea actual de
// ejecución y errores de ensamblado anclados a línea/columna.
import { autocompletion, completionKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { setDiagnostics, type Diagnostic } from "@codemirror/lint";
import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, RangeSet, StateEffect, StateField } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  GutterMarker,
  ViewPlugin,
  gutter,
  highlightActiveLine,
  highlightActiveLineGutter,
  hoverTooltip,
  keymap,
  lineNumbers,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import type { AsmError } from "../api";
import { fuenteAutocompletado } from "./autocompletar";
import { ayudaDe, descripcionInstr } from "./ayuda-isa";
import { frasesBusqueda } from "./buscar";
import { definicionDeEtiqueta, palabraEn, usosDeEtiqueta } from "./etiquetas";
import { esMnemonico } from "./isa";
import { riscvLanguage } from "./riscv";
import { temaEditor } from "./tema";

// ---------------------------------------------------------------------------
// Breakpoints (RF-04): toggle por click en el margen.
// ---------------------------------------------------------------------------

const toggleBreakpoint = StateEffect.define<number>(); // pos del comienzo de línea

class BpMarker extends GutterMarker {
  override toDOM(): Node {
    const el = document.createElement("div");
    el.className = "cm-breakpoint-marker";
    el.textContent = "●";
    return el;
  }
}
const bpMarker = new BpMarker();

const breakpointField = StateField.define<RangeSet<GutterMarker>>({
  create: () => RangeSet.empty,
  update(set, tr) {
    set = set.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(toggleBreakpoint)) {
        let existe = false;
        set.between(e.value, e.value, () => {
          existe = true;
        });
        set = existe
          ? set.update({ filter: (from) => from !== e.value })
          : set.update({ add: [bpMarker.range(e.value)] });
      }
    }
    return set;
  },
});

/** Alterna el breakpoint de una línea 1-based (uso programático/tests). */
export function toggleBreakpointAtLine(view: EditorView, line: number): void {
  if (line < 1 || line > view.state.doc.lines) return;
  view.dispatch({ effects: toggleBreakpoint.of(view.state.doc.line(line).from) });
}

/** Líneas (1-based) que tienen breakpoint. */
export function getBreakpointLines(view: EditorView): number[] {
  const lineas: number[] = [];
  const iter = view.state.field(breakpointField).iter();
  while (iter.value !== null) {
    lineas.push(view.state.doc.lineAt(iter.from).number);
    iter.next();
  }
  return lineas;
}

// ---------------------------------------------------------------------------
// Línea actual de ejecución (RF-04): resaltado durante paso/breakpoint.
// ---------------------------------------------------------------------------

const setExecLineEffect = StateEffect.define<number | null>(); // línea 1-based

const execLineDeco = Decoration.line({ class: "cm-linea-actual" });

const execLineField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    deco = deco.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(setExecLineEffect)) {
        if (e.value === null || e.value < 1 || e.value > tr.state.doc.lines) {
          deco = Decoration.none;
        } else {
          deco = Decoration.set([execLineDeco.range(tr.state.doc.line(e.value).from)]);
        }
      }
    }
    return deco;
  },
  provide: (f) => EditorView.decorations.from(f),
});

/** Marca (o limpia, con null) la línea que está por ejecutarse. */
export function setExecLine(view: EditorView, line: number | null): void {
  const effects: StateEffect<unknown>[] = [setExecLineEffect.of(line)];
  if (line !== null && line >= 1 && line <= view.state.doc.lines) {
    const pos = view.state.doc.line(line).from;
    effects.push(EditorView.scrollIntoView(pos, { y: "nearest" }));
  }
  view.dispatch({ effects });
}

// ---------------------------------------------------------------------------
// Errores de ensamblado anclados (RF-02).
// ---------------------------------------------------------------------------

/** Ancla los errores del ensamblador como diagnósticos del editor. */
export function setErrors(view: EditorView, errores: AsmError[]): void {
  const doc = view.state.doc;
  const diags: Diagnostic[] = errores
    .filter((e) => e.line >= 1 && e.line <= doc.lines)
    .map((e) => {
      const linea = doc.line(e.line);
      const desde = Math.min(linea.from + Math.max(e.col - 1, 0), linea.to);
      return {
        from: desde,
        to: linea.to > desde ? linea.to : desde,
        severity: "error" as const,
        message: e.msg,
      };
    });
  view.dispatch(setDiagnostics(view.state, diags));
}

// ---------------------------------------------------------------------------
// Hover de ayuda por instrucción (RP14-F3)
// ---------------------------------------------------------------------------

/// Tooltip al pasar sobre un mnemónico: muestra sintaxis + descripción del
/// diccionario. Un mnemónico sin entrada (o cualquier otra palabra) NO
/// produce tooltip — nunca uno vacío.
const hoverAyuda = hoverTooltip((view, pos) => {
  const doc = view.state.doc.toString();
  const palabra = palabraEn(doc, pos);
  if (!palabra || !esMnemonico(palabra.texto)) return null;
  const ayuda = ayudaDe(palabra.texto);
  if (!ayuda) return null;
  return {
    pos: palabra.from,
    end: palabra.to,
    above: true,
    create() {
      const dom = document.createElement("div");
      dom.className = "cm-ayuda-tooltip";
      const sint = document.createElement("div");
      sint.className = "cm-ayuda-sintaxis";
      sint.textContent = ayuda.sintaxis;
      const desc = document.createElement("div");
      desc.textContent = ayuda.descripcion;
      dom.append(sint, desc);
      return { dom };
    },
  };
});

// ---------------------------------------------------------------------------
// Resaltar usos de la etiqueta bajo el cursor (RP14-F3)
// ---------------------------------------------------------------------------

const marcaUsoEtiqueta = Decoration.mark({ class: "cm-uso-etiqueta" });

const resaltarUsosEtiqueta = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = this.calcular(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.selectionSet) {
        this.decorations = this.calcular(u.view);
      }
    }
    calcular(view: EditorView): DecorationSet {
      const sel = view.state.selection.main;
      const doc = view.state.doc.toString();
      const palabra = palabraEn(doc, sel.head);
      // Solo resaltar si el cursor está sobre una etiqueta REAL (tiene
      // definición) y no sobre un mnemónico/registro.
      if (!palabra || esMnemonico(palabra.texto)) return Decoration.none;
      if (definicionDeEtiqueta(doc, palabra.texto) === null) return Decoration.none;
      const usos = usosDeEtiqueta(doc, palabra.texto);
      if (usos.length < 2) return Decoration.none; // sin usos además del cursor
      return Decoration.set(usos.map((u) => marcaUsoEtiqueta.range(u.from, u.to)));
    }
  },
  { decorations: (v) => v.decorations },
);

// ---------------------------------------------------------------------------
// Ir-a-definición de etiqueta: Cmd/Ctrl-click (RP14-F3)
// ---------------------------------------------------------------------------

const irADefinicionEtiqueta = EditorView.domEventHandlers({
  mousedown(event, view) {
    if (!event.metaKey && !event.ctrlKey) return false;
    const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (pos === null) return false;
    const doc = view.state.doc.toString();
    const palabra = palabraEn(doc, pos);
    if (!palabra || esMnemonico(palabra.texto)) return false;
    const def = definicionDeEtiqueta(doc, palabra.texto);
    if (def === null) return false; // sin definición: no romper, dejar pasar
    event.preventDefault();
    view.dispatch({ selection: { anchor: def }, scrollIntoView: true });
    view.focus();
    return true;
  },
});

// ---------------------------------------------------------------------------
// Construcción del editor
// ---------------------------------------------------------------------------

export function createEditor(
  parent: HTMLElement,
  doc: string,
  onBreakpointsChanged: (lineas: number[]) => void,
  puedeAgregarBreakpoint: (linea: number) => boolean = () => true,
  onDocChanged?: () => void,
): EditorView {
  const bpGutter = gutter({
    class: "cm-breakpoint-gutter",
    markers: (v) => v.state.field(breakpointField),
    initialSpacer: () => bpMarker,
    domEventHandlers: {
      mousedown(view, lineaBloque) {
        // Quitar un punto existente se permite siempre; agregar uno nuevo
        // se consulta con la app, que rechaza líneas sin código ejecutable
        // (evita puntos rojos que no corresponden a un breakpoint real).
        let existe = false;
        view.state.field(breakpointField).between(lineaBloque.from, lineaBloque.from, () => {
          existe = true;
        });
        if (!existe) {
          const linea = view.state.doc.lineAt(lineaBloque.from).number;
          if (!puedeAgregarBreakpoint(linea)) return true;
        }
        view.dispatch({ effects: toggleBreakpoint.of(lineaBloque.from) });
        onBreakpointsChanged(getBreakpointLines(view));
        return true;
      },
    },
  });

  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc,
      extensions: [
        readOnlyComp.of([EditorState.readOnly.of(false), EditorView.editable.of(true)]),
        breakpointField,
        bpGutter,
        lineNumbers(),
        execLineField,
        highlightActiveLine(),
        highlightActiveLineGutter(),
        history(),
        keymap.of([...defaultKeymap, ...completionKeymap, ...historyKeymap, indentWithTab]),
        // Búsqueda/reemplazo (RP-24): Cmd/Ctrl+F abre el panel arriba, con
        // resaltado de las coincidencias de la selección y etiquetas en el
        // idioma activo. En modo binario (read-only) CodeMirror oculta los
        // controles de reemplazo, así que el panel solo busca.
        keymap.of(searchKeymap),
        search({ top: true }),
        highlightSelectionMatches(),
        EditorState.phrases.of(frasesBusqueda()),
        riscvLanguage,
        // Autocompletado contextual (RP14): mnemónicos, registros y
        // etiquetas del buffer, con descripción del diccionario en el info.
        autocompletion({ override: [(ctx) => fuenteAutocompletado(ctx, descripcionInstr)] }),
        // Hover de ayuda, resaltado de usos e ir-a-definición (RP14-F3).
        hoverAyuda,
        resaltarUsosEtiqueta,
        irADefinicionEtiqueta,
        // Tema del editor por tokens (RP17B-F2): chrome + sintaxis siguen el
        // tema activo en vivo. Reemplaza al tema fijo que había antes.
        temaEditor,
        // Al editar: limpiar diagnósticos viejos y reportar breakpoints
        // (sus posiciones se mueven con el texto).
        // El setTimeout(0) es necesario: CodeMirror prohíbe despachar una
        // transacción dentro del updateListener (el update sigue en curso)
        // y setErrors hace view.dispatch; se difiere hasta que termine.
        EditorView.updateListener.of((u) => {
          if (u.docChanged) {
            // Aviso sincrónico a la app: el texto cambió, así que el
            // programa ensamblado (si lo hay) quedó desactualizado.
            onDocChanged?.();
            setTimeout(() => {
              setErrors(view, []);
              onBreakpointsChanged(getBreakpointLines(view));
            }, 0);
          }
        }),
      ],
    }),
  });
  return view;
}

/** Modo binario (RP11): el listado desensamblado es solo-lectura. */
const readOnlyComp = new Compartment();

export function setReadOnly(view: EditorView, ro: boolean): void {
  view.dispatch({
    effects: readOnlyComp.reconfigure([
      EditorState.readOnly.of(ro),
      EditorView.editable.of(!ro),
    ]),
  });
}

/** Reemplaza todo el contenido (carga de ejemplos). */
export function setDoc(view: EditorView, doc: string): void {
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: doc } });
}

export function getDoc(view: EditorView): string {
  return view.state.doc.toString();
}
