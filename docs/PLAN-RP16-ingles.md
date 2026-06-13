# PLAN-RP16 — Internacionalización: inglés (UI)

Punto RP-16 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

**Restricción del autor:** este punto va al final; el español es el diferencial de
la tesis, el inglés es alcance. Por eso v1 traduce SOLO la UI cáscara (ver alcance).

## Contexto (anclas en código, medido por la coordinadora 2026-06-13)

- i18n actual: `src/i18n/es.json` (164 claves) + `src/i18n/index.ts` con un dict
  ESTÁTICO (`import es` → `const dict = es`) y `t(key, params)`. NO hay selector
  de idioma ni concepto de idioma activo.
- Tres capas de texto con esfuerzo MUY distinto:
  1. **UI cáscara** (164 claves JSON) — externalizada, lista para traducir.
  2. **Mensajes didácticos del ensamblador/runtime** (~98 strings con acentos en
     `src-tauri/src/core/*.rs` + commands.rs) — HARDCODEADOS en Rust, NO
     externalizados. Material de tesis en español.
  3. **Contenido de las lecciones (RP-9)** — 10 `.s` en español, currículo
     completo. Material de tesis.

## Alcance (decisión del autor, 2026-06-13)

- SÍ (v1): traducir la capa 1 (UI cáscara) a inglés; selector de idioma con
  persistencia; anti-drift en.json↔es.json; docs.
- NO (v1, documentado como futuro CON RAZÓN, no como olvido):
  - Capa 2 (mensajes del ensamblador/runtime): externalizarlos del Rust es un
    refactor grande de bajo ROI para un público hispanohablante; y son material
    didáctico de tesis. En v1, si el usuario en modo inglés comete un error de
    ensamblado, el diagnóstico sale en español — LIMITACIÓN CONOCIDA Y ACEPTADA,
    registrada en docs y Ayuda. Propuesta futura: pasar un locale al core o
    externalizar a un catálogo de mensajes.
  - Capa 3 (lecciones): traducir el currículo es rehacerlo; queda como propuesta
    de trabajo futuro (un `lecciones-en/` paralelo el día que haya demanda).

## Diseño acordado (no re-litigar)

- **Loader multi-idioma:** `index.ts` pasa de dict estático a dict por idioma
  activo. Idiomas: `es` (default, primario) y `en`. `setLang(lang)` cambia el
  activo; `t()` lee del activo con FALLBACK a español si una clave faltara (que
  el anti-drift impide, pero el fallback es la red). Persistencia en localStorage
  (clave versionada `emurv.lang.v1`), patrón RP-15.
- **Aplicar el cambio:** al cambiar idioma, re-renderizar la UI. Si el
  re-render reactivo de todos los paneles es invasivo, es ACEPTABLE recargar la
  ventana del webview tras persistir el idioma (simple, robusto, sin estado a
  medio traducir) — decidir según el costo real y registrar en BITACORA.
- **Selector:** control en la toolbar o en un menú (es/en), mostrando el idioma
  actual; el español queda primero (es el primario).
- **en.json:** traducción de las 164 claves. Inglés técnico estándar de
  herramientas de desarrollo (Assemble, Step, Run, Stop, Reset, Registers,
  Memory, Console, Breakpoints, Watchpoints…). Términos RISC-V se mantienen
  (ABI, ecall, etc.). Revisar que las interpolaciones `{clave}` se preserven.
- **Anti-drift (patrón RP-14/15):** test que en.json y es.json tienen EXACTAMENTE
  el mismo conjunto de claves — ni faltantes ni sobrantes. Si un RP futuro agrega
  una clave a es.json y olvida en.json, test rojo.

## Fase 1 — Loader multi-idioma + selector + persistencia

Refactor de index.ts (idioma activo, setLang, fallback es, localStorage) +
selector en la UI + es/en registrados (en.json puede empezar copiando es y se
completa en F2; o se hace junto — a elección, pero F1 cierra con el mecanismo
funcionando aunque en muestre español). Tests: setLang persiste y t() lee del
activo; fallback a es si falta clave.
**Compuerta F1:** tests verdes + tsc limpio + suite Rust intacta.
Commit `rp16-fase-1: loader multi-idioma, selector y persistencia`.

## Fase 2 — Traducción en.json + anti-drift + auditoría de hardcodeos

1. en.json con las 164 claves traducidas (inglés técnico, interpolaciones
   preservadas).
2. Test anti-drift: conjuntos de claves es==en idénticos.
3. Auditoría: buscar strings de UI hardcodeados que NO pasen por t() (se
   escaparon del i18n) — si los hay, externalizarlos a ambos JSON. (grep de
   texto visible en .ts que no venga de t()).
**Compuerta F2:** anti-drift verde + auditoría hecha (hardcodeos = 0 o
externalizados) + tsc + verificación manual: cambiar a inglés muestra la UI en
inglés, volver a español funciona (BITACORA).
Commit `rp16-fase-2: traducción inglesa de la UI y anti-drift es/en`.

## Fase 3 — Docs y cierre

README (selector de idioma; nota de que diagnósticos del ensamblador y lecciones
son español en v1 — limitación conocida con su razón), Ayuda (mención del
selector), BITACORA (decisiones: alcance UI-only, método de aplicación del
cambio, limitación de capas 2/3). ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F3 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB.
Commit `rp16-fase-3: docs internacionalización`.

## Criterios de aceptación

- UI traducible es/en con selector y persistencia. ✓ F1/F2.
- en.json completo, anti-drift es==en por test. ✓ F2.
- Cero strings de UI hardcodeados fuera de t() (auditados). ✓ F2.
- Limitación de v1 (diagnósticos y lecciones en español) documentada como
  decisión, no omisión. ✓ F3.
- i18n ya era base (CLAUDE.md regla 6) — esto es traducción + selector, no
  reingeniería. ✓.
