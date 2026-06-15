# PLAN-RP30 — Pestañas + archivos recientes

Punto RP-30 de `docs/ROADMAP-PRO.md`. EL ÚLTIMO del roadmap y el de mayor riesgo
en frontend: convierte el modelo **un-solo-documento** en **multi-documento**.
Worktree propio (`rp30/pestanas`). Plan por fases con compuertas: PROHIBIDO
avanzar con la compuerta en rojo. Si el spike revela que el alcance es
desproporcionado, FRENAR y reportar (regla de conflictos) — un cierre con
decisión documentada (p. ej. pestañas sin per-doc xlen) también es válido.

**Decisión del autor:** abrir varios archivos a la vez (pestañas) + lista de
recientes, como un IDE profesional. RP-25 (indicador «•») y RP-27 (persistir
workspace) ya existen y deben SEGUIR funcionando, ahora por pestaña.

## Contexto (anclas verificadas)

- `src/main.ts` `class App` es single-doc: campos `editor` (1 `EditorView`),
  `rutaArchivo`, `docLimpio`, `programa`, `programaDesactualizado`, `modoBinario`,
  `bannerBinario`. Todo asume UN documento.
- RP-25: `actualizarTitulo()` (título con «•»), `hayCambiosSinGuardar()`,
  aviso al cerrar en `configurarCierreVentanas()`.
- RP-27: `src/workspace.ts` (clave `emurv.workspace.v1`, `{contenido, rutaArchivo}`),
  `restaurarWorkspace()`/`persistirWorkspace()`/`programarGuardadoWorkspace()`
  (debounce 400 ms). HOY persiste UN buffer.
- RP-32: `src/arquitectura.ts` `leerXlen()/setXlen()` (`emurv.xlen.v1`), GLOBAL.
- Backend es de UNA sesión: `api.assemble(...)` crea/reemplaza la sesión activa.
  Solo un programa corre a la vez.
- Patrón de persistencia del repo: módulo por dominio, clave versionada
  `emurv.X.vN`, función de parseo PURA testeable + wrappers tolerantes
  (`src/lecciones/progreso.ts`, `workspace.ts`).
- HTML: `index.html` `<header id="toolbar">` + `<main id="editor">` (anclar la
  barra de pestañas entre ambos).
- NO existe "recientes" hoy.

## Alcance

- SÍ: varias pestañas, cada una con su buffer (contenido, ruta, estado limpio,
  cursor/scroll, modo binario). Abrir, cambiar, cerrar (con aviso «•» si hay
  cambios), nueva pestaña, «+». Indicador «•» POR pestaña. Lista de archivos
  recientes persistida, accesible desde el menú Archivo. Persistir TODAS las
  pestañas + la activa entre sesiones (workspace v2, migrando v1).
- NO (salvo que el spike diga que es barato): xlen por pestaña (se mantiene
  GLOBAL como hoy; registrar per-doc como futuro), arrastrar para reordenar
  pestañas, split view, múltiples sesiones de emulación simultáneas (el backend
  es de una sesión: solo la pestaña activa interactúa con ejecutar/paso/depurar).

## Diseño acordado (el QUÉ; la estrategia del editor la FIJA el spike F1)

- **Modelo `Documento`** (`src/documento.ts` o dentro de un `Pestanas`): por
  pestaña `{ id, rutaArchivo, docLimpio, modoBinario, programa,
  programaDesactualizado, estadoEditor }`. El "nombre" de la pestaña se deriva de
  `rutaArchivo` (basename) o «Sin título» (reusar `titulo.sinTitulo` de RP-25).
- **Editor:** decisión del spike — preferir UN `EditorView` reutilizado con un
  `EditorState` por documento (CM6 permite `view.setState(...)`; preserva undo,
  cursor y scroll por doc y es más liviano que N vistas). Alternativa: N vistas.
  Elegir y fundar en BITACORA.
- **Sesión de backend ↔ pestaña activa:** solo la pestaña activa ensambla/ejecuta.
  Al cambiar de pestaña con una ejecución viva: detener o bloquear (definir; lo
  más simple y seguro: no permitir cambiar de pestaña mientras `corriendo`/
  `esperando`, o detener al cambiar). Cambiar de pestaña invalida el estado de
  ejecución de la nueva si su `programa` no corresponde a la sesión actual:
  re-ensamblar (mismo criterio que `programaDesactualizado`).
- **`actualizarTitulo()` (RP-25):** el título de la VENTANA refleja la pestaña
  activa; además cada pestaña pinta su propio «•».
- **Recientes** (`src/recientes.ts`): clave `emurv.recientes.v1`, lista de rutas
  (máx. N, p. ej. 10, sin duplicados, más reciente primero), parseo PURO
  testeable + wrappers tolerantes. Se añade al abrir/guardar un archivo con
  ruta. UI en el menú Archivo (submenú/lista). i18n es/en.
