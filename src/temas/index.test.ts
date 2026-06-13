// Tests del mecanismo de temas (RP17-F3): validación, persistencia,
// aplicación al DOM. localStorage + documentElement con stubs mínimos.
import assert from "node:assert/strict";
import { test } from "node:test";
import { aplicarTema, esTema, leerTema, setTema, TEMAS } from "./index";

function stubDom() {
  const attrs = new Map<string, string>();
  (globalThis as unknown as { document: Document }).document = {
    documentElement: {
      setAttribute: (k: string, v: string) => void attrs.set(k, v),
      removeAttribute: (k: string) => void attrs.delete(k),
      getAttribute: (k: string) => attrs.get(k) ?? null,
    },
  } as unknown as Document;
  return attrs;
}
function stubStorage() {
  const mem = new Map<string, string>();
  (globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (k: string) => mem.get(k) ?? null,
    setItem: (k: string, v: string) => void mem.set(k, v),
    removeItem: (k: string) => void mem.delete(k),
    clear: () => mem.clear(),
    key: () => null,
    length: 0,
  } as Storage;
  return mem;
}

test("esTema valida", () => {
  assert.ok(esTema("educativo"));
  assert.ok(esTema("retro"));
  assert.ok(esTema("pergamino"));
  assert.ok(esTema("kawaii"));
  assert.ok(esTema("disquete"));
  assert.ok(!esTema("rosa"));
  assert.ok(!esTema(null));
});

test("hay 5 temas, educativo primero (orden del selector)", () => {
  assert.deepEqual(TEMAS, ["educativo", "retro", "pergamino", "kawaii", "disquete"]);
});

test("aplicarTema: educativo quita el atributo, retro lo pone", () => {
  const attrs = stubDom();
  aplicarTema("retro");
  assert.equal(attrs.get("data-tema"), "retro");
  aplicarTema("educativo");
  assert.equal(attrs.has("data-tema"), false, "educativo = :root, sin atributo");
});

test("setTema persiste y aplica en vivo", () => {
  const attrs = stubDom();
  const mem = stubStorage();
  setTema("retro");
  assert.equal(mem.get("emurv.tema.v1"), "retro");
  assert.equal(attrs.get("data-tema"), "retro");
});

test("leerTema: default educativo, lee lo guardado, ignora basura", () => {
  const mem = stubStorage();
  assert.equal(leerTema(), "educativo");
  mem.set("emurv.tema.v1", "retro");
  assert.equal(leerTema(), "retro");
  mem.set("emurv.tema.v1", "basura");
  assert.equal(leerTema(), "educativo");
});
