# PLAN-RP24 — Find/Replace en el editor

Punto RP-24 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas: PROHIBIDO
avanzar con la compuerta en rojo. Se ejecuta en su WORKTREE (rama
`rp24/find-replace`), área `src/editor/` — disjunta de los otros RP del lote.

## Contexto

- Editor CodeMirror 6 en `src/editor/index.ts` (`createEditor`, lista de
  `extensions`). Hoy NO está `@codemirror/search` instalado (verificado).
- Otros paquetes `@codemirror/*` ya están en package.json (commands, view, etc.).

## Alcance

- SÍ: búsqueda y reemplazo en el editor — instalar `@codemirror/search`, agregar
  `search()` + `searchKeymap` + `highlightSelectionMatches` a las extensions;
  Cmd/Ctrl+F abre el panel de búsqueda, con reemplazo; resaltado de coincidencias;
  textos del panel en español (y en inglés según idioma). Funciona también en
  modo binario (listado read-only: buscar sí, reemplazar no aplica).
- NO: buscar en memoria (es RP-26, otro worktree); buscar multi-archivo.

## Diseño

- En `createEditor`, agregar a las extensions: `search({ top: true })`,
  `keymap.of(searchKeymap)`, `highlightSelectionMatches()`. Mantener el resto
  (resaltado de línea, breakpoints, lint) intacto.
- Textos del panel: CodeMirror permite `EditorState.phrases` o el `search`
  config para traducir las etiquetas ("Find", "Replace", "next"...). Mapear al
  idioma activo (módulo i18n) — al menos español; inglés puede quedar en el
  default de CM. Si traducir las phrases de CM es engorroso, documentarlo y
  dejar el panel en inglés (su contenido es estándar) — registrar la decisión.
- NO romper: el gutter de breakpoints, el resaltado de línea en ejecución, el
  modo binario read-only.

## Fase 1 — Búsqueda/reemplazo

Instalar `@codemirror/search`; cablear extensions; verificar Cmd+F abre el panel,
busca, reemplaza, resalta; modo binario permite buscar. Tests: si hay algo
testeable como función, hacerlo; el grueso es verificación manual (BITACORA).
**Compuerta F1:** Cmd+F funciona (buscar+reemplazar+resaltar); editor intacto
(breakpoints, exec-line); tsc + suite UI verdes; clippy.
Commit `rp24-fase-1: find/replace en el editor`.

## Fase 2 — Docs y cierre

README/Ayuda (atajo de búsqueda); BITACORA (decisión de phrases). NO tocar
ROADMAP-PRO ni Cap. VI. Claves i18n nuevas (si las hay): AL FINAL de es.json y
en.json (minimiza conflicto de merge con otros worktrees).
**Compuerta F2 (cierre):** cargo test + clippy AMBOS crates + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB.
Commit `rp24-fase-2: docs find/replace`.

## Criterios de aceptación

- Cmd/Ctrl+F: buscar + reemplazar + resaltar coincidencias. ✓ F1.
- Editor existente intacto (breakpoints, exec-line, binario). ✓ F1.
- Bundle ≤30 MB (dep nueva del ecosistema CM, liviana). ✓ F2.
