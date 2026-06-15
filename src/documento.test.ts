// Tests de la parte pura del modelo de documento (RP30): derivar el nombre de
// pestaña sin DOM. La mecánica de swap (view.setState) se verifica a mano.
import assert from "node:assert/strict";
import { test } from "node:test";
import { nombreDocumento } from "./documento";

test("nombreDocumento usa «Sin título» cuando no hay ruta", () => {
  assert.equal(nombreDocumento(null, "Sin título"), "Sin título");
});

test("nombreDocumento devuelve el basename de una ruta POSIX", () => {
  assert.equal(nombreDocumento("/Users/x/proyecto/main.s", "Sin título"), "main.s");
  assert.equal(nombreDocumento("/main.asm", "Sin título"), "main.asm");
});

test("nombreDocumento devuelve el basename de una ruta Windows", () => {
  assert.equal(nombreDocumento("C:\\Users\\x\\main.s", "Sin título"), "main.s");
});

test("nombreDocumento devuelve el nombre tal cual si no tiene separadores", () => {
  assert.equal(nombreDocumento("programa.asm", "Sin título"), "programa.asm");
});
