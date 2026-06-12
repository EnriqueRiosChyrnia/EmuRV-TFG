// Puente tipado con el backend Tauri (comandos y eventos).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface AsmError {
  line: number;
  col: number;
  msg: string;
}

export interface AssembleOk {
  text_base: number;
  data_base: number;
  sp_init: number;
  text_len: number;
  data_len: number;
  symbols: Record<string, number>;
  line_to_addr: Record<string, number>;
  addr_to_line: Record<string, number>;
}

export interface MachineState {
  registers: number[];
  pc: number;
  line: number | null;
}

export type StopReason =
  | { kind: "exited"; code: number }
  | { kind: "breakpoint"; addr: number }
  | { kind: "paused" }
  | { kind: "step_done" }
  | { kind: "runtime_error"; message: string };

export interface StoppedEvent {
  reason: StopReason;
  state: MachineState;
}

export const api = {
  /** Ensambla; rechaza con AsmError[] si el fuente tiene errores. */
  assemble: (source: string) => invoke<AssembleOk>("assemble", { source }),
  run: () => invoke<void>("run_program"),
  step: () => invoke<void>("step"),
  reset: () => invoke<void>("reset"),
  stop: () => invoke<void>("stop"),
  setBreakpoints: (addrs: number[]) => invoke<void>("set_breakpoints", { addrs }),
  sendInput: (text: string) => invoke<void>("send_input", { text }),
  /** Encola una tecla en el dispositivo de teclado MMIO (TEC4). Disparar y
   *  olvidar: sin sesión de emulación no hay cola y la tecla se descarta. */
  pushKey: (code: number): void => {
    invoke<void>("push_key", { code }).catch(() => {
      // sin sesión de emulación: nada que hacer con la tecla
    });
  },
  exportElf: (path: string) => invoke<void>("export_elf", { path }),
  /** Lee un archivo de texto (.asm) del disco para cargarlo en el editor. */
  abrirArchivo: (path: string) => invoke<string>("leer_archivo_texto", { path }),
  /** Guarda el contenido del editor como archivo de texto (.asm). */
  guardarArchivo: (path: string, contenido: string) =>
    invoke<void>("guardar_archivo_texto", { path, contenido }),
  readMem: (addr: number, len: number) => invoke<number[]>("read_mem", { addr, len }),
  /** Frame RGBA crudo (ArrayBuffer) más nuevo que `since`; 0 bytes si la
   *  VRAM no cambió. `since` viaja como string: JSON no representa u64
   *  completo y Number perdería precisión sobre 2^53. */
  readFramebuffer: (since: bigint) =>
    invoke<ArrayBuffer>("read_framebuffer", { since: since.toString() }),
  getState: () => invoke<MachineState>("get_state"),
};

export const events = {
  onConsole: (cb: (text: string) => void): Promise<UnlistenFn> =>
    listen<{ text: string }>("emu://consola", (e) => cb(e.payload.text)),
  onInputRequest: (cb: () => void): Promise<UnlistenFn> =>
    listen("emu://pedir-entrada", () => cb()),
  onStopped: (cb: (ev: StoppedEvent) => void): Promise<UnlistenFn> =>
    listen<StoppedEvent>("emu://detenido", (e) => cb(e.payload)),
  onRegisters: (cb: (st: MachineState) => void): Promise<UnlistenFn> =>
    listen<MachineState>("emu://registros", (e) => cb(e.payload)),
  /** El programa empezó a usar la pantalla gráfica (VRAM o control). */
  onVideoActivo: (cb: () => void): Promise<UnlistenFn> =>
    listen("emu://video-activo", () => cb()),
};
