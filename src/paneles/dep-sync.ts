// Sincronización del panel de Depuración entre la ventana principal y la
// ventana desacoplada (RP22-F3). Los breakpoints se crean en el GUTTER del
// editor (principal); la LISTA puede vivir en la ventana → se sincronizan
// por eventos de app (Tauri emit/listen, broadcast entre webviews). Protocolo
// (un solo lugar para los nombres + payloads, así main y panel.ts no divergen):
//   - app://dep-filas       main → ventana: la lista de breakpoints + condición.
//   - app://dep-pedir       ventana → main: handshake al abrir (pedir el estado).
//   - app://dep-quitar      ventana → main: quitar el breakpoint de una línea.
//   - app://dep-condiciones ventana → main: condiciones para que main las absorba.
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Un breakpoint como viaja entre ventanas: línea + dirección + condición. */
export interface FilaConCond {
  linea: number;
  addr: number | null;
  cond: string;
}

export interface ParCond {
  linea: number;
  cond: string;
}

const EV_FILAS = "app://dep-filas";
const EV_PEDIR = "app://dep-pedir";
const EV_QUITAR = "app://dep-quitar";
const EV_CONDS = "app://dep-condiciones";

export const depSync = {
  // main → ventana
  emitFilas: (filas: FilaConCond[]): void => void emit(EV_FILAS, { filas }),
  onFilas: (cb: (filas: FilaConCond[]) => void): Promise<UnlistenFn> =>
    listen<{ filas: FilaConCond[] }>(EV_FILAS, (e) => cb(e.payload.filas)),

  // ventana → main (handshake)
  pedir: (): void => void emit(EV_PEDIR, {}),
  onPedir: (cb: () => void): Promise<UnlistenFn> => listen(EV_PEDIR, () => cb()),

  // ventana → main (quitar breakpoint)
  quitar: (linea: number): void => void emit(EV_QUITAR, { linea }),
  onQuitar: (cb: (linea: number) => void): Promise<UnlistenFn> =>
    listen<{ linea: number }>(EV_QUITAR, (e) => cb(e.payload.linea)),

  // ventana → main (condiciones, para absorberlas y no perderlas al reacoplar)
  emitConds: (conds: ParCond[]): void => void emit(EV_CONDS, { conds }),
  onConds: (cb: (conds: ParCond[]) => void): Promise<UnlistenFn> =>
    listen<{ conds: ParCond[] }>(EV_CONDS, (e) => cb(e.payload.conds)),
};
