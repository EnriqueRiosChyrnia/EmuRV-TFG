# PLAN-RP17 Parte B — Temas nuevos + theme de sintaxis del editor

Continuación de RP-17 (Parte A DOCUMENTADA: tokens base, mecanismo data-tema +
selector + persistencia, Educativo + Retro-CRT). Esta Parte B agrega los temas
del diseño aprobado. Plan por fases con compuertas: PROHIBIDO avanzar con la
compuerta en rojo.

**Fuente:** diseño aprobado por el autor (claude.ai), preservado en
`docs/diseno-ui/propuestas-ui.html`. Decisión del autor (2026-06-13): la estética
«Cálido» será el ÚNICO look (eso se implementa en RP-19; ESTE plan NO toca la
estética, solo agrega temas como conjuntos de tokens).

## Contexto (anclas)

- `src/temas/tokens.css`: `:root`/`[data-tema="educativo"]` (default) +
  `[data-tema="retro"]` (Retro-CRT), ambos con contraste AA verificado por
  `src/temas/contraste.test.ts`. Mecanismo data-tema + selector + localStorage
  `emurv.tema.v1` ya andando (Parte A).
- Lista de temas en el TS (TEMAS) + claves `tema.*` en es.json/en.json (el
  anti-drift de RP-16 exige ambos idiomas).
- Editor: CodeMirror 6 con tema oneDark FIJO (`src/editor/index.ts`). El syntax
  highlighting NO sigue los temas de la app hoy — ese es el punto técnico de F2.

## Alcance

- SÍ: 3 temas nuevos (Pergamino, Retro Kawaii, Disquete 90s) como bloques de
  tokens; tokens de sintaxis `--sx-*` por tema; theme de CodeMirror que siga esas
  variables (highlight por clases, no colores fijos); contraste AA de TODOS los
  temas (5 en total); selector y i18n actualizados.
- NO: la estética Cálido / radio / IBM Plex / sombras (RP-19); cambiar el acento
  del Educativo a violeta (eso es parte de Cálido → RP-19); tema de usuario
  (futuro). Retro-CRT existente se MANTIENE (no se borra).

## Diseño acordado (tokens exactos del diseño — no re-litigar)

Pegar en `tokens.css` como bloques nuevos (valores del diseño aprobado):

```css
[data-tema="pergamino"] {            /* claro, papel cálido */
  --fondo:#f4ecd8; --fondo-panel:#efe6cf; --fondo-claro:#e6dbc0; --borde:#cbbd98;
  --texto:#352c1f; --texto-suave:#5f5544; --texto-sobre-acento:#ffffff;
  --acento:#1f5c9e; --ok:#2e7d32; --error:#b3261e; --cambio:#8a5d00;
  --resaltado-linea:rgba(31,92,158,.14); --resaltado-cambio:rgba(138,93,0,.14);
  --fila-par:rgba(0,0,0,.035); --overlay-fondo:rgba(40,33,20,.4);
  --sx-com:#8a7f63; --sx-dir:#2e7d32; --sx-lbl:#1f5c9e; --sx-ins:#8a3ca6; --sx-str:#3f7a2e; --sx-num:#9a5b00;
}
[data-tema="kawaii"] {               /* vaporwave pastel; candy-pink AA */
  --fondo:#efe7fb; --fondo-panel:#faf5ff; --fondo-claro:#ece3fa; --borde:#d3bfee;
  --texto:#3f2d63; --texto-suave:#6f5c8e; --texto-sobre-acento:#ffffff;
  --acento:#c0327d; --ok:#1f9e6e; --error:#d83a5e; --cambio:#b97400;
  --resaltado-linea:rgba(192,50,125,.16); --resaltado-cambio:rgba(185,116,0,.16);
  --fila-par:rgba(123,91,174,.05); --overlay-fondo:rgba(63,45,99,.4); --fondo-pantalla:#1c1430;
  --sx-com:#9683ba; --sx-dir:#1f8a5a; --sx-lbl:#b8327f; --sx-ins:#6a35c0; --sx-str:#2f8a4a; --sx-num:#c2691f;
}
[data-tema="disquete"] {             /* escritorio beige 90s */
  --fondo:#cdc3a8; --fondo-panel:#e9e2cf; --fondo-claro:#f4efdf; --borde:#8f8164;
  --texto:#2b2719; --texto-suave:#6a6047; --texto-sobre-acento:#fff4e8;
  --acento:#b14a14; --ok:#3c7a3f; --error:#b23022; --cambio:#9a6b00;
  --resaltado-linea:rgba(177,74,20,.16); --resaltado-cambio:rgba(154,107,0,.16);
  --fila-par:rgba(80,70,40,.05); --overlay-fondo:rgba(43,39,25,.45); --fondo-pantalla:#15120a;
  --sx-com:#8a7d5e; --sx-dir:#3c7a3f; --sx-lbl:#b14a14; --sx-ins:#6d3fb0; --sx-str:#2f7a3a; --sx-num:#9a5b00;
}
```

