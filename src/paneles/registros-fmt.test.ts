// RP32 — formato de registros por ancho (RV32 8 dígitos/i32, RV64 16/i64).
import { test } from "node:test";
import assert from "node:assert/strict";
import { decReg, hexReg } from "./registros-fmt";

test("hexReg: RV32 muestra 8 dígitos, RV64 los 16", () => {
  const hex16 = "0000000012345678";
  assert.equal(hexReg(hex16, "rv32"), "0x12345678");
  assert.equal(hexReg(hex16, "rv64"), "0x0000000012345678");
  // un valor con parte alta no trivial: RV32 solo ve los 8 bajos
  const alto = "0000010000000abc";
  assert.equal(hexReg(alto, "rv32"), "0x00000abc");
  assert.equal(hexReg(alto, "rv64"), "0x0000010000000abc");
});

test("decReg RV32: complemento a dos de 32 bits", () => {
  assert.equal(decReg("0000000000000005", "rv32"), "5");
  assert.equal(decReg("00000000ffffffff", "rv32"), "-1");
  assert.equal(decReg("0000000080000000", "rv32"), "-2147483648");
  // en RV32 la parte alta se ignora (solo importan los 32 bajos)
  assert.equal(decReg("ffffffff00000007", "rv32"), "7");
});

test("decReg RV64: complemento a dos de 64 bits (BigInt, sin perder precisión)", () => {
  assert.equal(decReg("0000000000000005", "rv64"), "5");
  assert.equal(decReg("ffffffffffffffff", "rv64"), "-1");
  assert.equal(decReg("8000000000000000", "rv64"), "-9223372036854775808");
  assert.equal(decReg("0000010000000000", "rv64"), "1099511627776"); // 2^40
  // > 2^53: un `number` perdería precisión; BigInt no
  assert.equal(decReg("0123456789abcdef", "rv64"), "81985529216486895");
});