- **Workspace v2** (`workspace.ts`): subir a `emurv.workspace.v2` =
  `{ pestanas: Documento[], activa: number }`; MIGRAR v1 (`{contenido,rutaArchivo}`)
  a una sola pestaña. Mantener `parsearWorkspace` puro + tests (v1 y v2).
  `restaurar/persistir` operan sobre todas las pestañas (debounce conservado).

## Fase 1 — Spike: estrategia de editor + modelo multi-doc

`src` (prototipo mínimo o nota razonada): confirmar `EditorView.setState` para
swap de documentos preservando undo/cursor/scroll; decidir 1-vista-N-estados vs
N-vistas; bosquejar `Documento` y cómo la sesión de backend sigue a la pestaña
activa; decidir migración workspace v1→v2 y si xlen sigue global. DECIDIR en
BITACORA [Seguro]. Si el costo del swap fiel resulta alto, registrar el
fallback. **Compuerta F1:** decisión fundada + suite actual intacta. Commit
`rp30-fase-1: spike pestanas (estrategia de editor + modelo)`.

## Fase 2 — Infraestructura de pestañas (1 pestaña = comportamiento de hoy)

Modelo `Documento` + gestor de pestañas + barra de pestañas en el HTML (entre
toolbar y editor) con nombre, «•» (RP-25) y botón cerrar, y un «+». Con UNA sola
pestaña, TODO se comporta idéntico a hoy (ensamblar/ejecutar/paso/depurar/
guardar/abrir/nuevo/binario). Refactor de los campos doc-céntricos de `App` a
`Documento` activo, sin cambiar comportamiento observable.
**Compuerta F2:** tsc + suite UI verde (comportamiento single-tab idéntico) +
abrir la app muestra 1 pestaña funcional. Commit
`rp30-fase-2: infraestructura de pestanas (paridad single-tab)`.

## Fase 3 — Multi-pestaña: abrir/cambiar/cerrar/nueva

Abrir archivo → nueva pestaña (o foco si ya está abierto). Cambiar pestaña
preserva contenido/cursor/scroll/«•»/ruta/modo binario. Cerrar pestaña con aviso
«•» (reusar `archivo.descartarConfirm`/`cerrarConfirm` o clave nueva). «Nuevo» y
«+» abren pestaña en blanco. La sesión de backend sigue a la activa (definido en
F1). i18n es/en (nueva pestaña, cerrar pestaña, etc.).
**Compuerta F3:** tsc + suite + verificación: abrir 2-3 archivos, alternar sin
perder estado, cerrar con aviso, ejecutar en la activa. Commit
`rp30-fase-3: abrir/cambiar/cerrar pestanas`.

## Fase 4 — Archivos recientes + persistencia de workspace v2

`src/recientes.ts` (parseo puro + tests) + entrada en el menú Archivo. `workspace`
v2 (todas las pestañas + activa) con migración v1→v2; tests de `parsearWorkspace`
para v1 y v2. Al reabrir la app se restauran las pestañas y la activa; el menú
Archivo lista los recientes (abre en pestaña).
**Compuerta F4:** tsc + suite (incluye tests recientes + workspace v1/v2) +
verificación: cerrar con 2 pestañas y reabrir las restaura; recientes funciona.
Commit `rp30-fase-4: archivos recientes + workspace v2`.

## Fase 5 — Docs, i18n y cierre

Documentar pestañas/recientes (donde viva la doc de UI / ayuda si aplica);
BITACORA consolidada; revisar que RP-25 y RP-27 quedan descritos como
"por pestaña". i18n es/en paridad. NO tocar ROADMAP-PRO ni Cap. VI.
**Compuerta F5 (cierre):** cargo test + clippy AMBOS crates (no deberían cambiar,
confirmar intactos) + tsc + ci-local --skip-bundle + tauri build + bundle ≤30 MB
+ verificación manual (varias pestañas, recientes, persistencia, «•» por
pestaña, ejecutar en la activa). Commit `rp30-fase-5: docs de pestanas y recientes`.

## Criterios de aceptación

- Varias pestañas abiertas, cambiar sin perder estado, cerrar con aviso «•». ✓ F2-F3.
- «+»/Nuevo abre pestaña en blanco; abrir archivo abre/enfoca pestaña. ✓ F3.
- Indicador «•» por pestaña (RP-25 extendido). ✓ F2-F3.
- Lista de archivos recientes persistida y usable desde el menú Archivo. ✓ F4.
- Persistencia de workspace v2 (todas las pestañas + activa), migrando v1. ✓ F4.
- Con 1 pestaña, comportamiento 100% idéntico al actual; toda la suite verde;
  Rust intacto. ✓ todas.
- Si el spike revela alcance desproporcionado, FRENAR y reportar para trocear o
  reconsiderar (p. ej. recientes sin persistencia de pestañas) — cierre con
  decisión documentada también es válido. ✓ F1.
