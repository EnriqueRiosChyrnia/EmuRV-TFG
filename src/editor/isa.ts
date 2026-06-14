// Fuente ÚNICA de tokens del ISA para el frontend (RP14): el resaltado
// (riscv.ts) y el autocompletado consumen de acá. Cubre todo lo que el
// ensamblador acepta: RV32I + M + Zicsr + F + D + A + C, más pseudos.
//
// ANTI-DRIFT: `assembler::known_mnemonics()` (Rust) es la lista canónica;
// un test (src-tauri/tests/rp14_anti_drift.rs) lee ESTE archivo y exige
// que MNEMONICS ∪ PSEUDOS == known_mnemonics(). Si el ensamblador gana o
// pierde una instrucción y esta lista no se actualiza, el test falla.
// (No incluye fmv.x.d/fmv.d.x: el assembler las RECHAZA — no existen en
// RV32 — así que no son instrucciones válidas que resaltar.)

/// Instrucciones reales (no pseudo). Las atómicas se listan en su forma
/// base; los sufijos .aq/.rl/.aqrl los maneja `esMnemonico`.
export const MNEMONICS: string[] = [
  // RV32I
  "lui", "auipc", "jal", "jalr",
  "beq", "bne", "blt", "bge", "bltu", "bgeu",
  "lb", "lh", "lw", "lbu", "lhu", "sb", "sh", "sw",
  "addi", "slti", "sltiu", "xori", "ori", "andi", "slli", "srli", "srai",
  "add", "sub", "sll", "slt", "sltu", "xor", "srl", "sra", "or", "and",
  "fence", "ecall", "ebreak",
  // RV32M
  "mul", "mulh", "mulhsu", "mulhu", "div", "divu", "rem", "remu",
  // RV64IM (RP32) — solo válidas en programas RV64 (el ensamblador las
  // rechaza con error didáctico en RV32; el editor igual las resalta).
  "ld", "lwu", "sd", "addiw", "slliw", "srliw", "sraiw",
  "addw", "subw", "sllw", "srlw", "sraw",
  "mulw", "divw", "divuw", "remw", "remuw",
  // Zicsr (RP8)
  "csrrw", "csrrs", "csrrc", "csrrwi", "csrrsi", "csrrci", "mret",
  // RV32F (RP12)
  "flw", "fsw",
  "fadd.s", "fsub.s", "fmul.s", "fdiv.s", "fsqrt.s", "fmin.s", "fmax.s",
  "fmadd.s", "fmsub.s", "fnmadd.s", "fnmsub.s",
  "fcvt.w.s", "fcvt.wu.s", "fcvt.s.w", "fcvt.s.wu", "fmv.x.w", "fmv.w.x",
  "feq.s", "flt.s", "fle.s", "fsgnj.s", "fsgnjn.s", "fsgnjx.s", "fclass.s",
  // RV32D (RP12)
  "fld", "fsd",
  "fadd.d", "fsub.d", "fmul.d", "fdiv.d", "fsqrt.d", "fmin.d", "fmax.d",
  "fmadd.d", "fmsub.d", "fnmadd.d", "fnmsub.d",
  "fcvt.w.d", "fcvt.wu.d", "fcvt.d.w", "fcvt.d.wu", "fcvt.s.d", "fcvt.d.s",
  "feq.d", "flt.d", "fle.d", "fsgnj.d", "fsgnjn.d", "fsgnjx.d", "fclass.d",
  // RV32A (RP13) — forma base; sufijos .aq/.rl/.aqrl vía esMnemonico
  "lr.w", "sc.w", "amoswap.w", "amoadd.w", "amoand.w", "amoor.w", "amoxor.w",
  "amomin.w", "amomax.w", "amominu.w", "amomaxu.w",
  // RV32C (RP13)
  "c.nop", "c.addi", "c.li", "c.mv", "c.jr", "c.j", "c.lw", "c.sw",
  "c.beqz", "c.bnez",
];

/// Pseudo-instrucciones (se expanden a reales en el ensamblador).
export const PSEUDOS: string[] = [
  "nop", "li", "la", "lla", "mv", "not", "neg", "seqz", "snez", "sltz", "sgtz",
  "beqz", "bnez", "blez", "bgez", "bltz", "bgtz", "bgt", "ble", "bgtu", "bleu",
  "j", "jr", "ret", "call", "tail",
  "csrr", "csrw", "csrs", "csrc",
  "fmv.s", "fmv.d", "fabs.s", "fabs.d", "fneg.s", "fneg.d",
];

/// Registros enteros: nombres ABI + xN.
export const REGISTERS_X: string[] = [
  "zero", "ra", "sp", "gp", "tp", "fp",
  "t0", "t1", "t2", "t3", "t4", "t5", "t6",
  "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
  "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7",
  ...Array.from({ length: 32 }, (_, i) => `x${i}`),
];

/// Registros flotantes: nombres ABI + fN (RP12).
export const REGISTERS_F: string[] = [
  "ft0", "ft1", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7",
  "fs0", "fs1", "fs2", "fs3", "fs4", "fs5", "fs6", "fs7",
  "fs8", "fs9", "fs10", "fs11",
  "fa0", "fa1", "fa2", "fa3", "fa4", "fa5", "fa6", "fa7",
  ...Array.from({ length: 32 }, (_, i) => `f${i}`),
];

const MNEM_SET = new Set([...MNEMONICS, ...PSEUDOS]);
const REG_SET = new Set([...REGISTERS_X, ...REGISTERS_F]);
/// Sufijos de ordenamiento de las atómicas (RV32A). Se prueban del más
/// largo al más corto para que «.aqrl» no se confunda con «.aq».
const SUFIJOS_AMO = [".aqrl", ".aq", ".rl"];

/// ¿`palabra` es un mnemónico (real o pseudo), tolerando los sufijos
/// .aq/.rl/.aqrl de las atómicas? Case-insensitive como el ensamblador.
export function esMnemonico(palabra: string): boolean {
  const w = palabra.toLowerCase();
  if (MNEM_SET.has(w)) return true;
  for (const s of SUFIJOS_AMO) {
    if (w.endsWith(s) && MNEM_SET.has(w.slice(0, -s.length))) return true;
  }
  return false;
}

/// ¿`palabra` es un registro (entero o flotante)?
export function esRegistro(palabra: string): boolean {
  return REG_SET.has(palabra.toLowerCase());
}
