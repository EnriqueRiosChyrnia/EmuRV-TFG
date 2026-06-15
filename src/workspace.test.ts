// Tests de parsearWorkspace (RP27 → RP30 v2): función pura, tolerante, que
// devuelve SIEMPRE la forma v2 {pestanas, activa} y MIGRA la forma v1.
import assert from "node:assert/strict";
import { test } from "node:test";
import { parsearWorkspace } from "./workspace";

test("parsearWorkspace devuelve null ante null, vacío y JSON corrupto", () => {
  assert.equal(parsearWorkspace(null), null);
  assert.equal(parsearWorkspace(""), null);
  assert.equal(parsearWorkspace("{no es json"), null);
  assert.equal(parsearWorkspace("123"), null);
  assert.equal(parsearWorkspace('"texto"'), null);
  assert.equal(parsearWorkspace("null"), null);
  assert.equal(parsearWorkspace("{}"), null); // ni pestanas ni contenido
});

// --- v1 → v2 (migración) ---

test("parsearWorkspace MIGRA v1 {contenido, rutaArchivo} a una pestaña", () => {
  assert.deepEqual(parsearWorkspace('{"contenido":"li a0,1","rutaArchivo":"/x/main.s"}'), {
    pestanas: [{ contenido: "li a0,1", rutaArchivo: "/x/main.s" }],
    activa: 0,
  });
});

test("parsearWorkspace v1: rutaArchivo ausente/no-string → null; contenido vacío OK", () => {
  assert.deepEqual(parsearWorkspace('{"contenido":"x"}'), {
    pestanas: [{ contenido: "x", rutaArchivo: null }],
    activa: 0,
  });
  assert.deepEqual(parsearWorkspace('{"contenido":"x","rutaArchivo":5}'), {
    pestanas: [{ contenido: "x", rutaArchivo: null }],
    activa: 0,
  });
  assert.deepEqual(parsearWorkspace('{"contenido":""}'), {
    pestanas: [{ contenido: "", rutaArchivo: null }],
    activa: 0,
  });
});

test("parsearWorkspace v1 exige contenido string", () => {
  assert.equal(parsearWorkspace('{"contenido":42}'), null);
  assert.equal(parsearWorkspace('{"rutaArchivo":"/a.s"}'), null);
});

// --- v2 (lectura directa) ---

test("parsearWorkspace lee v2 {pestanas, activa}", () => {
  const crudo = JSON.stringify({
    pestanas: [
      { contenido: "a", rutaArchivo: "/a.s" },
      { contenido: "b", rutaArchivo: null },
    ],
    activa: 1,
  });
  assert.deepEqual(parsearWorkspace(crudo), {
    pestanas: [
      { contenido: "a", rutaArchivo: "/a.s" },
      { contenido: "b", rutaArchivo: null },
    ],
    activa: 1,
  });
});

test("parsearWorkspace v2 sanea cada pestaña y descarta las inválidas", () => {
  const crudo = JSON.stringify({
    pestanas: [{ contenido: "ok", rutaArchivo: "/a.s" }, { rutaArchivo: "/sin-contenido" }, 7],
    activa: 0,
  });
  assert.deepEqual(parsearWorkspace(crudo), {
    pestanas: [{ contenido: "ok", rutaArchivo: "/a.s" }],
    activa: 0,
  });
});

test("parsearWorkspace v2 con pestanas vacías o todas inválidas → null", () => {
  assert.equal(parsearWorkspace('{"pestanas":[],"activa":0}'), null);
  assert.equal(parsearWorkspace('{"pestanas":[7,{"x":1}],"activa":0}'), null);
});

test("parsearWorkspace v2 acota activa al rango [0, n-1] (default 0)", () => {
  const dos = '{"pestanas":[{"contenido":"a"},{"contenido":"b"}],';
  assert.equal(parsearWorkspace(`${dos}"activa":9}`)?.activa, 1); // fuera de rango → último
  assert.equal(parsearWorkspace(`${dos}"activa":-3}`)?.activa, 0); // negativa → 0
  assert.equal(parsearWorkspace(`${dos}"activa":"x"}`)?.activa, 0); // no-número → 0
  assert.equal(parsearWorkspace(`${dos}"activa":1.9}`)?.activa, 1); // se trunca
});
