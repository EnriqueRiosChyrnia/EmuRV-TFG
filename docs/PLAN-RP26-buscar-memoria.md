# PLAN-RP26 — Búsqueda en el panel de Memoria

Punto RP-26 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas: PROHIBIDO
avanzar con la compuerta en rojo. Se ejecuta en su WORKTREE (rama
`rp26/buscar-memoria`), área `src/paneles/memoria.ts` — disjunta del lote.

## Contexto

- Panel de Memoria: `src/paneles/memoria.ts` (clase `Memoria`). Ya tiene
  controles (input de dirección + botones Ir/.text/.data/pila), `irA(addr)`,
  `refrescar()`, y usa `api.readMem(addr, len)`.
- Mapa: .text 0x00400000, .data 0x10010000, pila ~0x7FFFFFF0.

## Alcance

- SÍ: un campo de búsqueda en el panel para encontrar un valor/patrón en memoria
  y saltar a su dirección; navegación siguiente/anterior coincidencia; resaltar
  la coincidencia en el volcado.
- NO: buscar en el editor (es RP-24); reemplazar en memoria; búsqueda regex.

## Diseño

- Control nuevo en el panel (junto a los controles de dirección): input de
  búsqueda + selector de tipo (byte/word hex, o texto ASCII) + botón Buscar (y
  «siguiente»). Ejemplos de uso: buscar `2A` (byte), `0000002A` (word), o la
  cadena `Hola`.
- Escaneo: leer por bloques con `api.readMem` sobre un rango razonable
  (p. ej. desde .data hasta fin de la región de datos, o un rango configurable)
  y buscar el patrón. Para no congelar la UI con rangos enormes, escanear en
  trozos (p. ej. 4 KB) hasta la primera coincidencia; «siguiente» continúa desde
  la última. Si el rango es muy grande y un escaneo frontend resulta lento,
  acotar el rango buscado (datos + pila) y documentarlo; NO agregar comando Rust
  nuevo salvo necesidad real (registrar si se agrega).
- Al encontrar: `irA(dir alineada)` + marcar la coincidencia (resaltado en el
  volcado, clase CSS por tokens). Sin coincidencia: aviso didáctico breve.
- Endianness: el word se busca en little-endian (como se almacena) — documentar.

## Fase 1 — Búsqueda en memoria

Control + escaneo por bloques + saltar/resaltar + siguiente/anterior. Tests:
la función de búsqueda de patrón en un buffer (pura: dado bytes + patrón →
offsets) testeable sin DOM; bordes (no encontrado, patrón al borde de bloque).
**Compuerta F1:** buscar byte/word/ASCII salta y resalta; siguiente recorre;
no congela con rango grande; tsc + suite verdes; clippy.
Commit `rp26-fase-1: búsqueda en el panel de memoria`.

## Fase 2 — Docs y cierre

README/Ayuda (cómo buscar en memoria); BITACORA (rango escaneado, endianness,
si se agregó comando). NO tocar ROADMAP-PRO ni Cap. VI. Claves i18n AL FINAL de
es.json y en.json.
**Compuerta F2 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB.
Commit `rp26-fase-2: docs búsqueda en memoria`.

## Criterios de aceptación

- Buscar byte/word/ASCII en memoria, saltar a la dirección y resaltar. ✓ F1.
- Siguiente/anterior coincidencia. ✓ F1.
- Sin congelar la UI (escaneo por bloques / rango acotado). ✓ F1.
- Lógica de búsqueda testeada (función pura). ✓ F1.
