// Anti-drift del highlight por clases (RP17B-F2). El editor recolorea con
// el tema SOLO si las clases que emite el HighlightStyle (tema.ts) tienen su
// regla `color: var(--sx-*)` en styles.css. Un typo en cualquiera de los dos
// lados deja la sintaxis sin color sin romper nada — este test lo caza.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { test } from "node:test";

const aqui = dirname(fileURLToPath(import.meta.url));
const tema = readFileSync(resolve(aqui, "tema.ts"), "utf8");
const css = readFileSync(resolve(aqui, "../styles.css"), "utf8");

// Clases que el HighlightStyle declara: `class: "cm-sx-xxx"`.
const clasesEnTema = [...tema.matchAll(/class:\s*"(cm-sx-[a-z]+)"/g)].map((m) => m[1]);

test("el HighlightStyle declara las 6 clases de sintaxis", () => {
  assert.deepEqual(
    [...new Set(clasesEnTema)].sort(),
    ["cm-sx-com", "cm-sx-dir", "cm-sx-ins", "cm-sx-lbl", "cm-sx-num", "cm-sx-str"],
  );
});

test("cada clase del editor tiene su regla color:var(--sx-*) en styles.css", () => {
  for (const clase of clasesEnTema) {
    const sx = clase.replace("cm-", ""); // cm-sx-com → sx-com
    const re = new RegExp(`\\.${clase}\\s*\\{[^}]*color:\\s*var\\(--${sx}\\)`);
    assert.ok(re.test(css), `falta en styles.css: .${clase} { color: var(--${sx}) }`);
  }
});

test("el editor ya no depende de oneDark (highlight sigue al tema)", () => {
  const index = readFileSync(resolve(aqui, "index.ts"), "utf8");
  assert.ok(!/theme-one-dark|oneDark/.test(index), "index.ts no debe usar oneDark");
  assert.ok(/temaEditor/.test(index), "index.ts debe montar temaEditor");
});
