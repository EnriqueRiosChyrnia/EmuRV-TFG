// Tests de navegación de etiquetas (RP14-F3), funciones puras.
import assert from "node:assert/strict";
import { test } from "node:test";
import { definicionDeEtiqueta, palabraEn, usosDeEtiqueta } from "./etiquetas";

const DOC = "inicio:\n  add t0, t0, t1\n  j inicio\nfin:\n  beq t0, t1, fin\n";

test("definicionDeEtiqueta encuentra la definición y null si no existe", () => {
  const pos = definicionDeEtiqueta(DOC, "inicio");
  assert.equal(DOC.slice(pos!, pos! + 6), "inicio");
  assert.equal(definicionDeEtiqueta(DOC, "noexiste"), null);
});

test("definicionDeEtiqueta tolera indentación", () => {
  const doc = "   sangrada:\n  nop\n";
  assert.notEqual(definicionDeEtiqueta(doc, "sangrada"), null);
});

test("usosDeEtiqueta lista definición + usos", () => {
  const usos = usosDeEtiqueta(DOC, "inicio");
  assert.equal(usos.length, 2, "definición y el j inicio");
});

test("palabraEn devuelve el identificador en la posición", () => {
  const pos = DOC.indexOf("inicio", 10); // el uso en «j inicio»
  const p = palabraEn(DOC, pos + 2);
  assert.equal(p?.texto, "inicio");
});

test("palabraEn null si no hay identificador", () => {
  assert.equal(palabraEn("   \n", 1), null);
});
