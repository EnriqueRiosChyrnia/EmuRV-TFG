// Tests de la lógica pura de búsqueda en memoria (RP26): parseo del patrón,
// búsqueda en un buffer y escaneo por bloques (bordes: no encontrado y
// patrón partido entre dos lecturas).
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  buscarAdelante,
  buscarAtras,
  buscarPatron,
  parsearPatron,
  type LectorMem,
} from "./memoria-buscar";

// ---- parsearPatron ----

test("byte: hex con y sin prefijo", () => {
  assert.deepEqual(parsearPatron("2A", "byte"), [0x2a]);
  assert.deepEqual(parsearPatron("0x2a", "byte"), [0x2a]);
  assert.deepEqual(parsearPatron("0", "byte"), [0]);
});

test("byte: fuera de rango o no hex → null", () => {
  assert.equal(parsearPatron("100", "byte"), null); // 0x100 > 0xFF
  assert.equal(parsearPatron("ZZ", "byte"), null);
  assert.equal(parsearPatron("", "byte"), null);
});

test("word: 32 bits → 4 bytes little-endian", () => {
  assert.deepEqual(parsearPatron("0000002A", "word"), [0x2a, 0, 0, 0]);
  assert.deepEqual(parsearPatron("0xDEADBEEF", "word"), [0xef, 0xbe, 0xad, 0xde]);
});

test("word: fuera de 32 bits → null", () => {
  assert.equal(parsearPatron("1FFFFFFFF", "word"), null);
});

test("ascii: cada carácter a su byte; no-latin1 → null", () => {
  assert.deepEqual(parsearPatron("Hola", "ascii"), [0x48, 0x6f, 0x6c, 0x61]);
  assert.equal(parsearPatron("café→", "ascii"), null); // '→' no entra en 1 byte
});

// ---- buscarPatron (puro) ----

const BUF = [0x00, 0x2a, 0x00, 0x00, 0x2a, 0xff, 0x2a];

test("buscarPatron: primera aparición y desde", () => {
  assert.equal(buscarPatron(BUF, [0x2a]), 1);
  assert.equal(buscarPatron(BUF, [0x2a], 2), 4);
  assert.equal(buscarPatron(BUF, [0x2a, 0xff]), 4);
});

test("buscarPatron: no encontrado / patrón vacío o más largo que el buffer", () => {
  assert.equal(buscarPatron(BUF, [0x99]), -1);
  assert.equal(buscarPatron(BUF, []), -1);
  assert.equal(buscarPatron([1, 2], [1, 2, 3]), -1);
});

// ---- escaneo por bloques ----

// Memoria simulada: 256 bytes, valor[i] = i. `inicio` es la dirección base.
const BASE = 0x1000;
const MEM = Array.from({ length: 256 }, (_, i) => i);
const lector: LectorMem = async (addr, len) => {
  const off = addr - BASE;
  if (off < 0 || off + len > MEM.length) throw new Error("no mapeado");
  return MEM.slice(off, off + len);
};

test("buscarAdelante: encuentra y devuelve dirección absoluta", async () => {
  // tam pequeño para forzar varios bloques
  const dir = await buscarAdelante(lector, BASE, BASE + 256, [0x10, 0x11], BASE, 8);
  assert.equal(dir, BASE + 0x10);
});

test("buscarAdelante: patrón partido en el borde de un bloque", async () => {
  // con tam=8 el byte 0x07 cierra el 1er bloque y 0x08 abre el 2º: el
  // solape debe encontrar [0x07,0x08] igual.
  const dir = await buscarAdelante(lector, BASE, BASE + 256, [0x07, 0x08], BASE, 8);
  assert.equal(dir, BASE + 0x07);
});

test("buscarAdelante: no encontrado → null", async () => {
  const dir = await buscarAdelante(lector, BASE, BASE + 256, [0xab, 0xcd], BASE, 8);
  assert.equal(dir, null);
});

test("buscarAdelante: zona no mapeada corta el escaneo", async () => {
  // [0x00,0x00] nunca aparece (los valores son únicos), así el escaneo
  // recorre los 256 bytes mapeados y al pedir más allá el lector lanza → null.
  const dir = await buscarAdelante(lector, BASE, BASE + 512, [0x00, 0x00], BASE, 8);
  assert.equal(dir, null);
});

test("buscarAtras: última coincidencia antes del ancla", async () => {
  // 0x2a aparece en MEM en el índice 42 (único). Probamos un patrón repetido:
  // buscamos hacia atrás desde el final el último 0x00 alto... usamos 0x80.
  const dir = await buscarAtras(lector, BASE, BASE + 256, [0x80], BASE + 256, 8);
  assert.equal(dir, BASE + 0x80);
});

test("buscarAtras: ancla excluye la coincidencia en esa dirección", async () => {
  // hay un solo 0x10; buscando hacia atrás con ancla en su propia dirección
  // no debe devolverla (es estrictamente menor).
  const dir = await buscarAtras(lector, BASE, BASE + 256, [0x10], BASE + 0x10, 8);
  assert.equal(dir, null);
});
