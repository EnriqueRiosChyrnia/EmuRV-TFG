// Tests del provider de autocompletado (RP14-F2) con el runner de node
// (node:test vía tsx). La lógica pura no necesita DOM ni CodeMirror.
import assert from "node:assert/strict";
import { test } from "node:test";
import { etiquetasDelBuffer, opcionesAutocompletado } from "./autocompletar";

const textos = (r: ReturnType<typeof opcionesAutocompletado>) =>
  (r?.opciones ?? []).map((o) => o.texto);

test("inicio de línea sugiere mnemónicos (incluido el repertorio nuevo)", () => {
  const r = opcionesAutocompletado("  ", "");
  const t = textos(r);
  assert.ok(t.includes("add"));
  assert.ok(t.includes("fadd.s"), "F debe estar");
  assert.ok(t.includes("amoadd.w"), "A debe estar");
  assert.ok(t.includes("c.addi"), "C debe estar");
  assert.ok(t.includes("li"), "pseudo debe estar");
});

test("prefijo filtra los mnemónicos", () => {
  const r = opcionesAutocompletado("  fad", "");
  const t = textos(r);
  assert.deepEqual(t, ["fadd.s", "fadd.d"]);
});

test("tras un mnemónico aritmético sugiere registros (x y f)", () => {
  const r = opcionesAutocompletado("add ", "");
  const t = textos(r);
  assert.ok(t.includes("t0"));
  assert.ok(t.includes("a0"));
  assert.ok(t.includes("ft0"), "los f también se ofrecen");
  // no es inicio de línea: no debe ofrecer mnemónicos
  assert.ok(!t.includes("add"));
});

test("tras j sugiere las etiquetas del buffer", () => {
  const doc = "inicio:\n  add t0, t0, t1\nfin:\n  j ";
  const r = opcionesAutocompletado("  j ", doc);
  const t = textos(r);
  assert.ok(t.includes("inicio"));
  assert.ok(t.includes("fin"));
});

test("beq ofrece registros y, en el último operando, etiquetas", () => {
  const doc = "destino:\n  beq t0, t1, ";
  const r = opcionesAutocompletado("  beq t0, t1, ", doc);
  const t = textos(r);
  assert.ok(t.includes("destino"), "etiqueta disponible");
  assert.ok(t.includes("t0"), "registros también");
});

test("un add NO ofrece etiquetas (no es op de salto)", () => {
  const doc = "destino:\n  add t0, t1, ";
  const r = opcionesAutocompletado("  add t0, t1, ", doc);
  const t = textos(r);
  assert.ok(!t.includes("destino"));
});

test("etiquetasDelBuffer tolera indentación y descarta duplicados", () => {
  const doc = "a:\n   b:\n  add\na:\n";
  assert.deepEqual(etiquetasDelBuffer(doc).sort(), ["a", "b"]);
});

test("prefijo de registro filtra", () => {
  const r = opcionesAutocompletado("add f", "");
  const t = textos(r);
  assert.ok(t.every((x) => x.toLowerCase().startsWith("f")));
  assert.ok(t.includes("fa0"));
  assert.ok(t.includes("fp"), "fp (entero) también empieza con f");
});
