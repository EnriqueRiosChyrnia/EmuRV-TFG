# PLAN-RP20 — Rediseño de modales + Ayuda 2 columnas + Acerca de

Punto RP-20 de `docs/ROADMAP-PRO.md` — ÚLTIMO del rediseño visual (y del roadmap,
salvo el tema-de-usuario que queda futuro). Plan por fases con compuertas:
PROHIBIDO avanzar con la compuerta en rojo.

**Fidelidad al diseño (directriz del autor):** replicar `docs/diseno-ui/
propuestas-ui.html` AL PIE DE LA LETRA (clases, estructura, animaciones del bloque
de modales y de la ayuda 2-columnas / about). Desviaciones permitidas solo:
contraste AA, ids del JS, orden de registros.

## Contexto (anclas)

- Modal de ayuda actual: `src/paneles/ayuda.ts` — overlay simple `#ayuda-overlay`
  con secciones apiladas, textos por claves i18n `ayuda.*` (es.json/en.json). El
  CONTENIDO ya existe y está completo; RP-20 reorganiza la PRESENTACIÓN.
- Ejemplos: `<select>`/menú en la toolbar (main.ts, EJEMPLOS). Lecciones:
  navegador de RP-15 (`src/lecciones/`, progreso en localStorage). RP-20 les da la
  cara de «modal con lista» del mock, REUSANDO su lógica (no reescribir el
  navegador ni el progreso).
- Contenido de referencia ya redactado: `docs/AYUDA-CONTENIDO.md` y
  `docs/ABOUT-CONTENIDO.md`. Datos del autor (del mock, sin edad ni universidad —
  decisión del autor): «Enrique Emmanuel Ríos Chyrnia · Estudiante de Ingeniería
  Informática · Paraguay».
- Estética Cálido (RP-19): tokens --radio/--radio-panel/--fuente-ui/--sombra-panel
  ya en :root; el mock de modales usa `--radio-panel`, `--fuente-ui`, animaciones
  `emurv-fade`/`emurv-pop`. Wordmark SVG disponible (RP-19) para el About.

## Alcance

- SÍ: shell de modales (emurv-modal, modal-card, backdrop, animaciones, Escape,
  foco); modal Ayuda de 2 columnas (ay-nav índice agrupado + ay-cont con
  scroll-spy) con TODO el contenido existente reorganizado (ay-tabla, ay-chips,
  ay-nota, ay-num, etc.); sección Acerca de (about) con wordmark + autor + nota
  IA + créditos con licencias + nota GPLv2; modales de Ejemplos y Lecciones con
  la cara del mock (lista, barra de progreso, check/num) sobre la lógica
  existente; docs.
- NO: cambiar el CONTENIDO de la ayuda (solo su presentación); reescribir el
  navegador de lecciones de RP-15 o su persistencia (solo darle cara); tema de
  usuario (futuro).

## Diseño acordado (estructura EXACTA del mock — no re-litigar)

- **Shell modal** (líneas ~356-392 del mock): `.emurv-modal` (overlay) →
  `.modal-backdrop` (cierra al click, anim `emurv-fade`) + `.modal-card` (anim
  `emurv-pop`, `border-radius:var(--radio-panel)`, `--fuente-ui`). `.modal-head`
  (h2 + `.mh-sub` + `.modal-x` con icono). Escape cierra; foco entra al abrir y
  vuelve al disparador al cerrar (accesibilidad — el overlay actual ya maneja
  Escape, conservarlo).
- **Modal Ayuda 2 columnas** (líneas ~394-882): `.ay-body` grid `200px 1fr`.
  `.ay-nav` con grupos (`.ay-nav-tit`: Empezar / Referencia / Herramientas /
  Proyecto) y botones `data-sec`. `.ay-cont` scrolleable con secciones `<section
  id="ay-...">`. Scroll-spy: click en nav → `scrollTo` suave a la sección +
  marcar `.activo` (lógica `ayIr` del mock, ~931-939). Secciones EXACTAS del
  mock: ay-ciclo, ay-ejemplos, ay-syscalls (tabla `.ay-tabla` con `.ay-num`),
  ay-memoria, ay-teclado (tablas + `.ayuda-ejemplo`), ay-directivas
  (`.ay-chips`), ay-editor, ay-depuracion, ay-idioma, ay-about. Todos los textos
  salen de las claves i18n existentes (NO reescribir; mapear a la nueva
  estructura).
