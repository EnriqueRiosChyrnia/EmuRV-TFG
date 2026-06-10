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
  readMem: (addr: number, len: number) => invoke<number[]>("read_mem", { addr, len }),
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
};
