# PLAN-RP17 — Sistema de temas / skins

Punto RP-17 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

**Orden acordado con el autor (2026-06-13):** primero la BASE (Parte A: tokens +
fuente + iconos + mecanismo de temas con 2 built-in), ejecutable YA sin
decisiones de diseño; los temas experimentales y el tema de usuario (Parte B)
vienen DESPUÉS, cuando el autor traiga inspiración y la sesión coordinadora arme
el prompt para Claude Desktop/Design. Este plan cubre la PARTE A. La Parte B se
planificará aparte sobre el mecanismo que deja la Parte A.

## Contexto (anclas en código)

- `src/styles.css` (~530 líneas) con algunas variables propias (`--fondo`,
  `--acento`, …) pero la mayoría de colores/medidas hardcodeados.
- Frontend vanilla TS + Vite; CLAUDE.md veta frameworks pesados (sin React/Vue/
  Tailwind-runtime). Todo CSS puro.
- Persistencia local ya usada en RP-15 (lecciones) y RP-16 (idioma): localStorage
  con clave versionada — mismo patrón para el tema.
- RNF-04 (≤30 MB) intocable: la fuente self-hosted (~100 KB) e iconos SVG inline
  (cero dep) caben de sobra; el bundle se re-mide igual.

## Alcance (Parte A)

- SÍ: refactor de toda la estética a design tokens CSS (apariencia IDÉNTICA a
  hoy — refactor invisible); fuente monoespaciada self-hosted; iconos SVG inline
  en la toolbar; mecanismo de temas (selector + persistencia) con 2 temas
  built-in: «Educativo» (= el look actual, DEFAULT) y «Retro-CRT» (identidad del
  dominio: fósforo/ámbar, scanlines sutiles — no necesita inspiración externa,
  nace del framebuffer); checklist de legibilidad; docs.
- NO (Parte B, futuro): temas experimentales adicionales (vienen del diseño con
  Claude Desktop), editor/carga de tema de usuario, gamificación visual.

## Diseño acordado (no re-litigar)

### Design tokens (la llave)
- `src/temas/tokens.css` NUEVO (o sección dedicada en styles.css): TODAS las
  variables de la estética — colores (fondo, superficie, texto, texto-tenue,
  acento, acento-2, borde, foco, éxito/error/aviso), tipografía (familia mono,
  tamaños, interlineado), espaciado, radios, sombras, y los colores específicos
  de los paneles (registro resaltado, breakpoint, línea en ejecución, watchpoint,
  consola). styles.css deja de tener literales: todo referencia `var(--…)`.
- **Refactor INVISIBLE:** al terminar F1, la app se ve EXACTAMENTE igual que
  antes — los tokens toman los valores actuales. Es la prueba de la compuerta
  (comparación visual; si algo cambió de aspecto, un literal quedó suelto).
- Un «tema» = un conjunto de valores de esos tokens aplicado vía un atributo en
  `<html>` (p. ej. `data-tema="retro"`) que sobrescribe las variables. Cero CSS
  duplicado por tema.

### Tema por atributo + selector
- `data-tema` en el root; cada tema = un bloque `[data-tema="x"] { --… }`.
  «Educativo» = los valores por defecto (`:root`). Cambiar tema = cambiar el
  atributo + persistir en localStorage (`emurv.tema.v1`); sin recargar (las
  variables CSS aplican en vivo — ventaja sobre el idioma que sí recarga).
- Selector en toolbar/menú (junto al de idioma), i18n de los nombres.

### Fuente e iconos (mejoras de base)
- Fuente mono self-hosted (JetBrains Mono o Commit Mono, licencia OFL/libre —
  registrar cuál y su licencia en BITACORA): archivos en `src/assets/fonts/`,
  `@font-face`, token `--fuente-mono`. Editor + registros + memoria + consola la
  usan. Fallback a monospace del sistema.
- Iconos SVG inline (Lucide/Phosphor, licencia ISC/MIT — copiar los SVG, CERO
  dependencia npm) en los botones de la toolbar, conservando el texto (o
  texto+icono; los botones hoy son solo texto). Accesibilidad: `aria-label`.

### Regla dura de legibilidad (innegociable)
- Checklist de contraste para CADA tema sobre los pares críticos: texto del
  editor/fondo, registros, memoria, consola, mensajes de error. Mínimo WCAG AA
  (4.5:1) para texto normal. Si un tema no llega, se corrige el tema, no se
  baja la vara. Si es barato, test que calcula el ratio de contraste de los
  pares críticos de cada tema y falla bajo 4.5:1.

## Fase 1 — Design tokens (refactor invisible)

Extraer toda la estética a tokens; styles.css sin literales de color/medida;
apariencia idéntica. **Compuerta F1:** comparación visual antes/después =
idéntica (capturas en BITACORA si el flujo de capturas aplica); tsc/build
limpios; suite Rust intacta; bundle ≤ 30 MB.
Commit `rp17-fase-1: design tokens (refactor invisible de la estética)`.

## Fase 2 — Fuente mono + iconos SVG

Fuente self-hosted + @font-face + token; iconos SVG inline en toolbar con
aria-label. **Compuerta F2:** licencia de fuente registrada; bundle re-medido
≤ 30 MB; tsc/build limpios; verificación visual (BITACORA).
Commit `rp17-fase-2: fuente monoespaciada self-hosted e iconos SVG`.

## Fase 3 — Mecanismo de temas + Educativo + Retro-CRT

`data-tema` + selector + persistencia (localStorage emurv.tema.v1, patrón
RP-15/16); tema «Educativo» (default, = actual) y «Retro-CRT»; checklist (y test
si es barato) de contraste AA en AMBOS. Tests: función pura de tema
(set/get/persist); el contraste de los pares críticos cumple AA en los dos temas.
Verificación manual: cambiar de tema en vivo sin recargar, persiste al reabrir
(BITACORA). **Compuerta F3:** tests verdes + contraste AA verificado + tsc/build
+ bundle ≤ 30 MB. Commit `rp17-fase-3: mecanismo de temas con Educativo y Retro-CRT`.

## Fase 4 — Docs y cierre de la Parte A

README (temas + cómo cambiarlos), Ayuda (selector de tema), BITACORA
(arquitectura de tokens, licencias de fuente/iconos, decisiones de contraste),
y una NOTA para la Parte B: cómo agregar un tema nuevo = un bloque
`[data-tema="x"]` de tokens (para que el diseño con Claude Desktop se integre sin
reingeniería). ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F4 (cierre Parte A):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB.
Commit `rp17-fase-4: docs sistema de temas (Parte A)`.

## Criterios de aceptación (Parte A)

- Estética 100% por tokens; refactor invisible verificado. ✓ F1.
- Fuente mono self-hosted + iconos SVG, licencias registradas, bundle OK. ✓ F2.
- Mecanismo de temas con ≥2 built-in, default Educativo, cambio en vivo,
  persistencia. ✓ F3.
- Legibilidad AA en todos los temas (checklist + test si barato). ✓ F3.
- Agregar un tema nuevo = un bloque de tokens (documentado para la Parte B). ✓ F4.
- Sin frameworks; CSS puro. ✓.

## Parte B (NO en este plan — se planifica después)

- Prompt para Claude Desktop/Design: análisis del proyecto + imágenes de
  inspiración del autor → propuestas de temas experimentales (conjuntos de
  tokens). La coordinadora lo arma cuando la Parte A cierre.
- Temas experimentales = agregar bloques `[data-tema="x"]` (datos, no código).
- Tema de usuario: editor de tokens en la UI o carga de un JSON/CSS de tema.
