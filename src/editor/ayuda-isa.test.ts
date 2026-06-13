// Tests del diccionario de hover (RP14-F3): cobertura 100% + sufijos A.
import assert from "node:assert/strict";
import { test } from "node:test";
import { ayudaDe, AYUDA_ISA, descripcionInstr } from "./ayuda-isa";
import { MNEMONICS, PSEUDOS } from "./isa";

test("cobertura 100%: cada mnemónico de isa.ts tiene entrada de ayuda", () => {
  const faltan = [...MNEMONICS, ...PSEUDOS].filter((m) => !AYUDA_ISA[m]);
  assert.deepEqual(faltan, [], `sin ayuda: ${faltan.join(", ")}`);
});

test("el diccionario no inventa entradas fuera del ISA", () => {
  const validos = new Set([...MNEMONICS, ...PSEUDOS]);
  const sobran = Object.keys(AYUDA_ISA).filter((k) => !validos.has(k));
  assert.deepEqual(sobran, [], `entradas de más: ${sobran.join(", ")}`);
});

test("ayudaDe resuelve sufijos de atómicas a la base", () => {
  assert.equal(ayudaDe("amoadd.w.aqrl")?.sintaxis, AYUDA_ISA["amoadd.w"].sintaxis);
  assert.equal(ayudaDe("lr.w.aq")?.descripcion, AYUDA_ISA["lr.w"].descripcion);
});

test("ayudaDe es case-insensitive y devuelve undefined para basura", () => {
  assert.ok(ayudaDe("FADD.S"));
  assert.equal(ayudaDe("xyz"), undefined);
});

test("descripcionInstr arma sintaxis — descripción", () => {
  const d = descripcionInstr("add");
  assert.ok(d?.startsWith("add rd, rs1, rs2 — "));
});
