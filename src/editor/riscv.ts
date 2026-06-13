// Modo de resaltado RV32IMAFDC para CodeMirror 6 (riesgo R3: no existe
// modo oficial, se define un StreamLanguage propio con regex). Las listas
// de tokens viven en isa.ts (fuente única anti-drift, RP14).
import { StreamLanguage } from "@codemirror/language";
import { esMnemonico, esRegistro } from "./isa";

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
    // identificador: mnemónico, registro o etiqueta. El patrón admite el
    // punto interno de las instrucciones con sufijo (fadd.s, amoadd.w.aqrl,
    // c.addi) — por eso un mnemónico se captura entero antes de buscarlo.
    const m = stream.match(/[A-Za-z_$][\w.$]*/);
    if (m) {
      const word = (m as RegExpMatchArray)[0];
      if (stream.peek() === ":") return "labelName";
      if (esRegistro(word)) return "variableName.special";
      if (esMnemonico(word)) return "keyword";
      return "name";
    }
    stream.next();
    return null;
  },
  languageData: {
    commentTokens: { line: "#" },
  },
});
