# PLAN-RP18 — Ajustes de layout y UX

Punto RP-18 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

**Origen:** sesión de diseño con Claude (claude.ai) sobre capturas reales,
2026-06-13. Propuestas verificadas contra el código por la coordinadora — ver
`docs/UI-BRIEF.md` (sección «Problemas de layout conocidos»). Es trabajo
estructural de UI (≠ temas, que son RP-17 Parte B).

## Restricciones (heredadas)

- Sin frameworks; CSS puro + TS vanilla. Estética solo por tokens (no introducir
  literales de color/medida nuevos; si hace falta un valor parametrizable,
  proponerlo como token).
- NO renombrar ni eliminar los `id` que el JS consume (ver UI-BRIEF.md).
- Legibilidad WCAG AA intacta; bundle ≤ 30 MB; UI en español (i18n es/en).
- Público principiante: criterio = claridad, reducir ruido, no «wow».

## Hechos verificados (anclas, 2026-06-13)

- `#app` grid 2col × 4filas; `#lateral` es grid de SOLO 2 pistas (`1.2fr 1fr`)
  para 4 paneles → raíz del aplastamiento de Registros. (styles.css)
- `#toolbar` es `display:flex` SIN `flex-wrap` → bug de desborde en ventana
  angosta (controles derechos se tapan). (styles.css)
- Registros ES `<table class="registros-tabla">` con filas `<tr>` por registro
  (registros.ts); el `<colgroup>` y la clase `.grupo` se agregan ahí.
- `.panel-titulo-clickeable` y `.panel.colapsado .panel-cuerpo { display:none }`
  YA existen en styles.css → progressive disclosure los reutiliza.
- CSRs y registros F YA son `<details>` cerrados por defecto (RP-8/RP-12) → el
  «punto D» de la propuesta YA está hecho; NO tocar.

## Alcance

- SÍ: fix responsive de toolbar; `#lateral` flex; canvas de video acotado;
  registros compactos + separadores de grupo + tooltips de rol; progressive
  disclosure (Memoria plegable, Depuración plegada por defecto, watchpoints en
  sub-sección con mini-ayuda); de-emphasis de botones secundarios; docs.
- NO: reordenar registros (es la convención RISC-V — PROHIBIDO, ver UI-BRIEF);
  reescribir `grid-template-areas` de `#app` (el ensanche de consola cuando el
  video está oculto queda como propuesta futura — más invasivo); temas (RP-17B);
  menú overflow «⋯» con ResizeObserver (mejora futura; el fix por defecto es
  `flex-wrap`).

## Fase 1 — BUG: toolbar responsive (lo más urgente y aislado)

`#toolbar { flex-wrap: wrap; row-gap: 8px; }` — en ventana angosta los controles
bajan a una 2ª fila en vez de cortarse. No toca ids ni JS.
**Compuerta F1:** verificación manual a varios anchos (incluido bien angosto):
ningún control se tapa; maximizada igual que antes (BITACORA con notas/capturas).
tsc/build limpios; suite Rust intacta.
Commit `rp18-fase-1: fix responsive de la toolbar (flex-wrap)`.

## Fase 2 — Densidad del lateral: #lateral flex + canvas acotado + registros

1. `#lateral` de grid 2-pistas → `display:flex; flex-direction:column` con
   prioridades: video `flex:0 0 auto`, registros `flex:2.2 1 0; min-height:160px`
   (prioritario), memoria `flex:1.5 1 0; min-height:120px`, depuración
   `flex:0 1 auto; max-height:200px`. (flex maneja solo el caso
   `#panel-video[hidden]`.)
2. `#video-canvas { width:auto; max-height:170px; margin:8px auto; }` — sin tocar
   los atributos `width="320" height="240"` (resolución interna intacta).
   Verificar que la ventana desacoplada `pantalla.html` NO se afecte (tiene su
   propio style).