- **Acerca de** (`#ay-about`, líneas ~842-881): `.ay-about-id` (wordmark grande +
  tagline con `.ay-ver` = versión), `.ay-autor` (nombre + rol), `.ay-nota` IA,
  grupos `.ay-cred-grupo` + listas `.ay-cred` (item: `.nom` con `<b>` + `.desc` +
  `.ay-lic`), nota `.ay-nota` de licencia GPLv2. Contenido EXACTO del mock
  (créditos con licencias ya verificados contra los manifiestos). Versión: leer
  del manifiesto si es barato; si no, «v0.1» con nota de mantenerla.
- **Modal Ejemplos** (~687-697): lista de `.modal-item` (`.mi-titulo` +
  `.mi-desc`). Cablear a la carga de ejemplos existente.
- **Modal Lecciones** (~699-710): `.modal-progreso` (barra) + `.modal-item` con
  `.modal-check` (completada) / `.modal-num` (pendiente) / «en curso». Cablear al
  navegador y progreso de RP-15 (NO duplicar estado).

## Fase 1 — Shell de modales + Ayuda 2 columnas (estructura + scroll-spy)

Shell común + reescritura de ayuda.ts a 2 columnas con el contenido i18n
existente reubicado en las secciones del mock; scroll-spy. Tests: lo testeable
(scroll-spy como función, presencia de todas las secciones); i18n es/en intacto
(anti-drift). **Compuerta F1:** la ayuda se ve como el mock (2 col, nav agrupado,
tablas/chips/notas); Escape/foco andan; tsc/build; suite Rust intacta; contraste
AA del modal en los 5 temas. Commit `rp20-fase-1: shell de modales y ayuda 2 columnas`.

## Fase 2 — Sección Acerca de

`#ay-about` con wordmark + autor + nota IA + créditos con licencias + nota GPLv2,
contenido exacto del mock. **Compuerta F2:** about completo y fiel; wordmark se
adapta al tema; licencias correctas; tsc/build. Commit `rp20-fase-2: sección Acerca de`.

## Fase 3 — Modales de Ejemplos y Lecciones

Darles la cara del mock sobre la lógica existente (carga de ejemplos; navegador y
progreso de RP-15). La barra de progreso lee el progreso real de RP-15.
**Compuerta F3:** abrir/elegir ejemplo y lección funciona igual que antes pero con
la cara nueva; progreso real reflejado; tsc/build; suite intacta. Commit
`rp20-fase-3: modales de ejemplos y lecciones`.

## Fase 4 — Docs y cierre

README/Ayuda; BITACORA (rediseño de modales, decisión versión, adaptaciones
mock→app). ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB + verificación visual final de los 3
modales en los 5 temas contra el mock. Commit `rp20-fase-4: docs rediseño de modales`.

## Criterios de aceptación

- 3 modales (Ejemplos/Lecciones/Ayuda) con la cara del mock, sobre la lógica
  existente. ✓ F1/F3.
- Ayuda en 2 columnas con índice agrupado + scroll-spy; contenido i18n existente
  reorganizado (no reescrito). ✓ F1.
- Acerca de completo y fiel (wordmark, autor, IA, créditos con licencias, GPLv2).
  ✓ F2.
- Progreso de lecciones real (RP-15), no duplicado. ✓ F3.
- 5 temas AA, ids/registros/Rust intactos, bundle ≤30 MB. ✓ todas.
- Cierra el rediseño visual y el ROADMAP-PRO (salvo tema-de-usuario, futuro).
