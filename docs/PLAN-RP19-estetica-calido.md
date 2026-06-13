# PLAN-RP19 — Estética «Cálido» (única) + microinteracciones + wordmark

Punto RP-19 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas: PROHIBIDO
avanzar con la compuerta en rojo.

**Fidelidad al diseño (directriz del autor):** replicar `docs/diseno-ui/
propuestas-ui.html` AL PIE DE LA LETRA — valores, espaciados, keyframes tal cual.
Decisión del autor: «Cálido» es el ÚNICO look (NO hay toggle Base) → los estilos
que en el mock están bajo `[data-estetica="calido"]` van SIEMPRE (a `:root` / sin
el selector de estética). Desviaciones permitidas solo: contraste AA, ids del JS,
orden de registros.

## Contexto (anclas)

- `src/temas/tokens.css`: 5 temas (RP-17B), tokens base + `--sx-*`. Educativo
  hoy tiene `--acento:#4ea1ff` (azul).
- `src/styles.css`: layout ya flexibilizado (RP-18), grupo-ejec con
  `border-radius:6px` literal (RP-18 lo dejó para tokenizar acá).
- RP-17A: JetBrains Mono self-hosted (`src/assets/fonts/`, `@font-face` en
  tokens.css) — MISMO patrón para IBM Plex Sans acá.
- Wordmark SVG del diseño: `docs/diseno-ui/assets/emurv-wordmark.svg` (bi-tono:
  `.logo-emu`=acento, `.logo-rv`=texto). El SVG inline está también en
  propuestas-ui.html (toolbar y about).

## Alcance

- SÍ: tokens nuevos (`--radio`, `--radio-panel`, `--fuente-ui`, `--sombra-panel`)
  a `:root`; IBM Plex Sans self-hosted; acento violeta del Educativo; lenguaje
  visual Cálido aplicado siempre (aire, paneles-tarjeta, bordes redondeados,
  densidad, jerarquía de toolbar); microinteracciones (transiciones,
  focus-visible, caret de consola, flash one-shot de registros, hover de
  breakpoint); grupo-ejec tokenizado; refinados de ruido visual; wordmark en la
  toolbar.
- NO: modales / ayuda 2-columnas / sección Acerca de (RP-20 — aunque el wordmark
  SVG se agrega acá y RP-20 lo reusa); toggle de estética (Cálido es único);
  tema de usuario (futuro).

## Diseño acordado (valores EXACTOS del mock — no re-litigar)

Tokens nuevos a `:root` (y disponibles para todos los temas):
`--radio:9px; --radio-panel:14px; --fuente-ui:"IBM Plex Sans", system-ui,
-apple-system, sans-serif; --sombra-panel:0 1px 2px rgba(0,0,0,.16), 0 6px 18px
rgba(0,0,0,.10);`

Educativo → acento violeta (reemplaza el azul): `--acento:#9b8cff;
--texto-sobre-acento:#16122b; --resaltado-linea:rgba(155,140,255,.20);
--sx-lbl:#a99bff; --sx-ins:#c4b6ff;` (VERIFICAR contraste AA — si raspa, ajustar
un escalón y registrar).

Lenguaje Cálido (del mock, aplicado SIEMPRE): `#app { gap:10px; padding:10px }`;
`#editor,#toolbar,#estado { border:1px solid var(--borde); border-radius:
var(--radio-panel) }`; `#lateral { gap:10px }`; `.panel { border + border-radius:
var(--radio-panel) + box-shadow:var(--sombra-panel) }`; `.panel h2 { background:
color-mix(in srgb, var(--acento) 9%, var(--fondo-panel)); padding:9px 13px }`;
densidad (registros td 4px 12px, th 7px 12px + tinte, `.cm` line-height:1.65
padding:10px 0, botones toolbar 7px 13px, consola-salida 10px 14px); jerarquía
toolbar (principal con `box-shadow` de acento; `.deemph` background/border
transparentes, hover los revela). Tipografía UI (`--fuente-ui`) en toolbar, panel
h2/h3, registros th, depuracion-vacio, wp-ayuda, consola-entrada, estado — el
CÓDIGO y los DATOS siguen en `--mono`.

