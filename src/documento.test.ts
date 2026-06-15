// Tests de la parte pura del modelo de documento (RP30): derivar el nombre de
// pestaña sin DOM. La mecánica de swap (view.setState) se verifica a mano.
import assert from "node:assert/strict";
import { test } from "node:test";
import { indiceActivoTrasCerrar, nombreDocumento } from "./documento";

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

test("indiceActivoTrasCerrar: cerrar la activa activa su misma posición", () => {
  // [A* B C], cierro A(0) → queda [B C], activa 0 (B)
  assert.equal(indiceActivoTrasCerrar(0, 0, 3), 0);
  // [A B* C], cierro B(1) → queda [A C], activa 1 (C, que cayó a la pos 1)
  assert.equal(indiceActivoTrasCerrar(1, 1, 3), 1);
});

test("indiceActivoTrasCerrar: cerrar la activa cuando es la última activa la nueva última", () => {
  // [A B C*], cierro C(2) → queda [A B], activa 1 (B)
  assert.equal(indiceActivoTrasCerrar(2, 2, 3), 1);
  // [A B*], cierro B(1) → queda [A], activa 0
  assert.equal(indiceActivoTrasCerrar(1, 1, 2), 0);
});

test("indiceActivoTrasCerrar: cerrar a la izquierda corre la activa", () => {
  // [A B C*], cierro A(0) → la activa C pasa de 2 a 1
  assert.equal(indiceActivoTrasCerrar(2, 0, 3), 1);
});

test("indiceActivoTrasCerrar: cerrar a la derecha no mueve la activa", () => {
  // [A* B C], cierro C(2) → la activa A sigue en 0
  assert.equal(indiceActivoTrasCerrar(0, 2, 3), 0);
});
