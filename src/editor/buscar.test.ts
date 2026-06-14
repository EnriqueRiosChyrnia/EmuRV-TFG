// Tests del mapa de traducción del panel de búsqueda (RP-24).
// Sin localStorage (entorno de test) el idioma activo es español, así que
// frasesBusqueda() devuelve las etiquetas en español.
import assert from "node:assert/strict";
import { test } from "node:test";
import { frasesBusqueda } from "./buscar";

// Frases fuente exactas que `@codemirror/search` pasa a state.phrase(...).
const FRASES_FUENTE = [
  "Find",
  "Replace",
  "next",
  "previous",
  "all",
  "match case",
  "by word",
  "regexp",
  "replace",
  "replace all",
  "close",
  "current match",
  "replaced $ matches",
  "replaced match on line $",
  "on line",
  "Go to line",
  "go",
];

test("frasesBusqueda cubre todas las frases fuente del panel", () => {
  const frases = frasesBusqueda();
  for (const fuente of FRASES_FUENTE) {
    assert.ok(fuente in frases, `falta la frase fuente «${fuente}»`);
  }
});

test("frasesBusqueda traduce al español (sin localStorage)", () => {
  const frases = frasesBusqueda();
  assert.equal(frases["Find"], "Buscar");
  assert.equal(frases["Replace"], "Reemplazar");
});

test("frasesBusqueda no deja ninguna etiqueta vacía ni sin traducir", () => {
  const frases = frasesBusqueda();
  for (const [fuente, texto] of Object.entries(frases)) {
    assert.ok(texto.length > 0, `«${fuente}» quedó vacía`);
    // Una clave i18n sin entrada caería al fallback = la propia clave.
    assert.ok(!texto.startsWith("editor.buscar."), `«${fuente}» sin traducir: ${texto}`);
  }
});

test("frasesBusqueda conserva el marcador $ en las plantillas", () => {
  const frases = frasesBusqueda();
  assert.ok(frases["replaced $ matches"].includes("$"));
  assert.ok(frases["replaced match on line $"].includes("$"));
});
