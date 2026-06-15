// Modelo de documento abierto (RP30): cada pestaña es UN documento con su
// buffer, ruta, estado limpio (RP25 «•» por pestaña), modo binario, programa
// ensamblado y su `EditorState` (con undo/cursor/selección/breakpoints/línea de
// ejecución/errores — todo viaja en el estado, ver BITACORA RP30-F1). El editor
// es UNA sola `EditorView` reutilizada; al cambiar de pestaña se intercambia el
// estado con `view.setState`. La parte pura (derivar el nombre) se testea sin
// DOM; la mecánica de swap vive en `main.ts` (acoplada a la vista).
import type { EditorState } from "@codemirror/state";
import type { AssembleOk } from "./api";

export interface Documento {
  /** Identidad estable de la pestaña (para la barra; no es la ruta). */
  readonly id: string;
  /** Ruta en disco (base de `.include`, RP5), o null si nunca se guardó. */
  rutaArchivo: string | null;
  /** Contenido en el último guardado/carga (RP23): base del «•». */
  docLimpio: string;
  /** Modo binario (RP11): listado desensamblado, solo lectura. */
  modoBinario: boolean;
  /** Último ensamblado exitoso de ESTE documento, o null. */
  programa: AssembleOk | null;
  /** El texto cambió desde el último ensamblado (line_to_addr obsoleto). */
  programaDesactualizado: boolean;
  /** Estado del editor del documento. Es la fuente de verdad MIENTRAS la
   *  pestaña NO es la activa; para la activa, lo es `view.state` (se vuelca acá
   *  al desactivarla). */
  estado: EditorState;
  /** Posición de scroll guardada (setState la resetea: se restaura a mano). */
  scrollTop: number;
}

let contador = 0;

/** Crea un documento alrededor de un `EditorState` ya construido. Por defecto el
 *  «limpio» es el contenido inicial del estado (buffer recién cargado). */
export function crearDocumento(
  estado: EditorState,
  opts?: { rutaArchivo?: string | null; docLimpio?: string },
): Documento {
  return {
    id: `doc-${++contador}`,
    rutaArchivo: opts?.rutaArchivo ?? null,
    docLimpio: opts?.docLimpio ?? estado.doc.toString(),
    modoBinario: false,
    programa: null,
    programaDesactualizado: false,
    estado,
    scrollTop: 0,
  };
}

/** Nombre visible de la pestaña: basename de la ruta, o «Sin título» si no hay
 *  ruta (reusa `titulo.sinTitulo` de RP25). PURO y testeable: separa por `/` y
 *  `\` para que ande igual en rutas POSIX y Windows. */
export function nombreDocumento(rutaArchivo: string | null, sinTitulo: string): string {
  if (!rutaArchivo) return sinTitulo;
  return rutaArchivo.split(/[/\\]/).pop() || rutaArchivo;
}

/** Índice activo resultante tras cerrar la pestaña `cerrado` (PURO, RP30-F3).
 *  `activa` es el índice activo PREVIO; `total` la cantidad de pestañas ANTES de
 *  cerrar (se asume > 1: nunca se cierra la última). Reglas:
 *  - si se cierra la activa: queda activa la de su misma posición, o la última
 *    si se cerró la última (acota a la nueva longitud `total - 1`);
 *  - si se cierra una a la izquierda: la activa se corre un lugar a la izquierda;
 *  - si se cierra una a la derecha: el índice activo no cambia. */
export function indiceActivoTrasCerrar(activa: number, cerrado: number, total: number): number {
  if (cerrado === activa) return Math.min(cerrado, total - 2); // total-2 = último índice nuevo
  if (cerrado < activa) return activa - 1;
  return activa;
}
