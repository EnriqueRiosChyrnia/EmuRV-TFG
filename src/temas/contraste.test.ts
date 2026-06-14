// Legibilidad innegociable (RP17-F3): el contraste WCAG de los pares
// críticos texto/fondo de CADA tema cumple AA (4.5:1). Lee los hex de
// tokens.css por bloque de tema — fuente única, sin duplicar valores.
// La fórmula (luminancia/ratio) y la lista de pares viven en contraste.ts
// (RP28), compartidas con el aviso en vivo del editor de tema de usuario.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { test } from "node:test";
import { AA, PARES, SINTAXIS, ratio } from "./contraste";

const aqui = dirname(fileURLToPath(import.meta.url));
const css = readFileSync(resolve(aqui, "tokens.css"), "utf8");

/// Extrae los `--token: #hex;` del bloque cuyo selector contiene `sel`.
function tokensDe(sel: string): Record<string, string> {
  const i = css.indexOf(sel);
  assert.ok(i >= 0, `bloque ${sel} no encontrado`);
  const cuerpo = css.slice(css.indexOf("{", i) + 1, css.indexOf("}", i));
  const out: Record<string, string> = {};
  for (const m of cuerpo.matchAll(/--([\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) {
    out[m[1]] = m[2];
  }
  return out;
}

for (const [sel, nombre] of [
  ['[data-tema="educativo"]', "Educativo"],
  ['[data-tema="retro"]', "Retro-CRT"],
  ['[data-tema="pergamino"]', "Pergamino"],
  ['[data-tema="kawaii"]', "Retro Kawaii"],
  ['[data-tema="disquete"]', "Disquete 90s"],
] as const) {
  test(`contraste AA (≥${AA}) en los pares críticos del tema ${nombre}`, () => {
    const tk = tokensDe(sel);
    for (const [fg, bg] of PARES) {
      const r = ratio(tk[fg], tk[bg]);
      assert.ok(
        r >= AA,
        `${nombre}: ${fg}(${tk[fg]}) sobre ${bg}(${tk[bg]}) = ${r.toFixed(2)}:1 < ${AA}`,
      );
    }
  });

  test(`contraste AA (≥${AA}) de la sintaxis del editor en el tema ${nombre}`, () => {
    // Los temas que no redefinen --sx-* heredan los de :root; el test los
    // resuelve desde :root cuando el bloque del tema no los trae.
    const base = tokensDe('[data-tema="educativo"]');
    const tk = { ...base, ...tokensDe(sel) };
    for (const sx of SINTAXIS) {
      const r = ratio(tk[sx], tk["fondo"]);
      assert.ok(
        r >= AA,
        `${nombre}: ${sx}(${tk[sx]}) sobre fondo(${tk["fondo"]}) = ${r.toFixed(2)}:1 < ${AA}`,
      );
    }
  });
}
