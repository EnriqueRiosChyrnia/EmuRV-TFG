// Tests de las funciones PURAS del dispositivo de sonido (RP31): el mapeo
// wave→tipo de oscilador y el plan de envolvente anti-click. No tocan
// AudioContext (no existe en node): solo la lógica testeable.
import assert from "node:assert/strict";
import { test } from "node:test";
import { waveType, planEnvolvente } from "./audio";

test("waveType mapea los 4 registros de forma de onda", () => {
  assert.equal(waveType(0), "square");
  assert.equal(waveType(1), "sine");
  assert.equal(waveType(2), "triangle");
  assert.equal(waveType(3), "sawtooth");
});

test("waveType cae en cuadrada ante valores fuera de rango", () => {
  assert.equal(waveType(4), "square");
  assert.equal(waveType(255), "square");
  assert.equal(waveType(-1), "square");
});

test("planEnvolvente: tono normal usa ataque y liberación fijos", () => {
  const p = planEnvolvente(200); // 200 ms
  assert.equal(p.total, 0.2);
  assert.equal(p.ataque, 0.005);
  assert.equal(p.liberacion, 0.005);
  assert.ok(p.pico > 0 && p.pico <= 1, "el pico es una ganancia válida");
});

test("planEnvolvente: tono muy corto encoge las rampas a la mitad", () => {
  // 6 ms: la mitad (3 ms) es menor que los 5 ms fijos, así que ataque y
  // liberación se recortan para no solaparse (anti-click).
  const p = planEnvolvente(6);
  assert.equal(p.total, 0.006);
  assert.equal(p.ataque, 0.003);
  assert.equal(p.liberacion, 0.003);
  assert.ok(p.ataque + p.liberacion <= p.total, "las rampas no exceden la duración");
});

test("planEnvolvente: duración no positiva da un tono vacío (no suena)", () => {
  assert.equal(planEnvolvente(0).total, 0);
  assert.equal(planEnvolvente(-100).total, 0);
});
