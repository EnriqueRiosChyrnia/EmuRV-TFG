// Anti-drift del navegador de lecciones (RP15-F1): el parser corre sobre
// los .s REALES del disco (la misma fuente que ?raw embebe). Si una
// lección rompe la plantilla de RP-9, estos tests lo cazan.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { test } from "node:test";
import { parseLeccion } from "./parser";

const aqui = dirname(fileURLToPath(import.meta.url));
const raiz = resolve(aqui, "../..");
const lee = (rel: string) => readFileSync(resolve(raiz, "lecciones", rel), "utf8");

// Orden de currículo (no alfabético): carpeta + archivo principal.
const CURRICULO: Array<[number, string]> = [
  [1, "01-registros/leccion.s"],
  [2, "02-aritmetica/leccion.s"],
  [3, "03-memoria/leccion.s"],
  [4, "04-condicionales/leccion.s"],
  [5, "05-bucles/leccion.s"],
  [6, "06-funciones/leccion.s"],
  [7, "07-syscalls/leccion.s"],
  [8, "08-multiarchivo/main.s"],
  [9, "09-mmio/leccion.s"],
  [10, "10-traps/leccion.s"],
];

test("las 10 lecciones parsean número, título, objetivos y paneles", () => {
  for (const [n, rel] of CURRICULO) {
    const meta = parseLeccion(lee(rel));
    assert.equal(meta.numero, n, `número de ${rel}`);
    assert.ok(meta.titulo.length > 0, `${rel} sin título`);
    assert.ok(meta.objetivos.length >= 1, `${rel} sin objetivos`);
    assert.ok(meta.paneles.length > 0, `${rel} sin paneles`);
  }
});

test("la 08 tiene sus tres archivos en disco", () => {
  for (const f of ["main.s", "utiles.s", "constantes.s"]) {
    assert.ok(lee(`08-multiarchivo/${f}`).length > 0, `falta 08/${f}`);
  }
});

test("parseLeccion separa objetivos y une continuaciones", () => {
  const meta = parseLeccion(
    "# Lección 99 — Prueba\n# OBJETIVOS — vas a poder:\n#   * uno\n#     sigue uno\n#   * dos\n# PANELES PARA OBSERVAR: «Consola»\n# CÓMO USARLA: x\n",
  );
  assert.equal(meta.numero, 99);
  assert.equal(meta.titulo, "Prueba");
  assert.deepEqual(meta.objetivos, ["uno sigue uno", "dos"]);
  assert.match(meta.paneles, /Consola/);
});

test("títulos de muestra: 01 Registros, 10 Traps", () => {
  assert.match(parseLeccion(lee("01-registros/leccion.s")).titulo, /Registros/);
  assert.match(parseLeccion(lee("10-traps/leccion.s")).titulo, /Traps/);
});
