// Tests del loader i18n (RP16-F1): resolución pura, fallback, persistencia.
import assert from "node:assert/strict";
import { test } from "node:test";
import { traducir, getLang, setLang, CLAVE_LANG } from "./index";

const DICTS = {
  es: { saludo: "Hola", soloEs: "Solo español", n: "Tenés {x}" },
  en: { saludo: "Hello", n: "You have {x}" },
};

test("traducir lee del idioma activo", () => {
  assert.equal(traducir(DICTS, "en", "saludo"), "Hello");
  assert.equal(traducir(DICTS, "es", "saludo"), "Hola");
});

test("fallback a español si la clave falta en el activo", () => {
  assert.equal(traducir(DICTS, "en", "soloEs"), "Solo español");
});

test("clave inexistente devuelve la propia clave", () => {
  assert.equal(traducir(DICTS, "en", "noexiste"), "noexiste");
});

test("interpola params en el idioma activo", () => {
  assert.equal(traducir(DICTS, "en", "n", { x: 3 }), "You have 3");
  assert.equal(traducir(DICTS, "es", "n", { x: 3 }), "Tenés 3");
});

test("setLang/getLang persisten con localStorage (stub)", () => {
  const mem = new Map<string, string>();
  (globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (k: string) => mem.get(k) ?? null,
    setItem: (k: string, v: string) => void mem.set(k, v),
    removeItem: (k: string) => void mem.delete(k),
    clear: () => mem.clear(),
    key: () => null,
    length: 0,
  } as Storage;
  // setLang escribe en localStorage y getLang refleja el cambio (el módulo
  // se inicializó sin stub, leerLang cayó a "es"; setLang lo corrige).
  setLang("en");
  assert.equal(getLang(), "en");
  assert.equal(mem.get(CLAVE_LANG), "en");
  setLang("es");
  assert.equal(getLang(), "es");
  assert.equal(mem.get(CLAVE_LANG), "es");
});