Tokens `--sx-*` (sintaxis) que TODOS los temas definen — agregar también a
`:root`/Educativo y a Retro-CRT los valores del diseño:
`:root` → `--sx-com:var(--texto-suave); --sx-dir:#7fcf8f; --sx-lbl:#6fb3ff;
--sx-ins:#c98cff; --sx-str:#9ec97f; --sx-num:#e0a36b;` (para Retro-CRT, derivar
en tonos ámbar coherentes y verificar contraste).

## Caveats de contraste (el test los caza — corregir el TOKEN, no bajar la vara)
El diseño marcó riesgos a vigilar en `contraste.test.ts`: Pergamino `--cambio`
(#8a5d00) y `--texto-suave` sobre `--fondo-panel`; Kawaii y Disquete son claros
(verificar texto sobre fondos claros). TODOS los pares críticos (texto/fondo de
editor, registros, memoria, consola, error) deben dar ≥ 4.5:1. Si uno raspa,
oscurecer/aclarar un escalón ese token.

## Fase 1 — Los 3 temas (tokens) + selector + i18n + contraste

Agregar los 3 bloques a tokens.css; sumar "pergamino"/"kawaii"/"disquete" a
TEMAS; claves `tema.pergamino`/`tema.kawaii`/`tema.disquete` en es.json Y en.json;
extender `contraste.test.ts` para cubrir los 5 temas en sus pares críticos.
**Compuerta F1:** test de contraste verde para los 5 temas; selector muestra los
5; anti-drift i18n es/en verde; tsc/build; verificación visual de cada tema
(BITACORA). Commit `rp17b-fase-1: temas Pergamino, Retro Kawaii y Disquete 90s`.

## Fase 2 — Theme de sintaxis del editor (sigue los tokens)

Hoy CodeMirror usa oneDark fijo → el código NO cambia de color con el tema.
Objetivo: el highlight del editor usa los `--sx-*` del tema activo.
- Enfoque: HighlightStyle de CM6 que emita CLASES (no colores inline) — p. ej.
  tags comment→`.cm-sx-com`, keyword→`.cm-sx-ins`, string→`.cm-sx-str`,
  number→`.cm-sx-num`, labelName/def→`.cm-sx-lbl`, directiva→`.cm-sx-dir` — y esas
  clases en CSS usan `color:var(--sx-*)`. Reemplazar/complementar oneDark por un
  theme base neutro (fondo/selección/gutter por tokens de la app) + este
  HighlightStyle. (Si el mapeo tag→clase de CM resulta espinoso, hacer un
  mini-spike y registrarlo; NO dejar el editor sin highlight.)
- El resultado: cambiar de tema recolorea TAMBIÉN el código del editor, en vivo.
**Compuerta F2:** el editor recolorea al cambiar tema (verificación visual en los
5 temas, BITACORA); resaltado de línea/breakpoints/gutter intactos; tsc/build;
suite Rust intacta. Commit `rp17b-fase-2: theme de sintaxis del editor por tokens`.

## Fase 3 — Docs y cierre

tokens.css: actualizar la nota «cómo agregar un tema» (ya contempla el patrón);
Ayuda: el texto de temas menciona los disponibles (RP-19/RP-20 rehará el modal,
acá solo el dato); BITACORA (decisiones, contraste por tema, enfoque del theme
CM). README si lista temas. ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F3 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB (la fuente IBM Plex es de RP-19, acá
no entra). Commit `rp17b-fase-3: docs temas nuevos`.

## Criterios de aceptación

- 3 temas nuevos seleccionables, persistidos, contraste AA. ✓ F1.
- Retro-CRT y Educativo intactos (5 temas en total). ✓ F1.
- El editor recolorea con el tema (syntax highlight por tokens). ✓ F2.
- i18n es/en de los nombres (anti-drift verde). ✓ F1.
- Estética Cálido / acento violeta Educativo / fuente UI: NO acá (RP-19). ✓.
