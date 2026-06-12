// Puente tipado con el backend Tauri (comandos y eventos).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface AsmError {
  /** Archivo de origen si el error está dentro de un .include (RP5). */
  file?: string | null;
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

/** Breakpoint como lo manda la UI (RP7): dirección + condición textual
 *  opcional (p. ej. "t0 == 5"). El parseo de la condición es del backend;
 *  un error didáctico vuelve como rechazo del invoke. */
export interface BpSpec {
  addr: number;
  cond: string | null;
}

/** Tipo de acceso vigilado por un watchpoint (RP7). */
export type WatchKind = "read" | "write" | "both";

/** Watchpoint sobre datos (RP7): vigila `len` bytes desde `addr`. */
export interface Watchpoint {
  addr: number;
  len: 1 | 2 | 4;
  kind: WatchKind;
}

export type StopReason =
  | { kind: "exited"; code: number }
  | { kind: "breakpoint"; addr: number }
  /** Un watchpoint disparó: `addr`/`value` son del ACCESO real; `pc` es la
   *  instrucción que accedió (la línea llega resuelta en `state.line`). */
  | { kind: "watchpoint"; addr: number; access: WatchKind; value: number; pc: number }
  | { kind: "ebreak_pause"; addr: number }
  | { kind: "paused" }
  | { kind: "step_done" }
  | { kind: "runtime_error"; message: string };

export interface StoppedEvent {
  reason: StopReason;
  state: MachineState;
}

export const api = {
  /** Ensambla; rechaza con AsmError[] si el fuente tiene errores. */
  assemble: (source: string, ruta: string | null = null) =>
    invoke<AssembleOk>("assemble", { source, ruta }),
  run: () => invoke<void>("run_program"),
  step: () => invoke<void>("step"),
  reset: () => invoke<void>("reset"),
  stop: () => invoke<void>("stop"),
  setBreakpoints: (bps: BpSpec[]) => invoke<void>("set_breakpoints", { bps }),
  /** Instala watchpoints (semántica reemplazar-todo, como los breakpoints).
   *  Un error didáctico de región vuelve como rechazo del invoke. */
  setWatchpoints: (watchpoints: Watchpoint[]) =>
    invoke<void>("set_watchpoints", { watchpoints }),
  /** Edición en vivo de un registro (RP7): `reg` = nombre ABI, xN o "pc".
   *  Solo con la máquina pausada; el rechazo trae el mensaje didáctico. */
  writeReg: (reg: string, val: number) => invoke<void>("write_reg", { reg, val: val >>> 0 }),
  /** Edición en vivo de memoria (RP7): 1, 2 o 4 bytes little-endian. */
  writeMem: (addr: number, bytes: number[]) =>
    invoke<void>("write_mem", { addr: addr >>> 0, bytes }),
  /** CSRs de máquina (RP8) en el orden fijo del backend:
   *  mstatus, mie, mtvec, mscratch, mepc, mcause, mtval, mip. */
  getCsrs: () => invoke<number[]>("get_csrs"),
  /** Edición en vivo de un CSR (RP8): `indice` según el orden de getCsrs().
   *  Solo con la máquina pausada; el rechazo trae el mensaje didáctico. */
  writeCsr: (indice: number, val: number) =>
    invoke<void>("write_csr", { indice, val: val >>> 0 }),
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
  /** Abre fuente o binario ELF con detección por magic (RP11). El caso
   *  binario YA deja la sesión creada (como un Ensamblar exitoso). */
  abrirPrograma: (path: string) => invoke<AperturaOk>("abrir_programa", { path }),
  /** Listado desensamblado del .text de la sesión: dirección → texto. */
  getDisasm: () => invoke<Record<string, string>>("get_disasm"),
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

/** Resultado de abrir_programa (RP11). */
export type AperturaOk =
  | { tipo: "fuente"; contenido: string }
  | { tipo: "binario"; ok: AssembleOk; listado: string };

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
  /** El programa pidió limpiar la consola (syscall 60, extensión EmuRV). */
  onConsoleClear: (cb: () => void): Promise<UnlistenFn> =>
    listen("emu://consola-limpiar", () => cb()),
};
