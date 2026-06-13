// Tests del scroll-spy puro (RP20).
import assert from "node:assert/strict";
import { test } from "node:test";
import { seccionActiva, type SeccionPos } from "./scroll-spy";

const SECS: SeccionPos[] = [
  { id: "a", top: 0 },
  { id: "b", top: 200 },
  { id: "c", top: 500 },
];

test("arriba del todo → primera sección", () => {
  assert.equal(seccionActiva(0, SECS), "a");
});

test("scrolleando past el top de b → b activa", () => {
  assert.equal(seccionActiva(210, SECS), "b");
});

test("justo antes de c → sigue b", () => {
  assert.equal(seccionActiva(480, SECS), "b");
});

test("past c → c activa (última)", () => {
  assert.equal(seccionActiva(900, SECS), "c");
});

test("el margen evita marcar tarde: top-margen ya cuenta", () => {
  // b.top=200, margen 12 → a scrollTop 190 ya cuenta como b.
  assert.equal(seccionActiva(190, SECS, 12), "b");
});

test("lista vacía → null", () => {
  assert.equal(seccionActiva(0, []), null);
});