3. Registros compactos: `.registros-tabla { table-layout:fixed; }` + `<colgroup>`
   con anchos (≈30/38/32%) en el render; separadores de grupo: clase `.grupo` en
   la PRIMERA fila de cada bloque ABI (ra=x1, t0=x5, s0=x8, a0=x10, s2=x18,
   t3=x28) con `tr.grupo td { border-top:1px solid var(--borde); }`; tooltips
   `title` con el rol de cada registro en la celda del nombre. ORDEN x0..x31
   INTACTO.
**Compuerta F2:** verificación visual (registros legibles sin hueco, pantalla no
domina, separadores correctos); orden de registros sin cambios (test/inspección);
tsc/build/clippy; bundle ≤ 30 MB.
Commit `rp18-fase-2: lateral flexible, canvas acotado y registros compactos`.

## Fase 3 — Progressive disclosure + de-emphasis

1. #panel-memoria: `<h2>` con `.panel-titulo-clickeable` + handler que togglea
   `.colapsado`; ABIERTO por defecto.
2. #panel-depuracion: arranca con `.colapsado` (PLEGADO); título visible
   «Depuración» (considerar sufijo «· avanzado» vía i18n); se expande al click y
   automáticamente al dar de alta el primer breakpoint
   (`panelDepuracion.classList.remove('colapsado')` en ese handler).
3. Watchpoints: mover a sub-sección plegada dentro de Depuración, con mini-ayuda
   «¿Qué es esto?» (texto i18n) visible solo al desplegar; breakpoints quedan
   directos a la vista. Usar `color-mix(in srgb, var(--acento) 10%, transparent)`
   para la cajita (ya se usa en styles.css — consistente, sin token nuevo).
4. De-emphasis: clase `.deemph` (`opacity:.78; :hover→1`) en los botones
   secundarios (Lecciones, Pantalla, Abrir, Guardar, Exportar ELF, Ayuda) donde
   se crean en main.ts; `.principal` (Ensamblar) sigue siendo el único con acento.
5. `grupo-ejec` (del diseño aprobado): envolver Ejecutar/Paso/Detener/Reiniciar
   en un contenedor `.grupo-ejec` para que se vean como UN bloque segmentado, no
   4 cajas sueltas (CSS de bordes compartidos del diseño). Solo la ESTRUCTURA +
   el segmentado básico acá; el pulido cálido (radios, hover) es RP-19.

Referencia visual exacta de todo lo anterior: `docs/diseno-ui/propuestas-ui.html`
(layout «propuesto», bloque CSS correspondiente). El diseño usa `data-colapsado`
como atributo; el código ya tiene la clase `.colapsado` — usá la que ya existe,
es equivalente y más simple.
**Compuerta F3:** funciones de plegado testeadas si son puras; verificación
manual del flujo (Depuración plegada al abrir, se expande al poner breakpoint,
watchpoints con ayuda); i18n de los textos nuevos en es.json Y en.json
(anti-drift de RP-16 lo exige); tsc/clippy.
Commit `rp18-fase-3: progressive disclosure y jerarquía de toolbar`.

## Fase 4 — Docs y cierre

README/Ayuda si algún flujo cambió de cara al usuario (Depuración plegada,
watchpoints con ayuda); BITACORA (decisiones, antes/después, qué se dejó como
futuro: ensanche de consola, menú overflow). ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB; verificación visual final a
varios anchos de ventana.
Commit `rp18-fase-4: docs ajustes de layout y UX`.

## Criterios de aceptación

- Toolbar usable a cualquier ancho (bug resuelto). ✓ F1.
- Registros legible, sin aplastar, con separadores y tooltips; orden x0..x31
  INTACTO. ✓ F2.
- Pantalla no domina el lateral. ✓ F2.
- Progressive disclosure: Depuración plegada por defecto, Memoria plegable,
  watchpoints con ayuda. ✓ F3.
- Jerarquía visual: solo Ensamblar con acento, secundarios atenuados. ✓ F3.
- Cero regresión funcional (ids intactos, suite verde); legibilidad AA y bundle
  ≤30 MB. ✓ todas.
- i18n es/en de textos nuevos (anti-drift verde). ✓ F3.
