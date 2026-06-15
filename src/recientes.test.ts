// Tests de la lista de recientes (RP30): parseo PURO tolerante + agregar
// (front, sin duplicados, acotado a MAX).
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  MAX_RECIENTES,
  agregarReciente,
  parsearRecientes,
  serializarRecientes,
} from "./recientes";

test("parsearRecientes tolera null, vacío, corrupto y formas no-array", () => {
  assert.deepEqual(parsearRecientes(null), []);
  assert.deepEqual(parsearRecientes(""), []);
  assert.deepEqual(parsearRecientes("{no es json"), []);
  assert.deepEqual(parsearRecientes("123"), []);
  assert.deepEqual(parsearRecientes('"texto"'), []);
  assert.deepEqual(parsearRecientes('{"a":1}'), []);
});

test("parsearRecientes filtra elementos no-string", () => {
  assert.deepEqual(parsearRecientes('["/a.s", 5, null, "/b.s", true]'), ["/a.s", "/b.s"]);
});

test("parsearRecientes acota a MAX_RECIENTES", () => {
  const muchas = Array.from({ length: MAX_RECIENTES + 5 }, (_, i) => `/f${i}.s`);
  assert.equal(parsearRecientes(JSON.stringify(muchas)).length, MAX_RECIENTES);
});

test("agregarReciente pone la ruta al frente", () => {
  assert.deepEqual(agregarReciente(["/a.s"], "/b.s"), ["/b.s", "/a.s"]);
});

test("agregarReciente no duplica: una ruta repetida sube al frente", () => {
  assert.deepEqual(agregarReciente(["/a.s", "/b.s", "/c.s"], "/b.s"), [
    "/b.s",
    "/a.s",
    "/c.s",
  ]);
});

test("agregarReciente acota a MAX_RECIENTES (descarta la más antigua)", () => {
  const llena = Array.from({ length: MAX_RECIENTES }, (_, i) => `/f${i}.s`);
  const r = agregarReciente(llena, "/nueva.s");
  assert.equal(r.length, MAX_RECIENTES);
  assert.equal(r[0], "/nueva.s");
  assert.equal(r.includes(`/f${MAX_RECIENTES - 1}.s`), false); // la más antigua se fue
});

test("agregarReciente no muta la lista de entrada", () => {
  const orig = ["/a.s"];
  agregarReciente(orig, "/b.s");
  assert.deepEqual(orig, ["/a.s"]);
});

test("serializarRecientes ↔ parsearRecientes es ida y vuelta", () => {
  const lista = ["/x/main.s", "/y/otro.asm"];
  assert.deepEqual(parsearRecientes(serializarRecientes(lista)), lista);
});
