// Tests del tema de usuario (RP28): validación/normalización, persistencia,
// aplicación de variables inline al DOM, y el guard anti-drift que ata
// DEFAULT_USUARIO al bloque «educativo» de tokens.css (fuente única).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { test } from "node:test";
import {
  CLAVE_TEMA_USUARIO,
  DEFAULT_USUARIO,
  TOKENS_USUARIO,
  aplicarVarsUsuario,
  esHex,
  guardarTemaUsuario,
  leerTemaUsuario,
  limpiarVarsUsuario,
  normalizarTema,
  temaUsuarioPorDefecto,
} from "./usuario";

// --- stubs mínimos de DOM/localStorage (mismo patrón que index.test.ts) ---
function stubDom() {
  const props = new Map<string, string>();
  (globalThis as unknown as { document: Document }).document = {
    documentElement: {
      style: {
        setProperty: (k: string, v: string) => void props.set(k, v),
        removeProperty: (k: string) => void props.delete(k),
      },
    },
  } as unknown as Document;
  return props;
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

test("esHex acepta #rrggbb y rechaza basura", () => {
  assert.ok(esHex("#1e2227"));
  assert.ok(esHex("#FFFFFF"));
  assert.ok(!esHex("#fff")); // 3 dígitos: no
  assert.ok(!esHex("1e2227")); // sin #
  assert.ok(!esHex("#12345g")); // no-hex
  assert.ok(!esHex(null));
  assert.ok(!esHex(123));
});

test("DEFAULT_USUARIO copia EXACTA del bloque educativo de tokens.css (anti-drift)", () => {
  // Lee los hex del bloque [data-tema="educativo"] tal como hace
  // contraste.test.ts. Si el Educativo cambia un token y no se actualiza
  // DEFAULT_USUARIO, este test falla → no hay copia desincronizada.
  const aqui = dirname(fileURLToPath(import.meta.url));
  const css = readFileSync(resolve(aqui, "tokens.css"), "utf8");
  const i = css.indexOf('[data-tema="educativo"]');
  const cuerpo = css.slice(css.indexOf("{", i) + 1, css.indexOf("}", i));
  const edu: Record<string, string> = {};
  for (const m of cuerpo.matchAll(/--([\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) {
    edu[m[1]] = m[2];
  }
  for (const k of TOKENS_USUARIO) {
    assert.equal(
      DEFAULT_USUARIO[k],
      edu[k],
      `DEFAULT_USUARIO.${k} (${DEFAULT_USUARIO[k]}) != educativo (${edu[k]})`,
    );
  }
});

test("normalizarTema: parcial hereda del default; basura se ignora", () => {
  const t = normalizarTema({ fondo: "#000000", acento: "no-hex", extra: "#fff000" });
  assert.equal(t.fondo, "#000000", "el válido pisa");
  assert.equal(t.acento, DEFAULT_USUARIO.acento, "el inválido hereda del default");
  assert.equal(t.texto, DEFAULT_USUARIO.texto, "el ausente hereda del default");
  assert.equal(Object.keys(t).length, TOKENS_USUARIO.length, "solo tokens conocidos");
});

test("normalizarTema: no-objeto → default completo", () => {
  assert.deepEqual(normalizarTema(null), DEFAULT_USUARIO);
  assert.deepEqual(normalizarTema("x"), DEFAULT_USUARIO);
});

test("temaUsuarioPorDefecto devuelve copia (no la constante)", () => {
  const a = temaUsuarioPorDefecto();
  a.fondo = "#111111";
  assert.equal(DEFAULT_USUARIO.fondo, "#1e2227", "no muta la constante");
});

test("guardar + leer hace round-trip; lee default si no hay nada", () => {
  const mem = stubStorage();
  assert.deepEqual(leerTemaUsuario(), DEFAULT_USUARIO, "sin nada: default");
  const propio = { ...DEFAULT_USUARIO, fondo: "#0a0a0a", acento: "#ff00aa" };
  guardarTemaUsuario(propio);
  assert.equal(mem.get(CLAVE_TEMA_USUARIO), JSON.stringify(propio));
  assert.deepEqual(leerTemaUsuario(), propio);
});

test("leerTemaUsuario ignora JSON corrupto (default)", () => {
  const mem = stubStorage();
  mem.set(CLAVE_TEMA_USUARIO, "{no es json");
  assert.deepEqual(leerTemaUsuario(), DEFAULT_USUARIO);
});

test("aplicarVarsUsuario setea --token; limpiar los quita", () => {
  const props = stubDom();
  aplicarVarsUsuario({ ...DEFAULT_USUARIO, fondo: "#123456" });
  assert.equal(props.get("--fondo"), "#123456");
  assert.equal(props.get("--sx-num"), DEFAULT_USUARIO["sx-num"]);
  assert.equal(props.size, TOKENS_USUARIO.length, "una var por token");
  limpiarVarsUsuario();
  assert.equal(props.size, 0, "limpiar deja :root sin vars de usuario");
});
