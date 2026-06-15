// Tests de parsearWorkspace (RP27): función pura, tolerante a entradas malas.
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
});

test("parsearWorkspace exige contenido string", () => {
  assert.equal(parsearWorkspace("{}"), null);
  assert.equal(parsearWorkspace('{"contenido":42}'), null);
  assert.equal(parsearWorkspace('{"rutaArchivo":"/a.s"}'), null);
});

test("parsearWorkspace lee contenido + rutaArchivo", () => {
  assert.deepEqual(parsearWorkspace('{"contenido":"li a0,1","rutaArchivo":"/x/main.s"}'), {
    contenido: "li a0,1",
    rutaArchivo: "/x/main.s",
  });
});

test("parsearWorkspace normaliza rutaArchivo ausente/no-string a null", () => {
  assert.deepEqual(parsearWorkspace('{"contenido":"x"}'), {
    contenido: "x",
    rutaArchivo: null,
  });
  assert.deepEqual(parsearWorkspace('{"contenido":"x","rutaArchivo":5}'), {
    contenido: "x",
    rutaArchivo: null,
  });
  assert.deepEqual(parsearWorkspace('{"contenido":"x","rutaArchivo":null}'), {
    contenido: "x",
    rutaArchivo: null,
  });
});

test("parsearWorkspace acepta contenido vacío (buffer en blanco)", () => {
  assert.deepEqual(parsearWorkspace('{"contenido":"","rutaArchivo":null}'), {
    contenido: "",
    rutaArchivo: null,
  });
});
