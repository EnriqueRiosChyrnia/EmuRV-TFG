// Tests de progreso y navegación del currículo (RP15-F3), funciones puras
// + wrappers de localStorage con un stub mínimo.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  alternarCompletada,
  guardarProgreso,
  leerProgreso,
  parsearProgreso,
  resetearProgreso,
  serializarProgreso,
  vecina,
} from "./progreso";

test("parsearProgreso tolera null, basura y JSON no-array", () => {
  assert.equal(parsearProgreso(null).size, 0);
  assert.equal(parsearProgreso("no es json").size, 0);
  assert.equal(parsearProgreso('{"a":1}').size, 0);
  assert.deepEqual([...parsearProgreso("[1,3,2]")].sort(), [1, 2, 3]);
  // descarta no-números
  assert.deepEqual([...parsearProgreso('[1,"x",2]')].sort(), [1, 2]);
});

test("serializar es estable y ordenado (round-trip)", () => {
  const p = new Set([3, 1, 2]);
  assert.equal(serializarProgreso(p), "[1,2,3]");
  assert.deepEqual([...parsearProgreso(serializarProgreso(p))].sort(), [1, 2, 3]);
});

test("alternarCompletada agrega y quita sin mutar el original", () => {
  const p = new Set<number>([1]);
  const con2 = alternarCompletada(p, 2);
  assert.deepEqual([...con2].sort(), [1, 2]);
  assert.deepEqual([...p], [1], "no muta el original");
  const sin1 = alternarCompletada(con2, 1);
  assert.deepEqual([...sin1].sort(), [2]);
});

test("vecina respeta el orden de currículo y los bordes", () => {
  assert.equal(vecina(5, +1, 10), 6);
  assert.equal(vecina(5, -1, 10), 4);
  assert.equal(vecina(1, -1, 10), null, "no hay anterior a la 1");
  assert.equal(vecina(10, +1, 10), null, "no hay siguiente a la 10");
});

test("leer/guardar/reset usan localStorage (con stub)", () => {
  const mem = new Map<string, string>();
  (globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (k: string) => mem.get(k) ?? null,
    setItem: (k: string, v: string) => void mem.set(k, v),
    removeItem: (k: string) => void mem.delete(k),
    clear: () => mem.clear(),
    key: () => null,
    length: 0,
  } as Storage;

  assert.equal(leerProgreso().size, 0);
  guardarProgreso(new Set([2, 5]));
  assert.deepEqual([...leerProgreso()].sort(), [2, 5]);
  resetearProgreso();
  assert.equal(leerProgreso().size, 0);
});
