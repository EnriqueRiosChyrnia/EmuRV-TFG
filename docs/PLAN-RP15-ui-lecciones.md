# PLAN-RP15 — UI de lecciones (navegador del currículo)

Punto RP-15 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

**Prerrequisito:** RP-9 DOCUMENTADO (las 10 lecciones existen en `lecciones/`).

## Contexto (anclas en código)

- La UI embebe los ejemplos con `?raw` de Vite + selector `EJEMPLOS` (main.ts:32-45)
  — patrón ya probado para texto-a-editor; sin sesión guardada (`null` buffer).
- `lecciones/01-registros/leccion.s` … `10-traps/leccion.s` + `08-multiarchivo/`
  (main.s + utiles.s + constantes.s) + `lecciones/README.md` (mapa de aprendizaje).
- **Cabecera de plantilla FIJA (RP-9), parseable:** cada `leccion.s` abre con
  `# Lección NN — TÍTULO`, `# OBJETIVOS —`, `# PANELES PARA OBSERVAR:`,
  `# CÓMO USARLA:`, y al final `# EJERCICIOS`. Esa estructura es la fuente única
  de los metadatos del navegador (no duplicar en un índice aparte → anti-drift
  por test, lección de RP-14).
- Caso especial **08-multiarchivo:** usa `.include`; RP-5 fijó que `.include`
  sin archivo guardado en disco da error didáctico (necesita base_dir). El
  navegador debe resolverlo (ver diseño).
- Progreso: localStorage del webview (frontend puro, sin tocar el backend Rust).

## Alcance

- SÍ: panel/navegador de lecciones en la UI (mapa, título, objetivos visibles
  antes de abrir), abrir lección al editor, progreso completada/persistido,
  navegación siguiente/anterior, manejo del caso multi-archivo, docs.
- NO: editor de lecciones del usuario (crear lecciones nuevas), gamificación
  (puntos/insignias — propuesta separada), verificación automática de ejercicios
  (autograding — eso es el «modo docente» pospuesto), sincronización en la nube.

## Diseño acordado (no re-litigar)

### Fuente de datos (anti-drift, patrón RP-14)
- Lecciones embebidas con `?raw` (las 10; la 08 embebe sus 3 archivos).
- `src/lecciones/index.ts` NUEVO: un parser `parseLeccion(texto) -> { numero,
  titulo, objetivos[], paneles, multiarchivo: bool }` que lee la cabecera de
  plantilla fija + el array ordenado de las 10. CERO metadatos duplicados a mano
  — todo sale del `.s`. Test: las 10 lecciones parsean con título y ≥1 objetivo
  (si una no cumple la plantilla, el test rojo lo caza — anti-drift).

### Navegador (UI)
- Panel/selector «Lecciones» SEPARADO del de «Ejemplos» (son cosas distintas:
  ejemplos = programas sueltos; lecciones = currículo ordenado). Muestra la lista
  ordenada con número + título; al seleccionar una, un encabezado muestra sus
  OBJETIVOS y qué PANELES mirar (extraídos por el parser) ANTES de cargar el
  código — así el estudiante sabe a qué entra.
- «Abrir lección» → carga el `leccion.s` al editor (buffer sin ruta, como los
  ejemplos), salvo el caso multi-archivo (abajo).
- i18n: rótulos del navegador en es.json (los títulos/objetivos de las lecciones
  ya están en español dentro del `.s` — no se traducen acá; RP-16 decidirá si la
  UI del navegador se internacionaliza, el CONTENIDO de lecciones es material
  didáctico aparte).

### Progreso y navegación
- Marcar «completada» (checkbox/botón por lección) persistido en localStorage
  (clave versionada, p. ej. `emurv.lecciones.v1`); indicador visual de
  completadas en la lista. Reset disponible.
- Navegación «siguiente/anterior» dentro del flujo: tras abrir la lección N,
  botones para ir a N±1 (respetando el orden del currículo, no el alfabético).

### Caso multi-archivo (08)
- Al abrir la 08, el navegador NO puede usar el buffer embebido simple (`.include`
  necesita disco). Solución coherente con RP-5: ofrecer «Guardar esta lección en
  una carpeta…» → escribe main.s + utiles.s + constantes.s a la carpeta elegida
  y abre main.s CON esa ruta base (el comando assemble ya acepta base_dir desde
  RP-5). Si el usuario cancela, mostrar aviso didáctico explicando por qué esta
  lección necesita guardarse. Documentar el flujo en la propia lección/Ayuda.

## Fase 1 — Embeber + parser + índice (fuente única)

Embeber las 10 (08 con sus 3 archivos) + `src/lecciones/index.ts` con parser y
array ordenado. Test UI (tsx): las 10 parsean título + objetivos; el orden del
currículo es el correcto; la 08 marca multiarchivo=true.
**Compuerta F1:** test verde + tsc limpio + suite Rust intacta.
Commit `rp15-fase-1: lecciones embebidas, parser de cabecera y índice`.

## Fase 2 — Navegador en la UI

Panel «Lecciones» separado de ejemplos: lista ordenada, encabezado con objetivos/
paneles al seleccionar, «Abrir lección» carga al editor. i18n. tsc limpio.
Verificación manual (BITACORA): abrir 3-4 lecciones, objetivos visibles correctos.
**Compuerta F2:** tsc + suite Rust intacta + flujo manual anotado.
Commit `rp15-fase-2: navegador de lecciones`.

## Fase 3 — Progreso, navegación y multi-archivo

localStorage de completadas + indicador + reset; siguiente/anterior por orden de
currículo; caso 08 (guardar a carpeta + abrir con base_dir, o aviso didáctico si
cancela). Tests: funciones puras de progreso (marcar/leer/reset dado un estado)
y de navegación (dado N → N±1 en orden de currículo, bordes en 01 y 10).
Verificación manual del caso 08 (BITACORA).
**Compuerta F3:** tests verdes + tsc + caso 08 verificado a mano.
Commit `rp15-fase-3: progreso, navegación y caso multi-archivo`.

## Fase 4 — Docs y cierre

README (sección «Aprendé con EmuRV» ahora apunta al navegador, no solo a la
carpeta), Ayuda integrada (cómo usar el navegador de lecciones), lecciones/README
(nota de que también se acceden desde la app), BITACORA. ROADMAP-PRO y Cap. VI:
NO tocar.
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB.
Commit `rp15-fase-4: docs navegador de lecciones`.

## Criterios de aceptación (del ROADMAP)

- Panel/menú de lecciones con el mapa de aprendizaje; abrir con objetivos
  visibles. ✓ F2.
- Progreso persistido localmente; navegación siguiente/anterior. ✓ F3.
- Sin gamificación en v1 (registrada como propuesta si el autor la quiere). ✓.
- Metadatos del navegador SIN duplicar el contenido de las lecciones (parser +
  anti-drift por test — patrón RP-14). ✓ F1.
- Caso multi-archivo resuelto coherente con RP-5 (base_dir). ✓ F3.
