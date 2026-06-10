// Modo de resaltado RV32IM para CodeMirror 6 (riesgo R3: no existe modo
// oficial, se define un StreamLanguage propio con regex).
import { StreamLanguage } from "@codemirror/language";

const MNEMONICS = new Set([
  // RV32I
  "lui", "auipc", "jal", "jalr",
  "beq", "bne", "blt", "bge", "bltu", "bgeu",
  "lb", "lh", "lw", "lbu", "lhu", "sb", "sh", "sw",
  "addi", "slti", "sltiu", "xori", "ori", "andi", "slli", "srli", "srai",
  "add", "sub", "sll", "slt", "sltu", "xor", "srl", "sra", "or", "and",
  "fence", "ecall", "ebreak",
  // RV32M
  "mul", "mulh", "mulhsu", "mulhu", "div", "divu", "rem", "remu",
  // pseudo-instrucciones
  "nop", "li", "la", "lla", "mv", "not", "neg", "seqz", "snez", "sltz", "sgtz",
  "beqz", "bnez", "blez", "bgez", "bltz", "bgtz",
  "bgt", "ble", "bgtu", "bleu",
  "j", "jr", "ret", "call", "tail",
]);

const REGISTERS = new Set([
  "zero", "ra", "sp", "gp", "tp", "fp",
  ...Array.from({ length: 32 }, (_, i) => `x${i}`),
  ...Array.from({ length: 7 }, (_, i) => `t${i}`),
  ...Array.from({ length: 12 }, (_, i) => `s${i}`),
  ...Array.from({ length: 8 }, (_, i) => `a${i}`),
]);

export const riscvLanguage = StreamLanguage.define({
  name: "riscv",
  token(stream) {
    if (stream.eatSpace()) return null;
    // comentario hasta fin de línea
    if (stream.match("#")) {
      stream.skipToEnd();
      return "comment";
    }
    // cadena
    if (stream.match(/"(?:[^"\\]|\\.)*"?/)) return "string";
    // carácter
    if (stream.match(/'(?:[^'\\]|\\.)'/)) return "string";
    // directiva
    if (stream.match(/\.[a-zA-Z]+/)) return "meta";
    // número (hex/bin/octal/decimal, con signo)
    if (stream.match(/[+-]?0[xX][0-9a-fA-F_]+/)) return "number";
    if (stream.match(/[+-]?0[bB][01_]+/)) return "number";
    if (stream.match(/[+-]?\d+/)) return "number";
    // identificador: mnemónico, registro o etiqueta
    const m = stream.match(/[A-Za-z_$][\w.$]*/);
    if (m) {
      const word = (m as RegExpMatchArray)[0];
      if (stream.peek() === ":") return "labelName";
      if (REGISTERS.has(word)) return "variableName.special";
      if (MNEMONICS.has(word.toLowerCase())) return "keyword";
      return "name";
    }
    stream.next();
    return null;
  },
  languageData: {
    commentTokens: { line: "#" },
  },
});