Microinteracciones (keyframes y reglas EXACTAS del mock): `transition` de
color/border/box-shadow en button/sel/input/summary; `:focus-visible { outline:2px
solid var(--acento); outline-offset:2px }`; consola con `.ce-prompt/.ce-caret`
(keyframe `emurv-caret`)/`.ce-hint`; registros `tr.cambiado td { animation:
emurv-cambio-flash .7s ease-out }` (one-shot, NO bucle); editor breakpoint hover
hint (`.ln:not(.bp):hover .num::after`) + barra de cambio en registros.

Refinados de ruido: th `text-transform:none; font-weight:500`; `.registros-xn`
`opacity:.38; font-size:10.5px`; memoria `.cero { opacity:.45 }`; sin cebra
(tarjeta + hover bastan) → `tr:nth-child(even){ background:transparent }` +
`tbody tr:hover td`; separadores de grupo más suaves (color-mix 50%); grupo-ejec
con `border-radius:var(--radio)` (tokeniza el 6px literal de RP-18).

Wordmark: SVG inline bi-tono en la toolbar (al inicio, antes del separador), con
`.logo-emurv { height:21px }`, `.logo-emu { fill:var(--acento) }`, `.logo-rv {
fill:var(--texto) }`. Reusar el SVG de `docs/diseno-ui/assets/emurv-wordmark.svg`.

## Fase 1 — Tokens Cálido + IBM Plex + acento violeta + lenguaje visual

Tokens nuevos a :root; IBM Plex Sans self-hosted (`@font-face`, archivo en
`src/assets/fonts/`, licencia OFL registrada en BITACORA); acento violeta del
Educativo; aplicar el lenguaje Cálido (aire/tarjetas/bordes/densidad/jerarquía).
**Compuerta F1:** se ve como el mock (verificación visual contra propuestas-ui.html,
BITACORA); contraste AA de los 5 temas SIGUE verde (incluido el violeta del
Educativo); bundle re-medido ≤30 MB (IBM Plex suma ~100KB); tsc/build; suite Rust
intacta. Commit `rp19-fase-1: estética Cálido (tokens, IBM Plex, paneles-tarjeta)`.

## Fase 2 — Microinteracciones + grupo-ejec tokenizado

Transiciones, focus-visible, caret de consola, flash one-shot de registros, hover
de breakpoint, grupo-ejec con var(--radio). Tests: el caret/flash son CSS
(verificación visual); si hay lógica nueva (p. ej. estructura del prompt de
consola), testear lo testeable.
**Compuerta F2:** microinteracciones visibles y NO molestas (one-shot, sin bucles
que distraigan a un principiante); focus-visible coherente por tema; tsc/build;
suite intacta. Commit `rp19-fase-2: microinteracciones y grupo-ejec tokenizado`.

## Fase 3 — Refinados de ruido + wordmark

Refinados (th, xn, memoria .cero, sin cebra, separadores suaves); wordmark SVG en
toolbar (bi-tono por tokens). **Compuerta F3:** wordmark se adapta al tema (cambia
de color con el acento/texto); ruido visual reducido como el mock; contraste AA
intacto; tsc/build. Commit `rp19-fase-3: refinados de ruido visual y wordmark`.

## Fase 4 — Docs y cierre

README/Ayuda (mención de la nueva estética, el wordmark); BITACORA (tokens
nuevos, licencia IBM Plex, decisión Cálido-único, ajustes de contraste si los
hubo). ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB + verificación visual final de los 5
temas contra el mock. Commit `rp19-fase-4: docs estética Cálido`.

## Criterios de aceptación

- La app se ve como el mock (estética Cálido aplicada siempre, sin toggle). ✓ F1-F3.
- IBM Plex Sans self-hosted (licencia registrada), bundle ≤30 MB. ✓ F1.
- Educativo con acento violeta cumpliendo AA. ✓ F1.
- Microinteracciones del mock (caret, flash one-shot, focus, hover bp). ✓ F2.
- grupo-ejec tokenizado; refinados de ruido; wordmark adaptable al tema. ✓ F2-F3.
- 5 temas siguen AA; ids/registros/Rust intactos. ✓ todas.
- Modales/ayuda/about: NO acá (RP-20). ✓.
