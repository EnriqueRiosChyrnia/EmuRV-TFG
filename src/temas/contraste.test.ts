// Legibilidad innegociable (RP17-F3): el contraste WCAG de los pares
// críticos texto/fondo de CADA tema cumple AA (4.5:1). Lee los hex de
// tokens.css por bloque de tema — fuente única, sin duplicar valores.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { test } from "node:test";

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

function luminancia(hex: string): number {
  const c = hex.replace("#", "");
  const v = [0, 2, 4]
    .map((i) => parseInt(c.slice(i, i + 2), 16) / 255)
    .map((x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
}
function ratio(a: string, b: string): number {
  const la = luminancia(a);
  const lb = luminancia(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

// Pares críticos: texto/fondos, texto-suave, error, acento y ok como texto.
const PARES: Array<[string, string]> = [
  ["texto", "fondo"],
  ["texto", "fondo-panel"],
  ["texto", "fondo-claro"],
  ["texto-suave", "fondo-panel"],
  ["texto-suave", "fondo"],
  ["error", "fondo-panel"],
  ["error", "fondo"],
  ["acento", "fondo"],
  ["acento", "fondo-panel"],
  ["ok", "fondo"],
  ["cambio", "fondo-panel"],
];

// Sintaxis del editor (RP17B): cada --sx-* es texto que CodeMirror pinta
// sobre el fondo del editor, que es --fondo (ver styles.css/.cm-editor).
// La legibilidad del código es tan innegociable como la de la UI.
const SINTAXIS = ["sx-com", "sx-dir", "sx-lbl", "sx-ins", "sx-str", "sx-num"];

const AA = 4.5;

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
