// Señal main → ventanas desacopladas (fix post-RP22): el programa o el estado
// cambió POR FUERA de una ejecución —al Ensamblar o Reiniciar— y `emu://detenido`
// (que cubre las paradas de ejecución) NO se emite en esos casos. Sin esta señal,
// una ventana desacoplada (p. ej. Memoria) se queda con el programa viejo o vacío
// hasta que se reacopla. Mismo transporte que dep-sync: Tauri emit/listen, que
// hace broadcast entre todas las webviews del mismo proceso.
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";

const EV_RESYNC = "app://panel-resync";

export const panelSync = {
  /** main: avisá que el programa/estado cambió (Ensamblar, Reiniciar). */
  notificar: (): void => void emit(EV_RESYNC),
  /** ventana desacoplada: re-sincronizate al recibir el aviso. */
  onResync: (cb: () => void): Promise<UnlistenFn> => listen(EV_RESYNC, () => cb()),
};
