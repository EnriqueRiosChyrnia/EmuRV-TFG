// Anti-drift es/en (RP16-F2): los dos catálogos tienen EXACTAMENTE las
// mismas claves y las mismas interpolaciones {…}. Si un RP futuro agrega
// una clave a es.json y olvida en.json (o al revés), el test rojo.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { test } from "node:test";

const aqui = dirname(fileURLToPath(import.meta.url));
const lee = (n: string): Record<string, string> =>
  JSON.parse(readFileSync(resolve(aqui, n), "utf8"));
const es = lee("es.json");
const en = lee("en.json");

test("es y en tienen el MISMO conjunto de claves", () => {
  const kes = Object.keys(es).sort();
  const ken = Object.keys(en).sort();
  const faltanEn = kes.filter((k) => !(k in en));
  const sobranEn = ken.filter((k) => !(k in es));
  assert.deepEqual(faltanEn, [], `faltan en en.json: ${faltanEn.join(", ")}`);
  assert.deepEqual(sobranEn, [], `sobran en en.json: ${sobranEn.join(", ")}`);
});

test("cada clave preserva sus interpolaciones {…}", () => {
  const placeholders = (s: string) => (s.match(/\{[a-zA-Z]+\}/g) ?? []).sort();
  for (const k of Object.keys(es)) {
    assert.deepEqual(
      placeholders(en[k]),
      placeholders(es[k]),
      `interpolaciones distintas en «${k}»: es=${es[k]} / en=${en[k]}`,
    );
  }
});

test("ninguna traducción quedó vacía", () => {
  for (const k of Object.keys(en)) {
    assert.ok(en[k].trim().length > 0, `«${k}» vacía en en.json`);
  }
});
