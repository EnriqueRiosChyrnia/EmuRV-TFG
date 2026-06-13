// Guarda la regla DURA de RP18: NO reordenar los registros (convención
// RISC-V x0..x31). Si un cambio futuro toca el orden, los separadores de
// grupo o deja un rol sin traducir, este test se pone rojo.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { test } from "node:test";
import { ABI, GRUPOS_ABI, claveRol } from "./registros-meta";

const aqui = dirname(fileURLToPath(import.meta.url));
const lee = (n: string): Record<string, string> =>
  JSON.parse(readFileSync(resolve(aqui, `../i18n/${n}`), "utf8"));

test("ABI = orden canónico RISC-V x0..x31 (NO reordenar)", () => {
  assert.equal(ABI.length, 32);
  assert.deepEqual([...ABI], [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1",
    "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7",
    "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
    "t3", "t4", "t5", "t6",
  ]);
});

test("los separadores .grupo caen en el inicio de cada bloque ABI", () => {
  assert.deepEqual([...GRUPOS_ABI].sort((a, b) => a - b), [1, 5, 8, 10, 18, 28]);
  // ra, t0, s0, a0, s2, t3 — el primero de cada bloque funcional.
  assert.deepEqual(
    [...GRUPOS_ABI].sort((a, b) => a - b).map((i) => ABI[i]),
    ["ra", "t0", "s0", "a0", "s2", "t3"],
  );
});

test("claveRol da un rol traducido para los 32, ninguno cae en «otro»", () => {
  const es = lee("es.json");
  const en = lee("en.json");
  for (const nombre of ABI) {
    const clave = claveRol(nombre);
    assert.notEqual(clave, "registros.rol.otro", `${nombre} sin rol propio`);
    assert.ok(clave in es, `falta ${clave} en es.json`);
    assert.ok(clave in en, `falta ${clave} en en.json`);
  }
});
