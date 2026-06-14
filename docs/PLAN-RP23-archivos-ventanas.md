# PLAN-RP23 — Gestión de archivos + cierre de ventanas hijas

Punto RP-23 de `docs/ROADMAP-PRO.md`. Dos mejoras de UX pedidas por el autor.
Plan por fases con compuertas: PROHIBIDO avanzar con la compuerta en rojo.

## Pedidos del autor

1. Al cerrar la ventana PRINCIPAL (la X), si hay paneles/pantalla DESACOPLADOS,
   que se cierren también (hoy quedan huérfanos).
2. Gestión de archivos tipo IDE en un MENÚ DESPLEGABLE «Archivo ⌄» que contenga
   Nuevo, Abrir, Guardar y Guardar como (en vez de botones sueltos). Incluye
   «Guardar como» nuevo y «Guardar» inteligente.

## Contexto (anclas verificadas)

- Ventanas secundarias: labels `panel-registros|panel-memoria|panel-depuracion`
  (desacople.ts, `WebviewWindow`) y `pantalla` (video.ts). NO hay manejo de
  cierre de la principal → quedan abiertas.
- Toolbar (main.ts ~169): helper `boton(id, claveI18n, clase, parent)` que arma
  `<button>` con icono SVG (`icono(id)`) + texto + aria-label. Archivo hoy:
  `abrir`(209) / `guardar`(210) / `exportar`(211), todos `.deemph`, seguidos.
- `guardarArchivo()` (747): SIEMPRE abre el diálogo `save` — o sea hoy «Guardar»
  ya se comporta como «Guardar como», ignorando `this.rutaArchivo`.
- `this.rutaArchivo: string|null` = ruta del archivo actual (base para `.include`,
  RP-5). Se setea al abrir/guardar; null en ejemplos/lecciones embebidos.
- i18n: claves en es.json Y en.json (anti-drift de RP-16 obliga ambos).
- El diseño del mock (docs/diseno-ui/) NO tenía Nuevo/Guardar-como: son
  ADICIONES; hacerlas coherentes con la estética Cálido existente (deemph,
  iconos Lucide), no hay conflicto de fidelidad.

## Alcance

- SÍ: cerrar ventanas hijas al cerrar la principal; botón Nuevo; botón Guardar
  como; «Guardar» inteligente (ruta actual sin re-preguntar; si no hay, pide);
  reagrupar la sección de archivo en la toolbar; iconos + i18n es/en; docs.
- NO: pestañas/multi-documento; historial de recientes; autoguardado;
  persistir sesión entre arranques (futuro si se quiere).

## Diseño acordado (no re-litigar)

### Fase 1 — Cerrar ventanas hijas al salir
- En main, al construir, registrar `getCurrentWindow().onCloseRequested(async ...)`:
  antes de que la principal cierre, cerrar TODAS las webviews secundarias
  (`getAllWebviewWindows()` filtrando la `main`, o cerrar por labels conocidas
  panel-*/pantalla). Usar la API `@tauri-apps/api/webviewWindow` / `window`.
  Verificar que NO bloquee el cierre de la principal (cerrar las hijas y dejar
  seguir el cierre). Si resulta más robusto en Rust (window event en lib.rs),
  evaluarlo, pero intentar frontend primero (RP-22 fue todo frontend).
- Caso: paneles desacoplados en modo MOVER (RP-22) — al cerrarlos por este
  camino NO hace falta restaurarlos al lateral (la app entera se cierra).

### Fase 2 — Nuevo / Guardar como / Guardar inteligente + reagrupar
- **Guardar inteligente** (`guardarArchivo`): si `this.rutaArchivo != null` →
  guardar directo ahí (`api.guardarArchivo(this.rutaArchivo, getDoc(...))`) sin
  diálogo; si null → comportarse como «Guardar como».
- **Guardar como** (`guardarComo`): siempre diálogo `save`; al confirmar, guarda
  y actualiza `this.rutaArchivo`. Extraer la lógica común con guardarArchivo.
- **Nuevo** (`nuevoArchivo`): salir de modo binario, editor a un buffer en blanco
  (o plantilla mínima: comentario + `.text`/`main:` — a elección, registrar),
  `this.rutaArchivo = null`, `this.programa = null`, estado «sin programa»,
  limpiar consola/errores. SI hay cambios sin guardar conviene un `confirm()`
  didáctico antes de descartar — incluirlo (los IDEs lo hacen; barato y evita
  perder trabajo). Detección de «sucio» mínima: comparar doc actual vs último
  guardado/cargado, o un flag; si es caro, un confirm incondicional «¿Descartar
  el programa actual?» es aceptable v1.
- **Toolbar — menú desplegable «Archivo»** (pedido del autor, 2026-06-13):
  en vez de 4 botones sueltos, UN control «Archivo ⌄» que al hacer click abre un
  menú desplegable (popover anclado al botón) con: Nuevo · Abrir · Guardar ·
  Guardar como. «Exportar ELF» queda APARTE (no entra al menú: produce binario,
  no gestiona el fuente). El control «Archivo» usa el mismo estilo de los otros
  desplegables de la barra (`.sel` con ⌄, como Ejemplos/Tema/Idioma) para
  coherencia. El popover: lista de ítems con icono Lucide + texto, cada uno
  dispara su acción y cierra el menú; cerrar al hacer click afuera y con Escape;
  accesible (foco/aria). Si NO existe ya un patrón de popover reutilizable en el
  código, crear uno simple y genérico (podría servir luego a Tema/Idioma);
  registrar la decisión. Iconos Lucide para `nuevo` (file-plus), `guardar-como`
  (variante de save), y para los ítems Abrir/Guardar (reusar los que ya existen)
  — paths SVG copiados, cero dep (patrón RP-17/19).
- i18n: claves nuevas `toolbar.nuevo`, `toolbar.guardarComo`,
  `archivo.descartarConfirm` (y las que hagan falta) en es.json Y en.json.
- Opcional (si es barato): el título de la ventana muestra el archivo actual
  («programa.asm — EmuRV»); ayuda a ubicarse. Registrar si entra.

## Fase 3 — Docs y cierre

README/Ayuda (sección de archivo: Nuevo/Abrir/Guardar/Guardar como; nota del
cierre de ventanas); BITACORA (decisiones: guardar inteligente, confirm de
Nuevo, enfoque frontend del cierre). ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F3 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB + verificación manual: (a) desacoplar
un panel y cerrar la principal → la hija se cierra; (b) Nuevo/Guardar/Guardar
como/Abrir funcionan y «Guardar» no re-pregunta si ya hay ruta.

## Criterios de aceptación

- Cerrar la ventana principal cierra también las desacopladas. ✓ F1.
- Nuevo (con confirm si hay cambios) + Guardar como + Guardar inteligente
  (no re-pregunta con ruta conocida), TODOS dentro del menú desplegable
  «Archivo». ✓ F2.
- Menú «Archivo ⌄» desplegable, coherente con los otros `.sel` de la barra;
  cierra al click-afuera/Escape; «Exportar ELF» queda fuera del menú. ✓ F2.
- i18n es/en de todo lo nuevo (anti-drift verde). ✓ F2.
- Sin romper modo binario, ejemplos/lecciones (rutaArchivo null), `.include`
  (base_dir). ✓ F2.
