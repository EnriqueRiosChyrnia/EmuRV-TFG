// Formato de valores de registro para el panel (RP32). Módulo PURO (sin DOM
// ni Tauri) para poder testearlo aislado. Los valores llegan del backend como
// hex de 16 dígitos (u64) — ver `MachineState.registers`. RV32 muestra los 8
// dígitos bajos y el decimal con signo de 32 bits; RV64 muestra los 16 dígitos
// y el decimal con signo de 64 bits (BigInt, sin pérdida de precisión).
import type { Xlen } from "../arquitectura";

/** Celda hex de un registro: `0x` + 8 dígitos (RV32) o 16 (RV64). `hex16`
 *  llega zero-padded a 16 desde el backend. */
export function hexReg(hex16: string, xlen: Xlen): string {
  const v = xlen === "rv64" ? hex16 : hex16.slice(-8);
  return `0x${v}`;
}

/** Celda decimal CON SIGNO de un registro: i32 (RV32) o i64 (RV64). El i64 se
 *  calcula con BigInt porque un valor > 2^53 no es exacto como `number`. */
export function decReg(hex16: string, xlen: Xlen): string {
  if (xlen === "rv64") {
    const u = BigInt(`0x${hex16}`);
    const i = u >= 1n << 63n ? u - (1n << 64n) : u; // complemento a dos de 64
    return i.toString();
  }
  const low = Number.parseInt(hex16.slice(-8), 16) >>> 0;
  return String(low | 0); // complemento a dos de 32
}
