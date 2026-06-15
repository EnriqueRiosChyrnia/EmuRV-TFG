# ROADMAP-PRO — De herramienta educativa a herramienta profesional

Extensiones post-v0.1 aprobadas por el autor (2026-06-12), surgidas del análisis de huecos
sobre el grafo de conocimiento del proyecto. Objetivo: que EmuRV sirva también a
desarrolladores de ensamblador RISC-V, sin abandonar el posicionamiento educativo
(la visualización de datapath/pipeline sigue cedida a Ripes por diseño, ver Cap. I/II).

Estos puntos están FUERA del alcance del SPEC v0.1. No son deuda de la tesis: son roadmap.
La regla 2 de CLAUDE.md (no expandir alcance) aplica DENTRO de cada plan RP-N, no contra
este documento.

## Protocolo de trabajo (leer antes de tocar cualquier punto)

Flujo acordado con el autor — cada punto pasa por este ciclo:

1. **Pedido de plan.** El autor dice "creá el plan para el siguiente punto" (o nombra un
   RP-N concreto). Claude toma el primer punto en estado `PENDIENTE` (orden de la tabla),
   escribe `docs/PLAN-RP<N>-<slug>.md` — plan por fases con compuertas de verificación,
   mismo estilo que PLAN.md — y cambia el estado a `PLAN-CREADO`.
2. **Entrega de prompt.** Claude entrega al autor un prompt autocontenido listo para pegar
   en otra terminal (sesión separada de Claude Code en este repo). El prompt debe instruir:
   leer CLAUDE.md, SPEC.md, ARQUITECTURA.md y el PLAN-RP correspondiente; ejecutar el plan
   fase por fase; tests verdes obligatorios; registrar decisiones en BITACORA.md;
   commit por fase con formato `rp<N>-fase-M: descripción`.
3. **Ejecución.** El autor corre el prompt en la otra terminal. Estado: `EN-EJECUCION`
   (lo marca Claude al entregar el prompt).
4. **Cierre y documentación.** El autor vuelve y dice "terminó, actualizá". Claude
   verifica evidencia real (git log, `cargo test`, archivos nuevos), NO confía en la
   palabra de la otra sesión. Si la evidencia cierra: actualiza estado a `DOCUMENTADO`
   y sincroniza docs afectadas (SPEC/ARQUITECTURA/README y Capítulo VI de la memoria
   si aplica). Si la evidencia NO cierra: estado vuelve a `EN-EJECUCION` y se reporta
   qué falta.
   **Grafo — semántica por lotes:** el hook post-commit ya cubre el código (AST,
   gratis) en cada commit. La re-extracción semántica de docs (`/graphify --update`,
   cuesta LLM) NO se corre por punto: se corre en lote — al completar cada 3-4 puntos,
   al cerrar el roadmap entero, o antes si el autor va a consultar el grafo sobre los
   docs nuevos. Los docs pendientes de re-extracción se acumulan solos: el próximo
   `--update` los detecta por manifest.
5. **Notificación final.** Cuando TODOS los puntos estén `DOCUMENTADO`, Claude debe
   notificarlo al autor de forma explícita y destacada ("ROADMAP-PRO completo") y
   ofrecer las dos rutas acordadas: (a) explorar más huecos, o (b) pasar a la parte
   visual de la aplicación.

Estados válidos: `PENDIENTE` → `PLAN-CREADO` → `EN-EJECUCION` → `DOCUMENTADO`.
Un solo punto en vuelo a la vez.

**Conflictos con requisitos del SPEC (RNF-04 u otros) — regla fijada por el autor
(2026-06-12):** ninguna sesión (ejecutora ni coordinadora) afloja un requisito por
cuenta propia "para poder avanzar". Orden obligatorio: (1) buscar arquitectura que
satisfaga requisito Y funcionalidad (precedente: RP-3, crate separado); (2) si no
existe, FRENAR y presentar al autor el conflicto con evidencia y alternativas
probadas; (3) SOLO el autor enmienda el SPEC — y la enmienda se registra en
BITACORA con fecha y porqué, actualizando el gate correspondiente. Requisito
cambiado con registro = ingeniería; requisito violado en silencio = deuda.

**Fidelidad al diseño (autor, 2026-06-13):** el diseño visual aprobado vive en
`docs/diseno-ui/propuestas-ui.html` (export de claude.ai/design). Para los puntos
visuales (RP-18..RP-20) la prioridad es REPLICARLO CON MÁXIMA FIDELIDAD —
espaciados, colores, tipografía, estructura, microinteracciones tal como están en
ese archivo, no una aproximación. Únicas desviaciones permitidas (no son
«ignorar el diseño», lo hacen funcional y se registran en BITACORA): (1) ajustar
un color de tema un escalón si no cumple contraste WCAG AA (ya pasó en RP-17B);
(2) no renombrar los `id` del DOM que el JS consume; (3) no reordenar los
registros (orden x0..x31, convención RISC-V). Todo lo demás: fiel al mockup.

**Orden acordado (autor, 2026-06-12):** cerrar el roadmap original hasta RP-9
inclusive (RP-7 → RP-8 → RP-9), luego ENTREGA A TESTER EXTERNO (primo del autor,
bundle .dmg — ojo: solo macOS Apple Silicon mientras no exista el port), y recién
después continuar con RP-10..RP-16. Modo docente: decisión pospuesta.

## Tabla de estado

| ID | Punto | Prioridad | Estado |
|----|-------|-----------|--------|
| RP-1 | Corregir bug latente de `ebreak` | Alta (quick win) | DOCUMENTADO |
| RP-2 | Sincronizar Capítulo VI con el código real | Alta (quick win) | DOCUMENTADO |
| RP-3 | Modo CLI / headless | Alta | DOCUMENTADO |
| RP-4 | CI con GitHub Actions | Alta | DOCUMENTADO |
| RP-5 | Ensamblador multi-archivo (`.include`, `.equ`, `.macro`, `%hi`/`%lo`) | Media | DOCUMENTADO |
| RP-6 | Stub GDB remoto (protocolo gdbserver) | Media | DOCUMENTADO |
| RP-7 | Watchpoints, breakpoints condicionales y edición en vivo | Media | DOCUMENTADO |
| RP-8 | CSRs (Zicsr) y manejo mínimo de traps | Baja | DOCUMENTADO |
| RP-9 | Currículo educativo progresivo (lecciones) | Media-Alta | DOCUMENTADO |
| RP-10 | Syscalls 8/9/93 y códigos de salida reales (17 descartado: no existe en RARS) | Alta | DOCUMENTADO |
| RP-11 | Carga de ELF externos + desensamblador (Capstone arch_riscv, +100 KB) | Media | DOCUMENTADO |
| RP-12 | Extensiones F/D (coma flotante) | Media | DOCUMENTADO |
| RP-13 | Extensiones A y C — RV32IMAFDC (RV64 diferido) | Baja | DOCUMENTADO |
| RP-14 | Editor profesional (autocompletar, hover, ir-a-etiqueta) | Media | DOCUMENTADO |
| RP-15 | UI de lecciones (navegador del currículo RP-9) | Media-Alta | DOCUMENTADO |
| RP-16 | Internacionalización: inglés (UI v1; diagnósticos/lecciones futuro) | Baja | DOCUMENTADO |
| RP-17 | Sistema de temas / skins (Parte A + B: 5 temas + syntax theme) | Media | DOCUMENTADO |
| RP-18 | Ajustes de layout y UX (bug toolbar, densidad de paneles, registros) | Alta | DOCUMENTADO |
| RP-19 | Estética «Cálido» (única) + microinteracciones + wordmark + grupo-ejec | Media | DOCUMENTADO |
| RP-20 | Rediseño de modales + Ayuda 2 columnas + Acerca de | Media | DOCUMENTADO |
| RP-21 | Recuperar vista de objetivos en modal de Lecciones (fix regresión RP-20) | Alta | DOCUMENTADO |
| RP-22 | Desacoplar paneles Registros/Memoria/Depuración a ventanas (modo mover) | Media | DOCUMENTADO |
| RP-23 | Gestión de archivos (Nuevo, Guardar como, guardar inteligente) + cerrar ventanas hijas al salir | Media | DOCUMENTADO |
| RP-24 | Find/Replace en el editor (Cmd+F) | Baja | DOCUMENTADO |
| RP-25 | Indicador de cambios sin guardar + aviso al cerrar | Baja | PENDIENTE |
| RP-26 | Búsqueda en el panel de Memoria | Baja | DOCUMENTADO |
| RP-27 | Persistir workspace/sesión (último archivo, layout, ventanas) | Media | PENDIENTE |
| RP-28 | Tema de usuario (crear/editar/cargar) | Media | DOCUMENTADO |
| RP-29 | ELF reubicable (auipc, independiente de dirección de carga) | Media | DOCUMENTADO |
| RP-30 | Pestañas + archivos recientes | Media-Alta | PENDIENTE |
| RP-31 | Sprites + sonido (sobre el framebuffer) | Media-Alta | PENDIENTE |
| RP-32 | RV64 — soporte 64 bits, conmutable RV32/RV64 | Alta (esfuerzo) | DOCUMENTADO |

## Detalle de los puntos

### RP-1 — Corregir bug latente de `ebreak`
- **Qué:** al reanudar tras un `ebreak`, el pc entregado al hook ya viene avanzado y el
  flujo post-hook avanza de nuevo → "instrucción inválida". Reproducido con test sonda;
  nunca se manifestó porque ningún ejemplo lo usa.
- **Referencia:** BITACORA.md línea ~282 (sesión 2026-06-11, marcado [Seguro]).
- **Criterios de aceptación:** test que ensambla `ebreak`, reanuda y sigue ejecutando
  correctamente; documentar `ebreak` como soportado en SPEC/ayuda solo después del fix.

### RP-2 — Sincronizar Capítulo VI con el código real
- **Qué:** el borrador del Cap. VI (en `../partes/`) afirma cosas que `tec-6` ya resolvió:
  §6.2.4 dice "entrada por turnos" (falso: teclado MMIO tiempo real implementado, tests
  tec1–tec5) y §6.2.6 lista "Detener no corta bucle apretado" como pendiente (resuelto
  con slicing por count, ver tec1_detener.rs). El bug de `ebreak` sí sigue vigente
  (hasta que RP-1 cierre).
- **Tipo:** solo documentación (memoria de tesis), no toca código.
- **Criterios de aceptación:** Cap. VI consistente con BITACORA y tests; ninguna
  afirmación de "pendiente" sobre algo ya implementado.

### RP-3 — Modo CLI / headless
- **Qué:** binario `emurv-cli` (o subcomando) que ensambla y ejecuta sin UI:
  `emurv asm prog.s -o prog.elf` y `emurv run prog.s --max-instr N`, con salida del
  programa por stdout, errores de ensamblado por stderr (mismo formato didáctico) y
  código de salida del proceso. Habilita corrección automática de tareas, scripting y CI.
- **Base:** el núcleo Rust (`core/`) ya está desacoplado de Tauri — es exponer un binario,
  no rediseñar.
- **Criterios de aceptación:** correr los 10 ejemplos del repo desde terminal con salidas
  correctas; entrada estándar conectada a syscall 5; exit code distinto de 0 ante error
  de ensamblado; documentado en README.

### RP-4 — CI con GitHub Actions
- **Qué:** workflow `.github/workflows/ci.yml`: `cargo test` + `cargo clippy` + build
  frontend en cada push/PR. Cubrir AMBOS crates: `src-tauri/` y `emurv-cli/` (crate
  separado desde RP-3; sus tests de integración corren los ejemplos como procesos).
  Matriz: macOS (obligatorio verde), Windows y Linux
  (build-only, experimental — la matriz revela hoy qué rompe del port, ver Cap. VI §6.2.1,
  en particular la compilación de Unicorn/CMake fuera de macOS ARM).
- **Restricción (decisión del autor 2026-06-12):** el remote de GitHub se crea recién
  al final del proyecto. Este punto cubre autoría + validación estática (actionlint) +
  script espejo local verde; el primer run real, el badge y el reporte empírico de
  Windows/Linux quedan DIFERIDOS a la creación del remote (checklist de activación
  registrada en el propio ci.yml y en BITACORA).
- **Criterios de aceptación (ajustados):** ci.yml validado con actionlint; cada comando
  del job obligatorio demostrado verde en local vía `scripts/ci-local.sh`, incluido un
  gate automatizado de peso de bundle ≤ 30 MB (RNF-04); deps de build por plataforma
  documentadas; checklist de activación lista.

### RP-5 — Ensamblador multi-archivo
- **Qué:** soporte de `.include "archivo.s"`, constantes `.equ`, macros `.macro`/`.endm`
  y operadores `%hi()`/`%lo()` (hoy rechazados con error didáctico, assembler.rs:168).
  Es el techo más duro del ensamblador actual: sin esto el máximo viable es un archivo.
- **Cuidado:** el `source_map` (línea→dirección) debe seguir funcionando a través de
  includes y expansión de macros — el resaltado de línea en ejecución y los breakpoints
  dependen de él.
- **Criterios de aceptación:** ejemplo nuevo multi-archivo funcionando con breakpoints
  en el archivo incluido; errores reportados con archivo+línea de origen; tests de
  two-pass con macros anidadas y `.equ` encadenados.

### RP-6 — Stub GDB remoto
- **Qué:** servidor protocolo gdbserver sobre el emulador para que gdb/LLDB/VS Code
  se conecten como target remoto (`target remote :3333`). Existe `udbserver` (crate
  de stub GDB para Unicorn Engine, mismo motor del proyecto) como base.
- **Por qué:** es la feature que un desarrollador senior espera — breakpoints
  condicionales, watchpoints y scripting de gdb gratis, sin reimplementar UI.
- **Criterios de aceptación:** sesión gdb real contra un ejemplo del repo: breakpoint,
  step, lectura de registros y memoria; documentar el flujo en docs/.

### RP-7 — Watchpoints, breakpoints condicionales y edición en vivo
- **Qué:** (a) watchpoints: detener cuando una dirección/rango de memoria se escribe o
  lee (hook MEM de Unicorn, mismo mecanismo que el teclado MMIO); (b) breakpoints con
  condición simple (`x5 == 0`); (c) edición de registros y memoria desde la UI con la
  máquina pausada, sin re-ensamblar.
- **Criterios de aceptación:** tests núcleo para watchpoint de escritura y lectura;
  UI: panel de breakpoints con condición editable; edición de registro refleja en la
  siguiente instrucción ejecutada.

### RP-8 — CSRs (Zicsr) y traps mínimos
- **Qué:** instrucciones `csrr`/`csrw`/`csrrs`/`csrrc` en el ensamblador, CSRs básicos
  (mstatus, mtvec, mepc, mcause), `mret`, y un manejo mínimo de excepciones que permita
  enseñar/desarrollar temas de nivel sistema operativo. Unicorn ya lo soporta; el costo
  está en el ensamblador propio y la UI.
- **Nota:** propuesto también en Cap. VI §6.2.2 como "antesala de temas de SO".
- **Criterios de aceptación:** ejemplo didáctico de trap handler funcionando (ecall
  inválido → handler → mret); panel de registros muestra CSRs relevantes en modo
  avanzado.

### RP-9 — Currículo educativo progresivo (lecciones)
- **Qué:** reorganizar/ampliar los programas del repo en una sección educativa
  secuencial (`lecciones/` junto a `ejemplos/`): el estudiante avanza lección a
  lección ejecutando programas y APRENDE EN EL CAMINO. Secuencia tentativa de
  módulos (el plan la cerrará):
  1. Registros y movimiento de datos (li, mv, la; panel de registros como
     herramienta de observación).
  2. Aritmética y lógica (add/sub/mul/div, and/or/xor, shifts; overflow y
     representación en complemento a 2).
  3. Memoria (lw/sw/lb/sb, .data, endianness, el panel de memoria; punteros).
  4. Condicionales y saltos (beq/bne/blt..., construcción de if/else).
  5. Bucles (for/while con saltos; el ejemplo contador como punto de partida).
  6. Funciones y pila (jal/jalr/ret, convención de llamada, sp, prólogo/epílogo;
     el ejemplo 05-pila-funciones como base).
  7. Syscalls y E/S (print/read, convención RARS; consola interactiva).
  8. Constantes, includes y macros (el repertorio RP-5; ejemplo 11 como base).
  9. E/S proyectada en memoria y tiempo real (teclado MMIO, framebuffer; los
     ejemplos 07-10 como material avanzado).
  10. (Condicionado a RP-8) Traps e interrupciones — la antesala de SO.
- **Formato por lección (el plan lo fija):** programa principal ampliamente
  comentado en español + objetivos («qué vas a aprender») en cabecera +
  ejercicios al final (variantes TODO para que el estudiante modifique) +
  verificación: cada lección corre por CLI con salida esperada (testeable en CI,
  mismo patrón que los ejemplos actuales).
- **Honestidad de alcance:** «varios hilos / paralelismo» REAL no existe en
  RV32IM mono-hart sin SO — lo más cercano didácticamente: concurrencia aparente
  con syscalls de tiempo (lección 9, patrón del Tetris en tiempo real) y, si
  RP-8 se completa, interrupciones (lección 10). El plan NO promete threads.
- **Relación con la tesis:** este punto ES el corazón educativo del proyecto
  (Cap. I/II); el material puede citarse en la memoria como contribución
  didáctica. La integración visual (menú de lecciones en la UI, navegación,
  progreso del estudiante) pertenece a la fase visual — este punto produce el
  CONTENIDO y su estructura, consumible por CLI y por la UI actual (abrir
  archivo).
- **Criterios de aceptación:** estructura `lecciones/NN-tema/` con ≥ 8 módulos;
  cada lección con objetivos, programa comentado, ejercicios y salida esperada;
  todas corren por CLI con tests; índice `lecciones/README.md` con el mapa de
  aprendizaje; ejemplos actuales intactos (los avanzados se referencian, no se
  duplican).

### RP-10 — Syscalls 8/9/17/93 y códigos de salida reales
- **Qué:** completar el repertorio RARS: llamada 8 (leer cadena), 9 (sbrk/reserva
  dinámica), 93 (exit con código, convención RARS) y 17 (convención MARS heredada
  — [VERIFICAR contra la tabla oficial de RARS antes de implementar: el número
  podría corresponder a otra llamada; el Cap. VI §6.2.3 registra esta duda]).
- **Por qué primero:** hoy el código de salida del invitado es SIEMPRE 0 — la
  corrección automática vía emurv-cli (RP-3) no puede distinguir éxito/fallo del
  programa del estudiante. Este punto multiplica el valor del CLI y es la base
  del futuro modo docente.
- **Además — limpiar la consola (pedido del autor, 2026-06-12):**
  (a) botón «Limpiar» en el panel Consola de la UI — `Consola.clear()` ya existe
  (lo usa Reiniciar); es exponerlo con botón + i18n; no borra el estado de la
  máquina, solo el texto acumulado;
  (b) syscall de limpieza para el programa invitado (servicio propio de EmuRV,
  número fuera de la tabla RARS — p. ej. 60 — documentado como extensión; en el
  CLI emite secuencia ANSI clear o nada, decisión registrada): los programas
  tipo juego por turnos pueden redibujar pantalla limpia. La lección/ejemplos
  que lo aprovechen se actualizan después (no es deuda de este punto).
- **Criterios:** syscall 93 → exit code real propagado por emurv-cli al proceso;
  8 con límite de buffer y comportamiento RARS documentado; 9 con heap simple
  (puntero creciente, sin free) y región validada; botón Limpiar visible y
  funcional sin sesión y con sesión corriendo; syscall de limpieza con test;
  tests núcleo + proceso; Ayuda y docs actualizadas.

### RP-11 — Carga de ELF externos + desensamblador (Capstone)
- **Qué:** la operación inversa del export: abrir un ELF32 RISC-V producido por
  la toolchain GNU (gcc/as/ld) y ejecutarlo/depurarlo en EmuRV. Más desensamblado
  en el panel de memoria (instrucción decodificada junto al volcado hex) vía
  Capstone — ya previsto en el diseño original (ARQUITECTURA.md la menciona).
- **Cuidados:** sin fuente no hay source_map — el modo «binario cargado» debe
  degradar honestamente (sin resaltado de línea; breakpoints por dirección;
  el desensamblador es la vista principal). Validar machine/class/segmentos
  contra el mapa de memoria RARS con errores didácticos.
- **Criterios:** cargar y correr un ELF compilado con gcc RISC-V real (test con
  binario fixture); desensamblado correcto verificado contra objdump en un
  programa conocido; depuración por dirección + gdb stub funcionando sobre el
  binario cargado.

### RP-12 — Extensiones F/D (coma flotante)
- **Qué:** soporte RV32F/RV32D: registros f0–f31 en el ensamblador propio
  (flw/fsw, fadd.s, fcvt…, nombres ABI fa0…), panel de registros flotantes en
  la UI, syscalls RARS de float (2, 3, 6, 7…), tests contra valores IEEE 754
  conocidos. Unicorn ya ejecuta F/D — el costo es ensamblador + UI.
- **Por qué:** cursos de representación numérica; «herramienta RISC-V
  profesional» sin floats es un subconjunto. Registrado en Cap. VI §6.2.2.

### RP-13 — Extensiones A, C y RV64
- **Qué:** atómicas (lr/sc, amo*), comprimidas (C: decodificación y ensamblado
  de 16 bits — OJO: rompe el supuesto «toda instrucción ocupa 4 bytes» del
  source_map y los breakpoints; auditar primero) y RV64 (registros de 64 bits —
  toca paneles, syscalls y mapa de memoria; evaluar como modo aparte).
- **Prioridad baja:** valor didáctico menor en FIUNI hoy; A sin multi-hart es
  semánticamente trivial (documentarlo). Hacer al final o descartar con registro.

### RP-14 — Editor profesional
- **Qué:** sobre CodeMirror 6 ya presente: autocompletado de mnemónicos y
  registros (la lista ya existe en editor/riscv.ts), hover con ayuda por
  instrucción (sintaxis + semántica en español — reusar/expandir el material de
  RP-9), ir-a-definición de etiquetas (el ensamblador ya conoce los símbolos),
  resaltado de usos de una etiqueta. Sugerencias «¿quiso decir…?» del Cap. VI
  §6.2.5 entran acá si no salieron antes.
- **Nota:** candidato natural a ejecutarse DENTRO de la fase visual — decidirlo
  al llegar.

### RP-15 — UI de lecciones (navegador del currículo)
- **Qué:** la cara visual de RP-9: panel/menú de lecciones con el mapa de
  aprendizaje, abrir lección con objetivos visibles, marcar completada (progreso
  persistido localmente), navegación siguiente/anterior. Sin gamificación en v1
  (puntos/insignias = propuesta aparte si el autor quiere).
- **Dependencia:** RP-9 (el contenido). Pertenece a la fase visual — registrado
  acá para que no se pierda.

### RP-16 — Internacionalización: inglés
- **Qué:** traducción completa de la UI (es.json → en.json), selector de idioma,
  y decidir el alcance para mensajes del ensamblador/runtime (los errores
  didácticos son MUCHOS — inventariar y decidir si v1 traduce solo UI o también
  diagnósticos). README bilingüe.
- **Restricción del autor (2026-06-12):** este punto va AL FINAL de todo — el
  español es el diferencial de la tesis; el inglés es alcance posterior.
- **Base:** i18n ya externalizado (CLAUDE.md regla 6) — es trabajo de traducción
  y revisión, no de ingeniería.

### RP-17 — Sistema de temas / skins
- **Origen:** idea del autor (2026-06-13) tras consultar opciones de estética.
  Va DESPUÉS de RP-16 (UI ya estable: editor RP-14 e UI de lecciones RP-15
  terminados antes de tocar la capa visual a fondo).
- **Restricción tecnológica (heredada de CLAUDE.md):** sin frameworks pesados
  (nada de React/Vue/MUI/shadcn/Bootstrap). CSS puro, cero runtime. Identidad
  propia, no plantilla genérica — el diferenciador del dominio es retro-CRT
  (paleta tipo CGA/VGA modo 13h, fósforo/ámbar, scanlines sutiles), que nace del
  framebuffer y el Tetris ya presentes.
- **Arquitectura por capas (la decisión que lo habilita todo):**
  1. **Refactor a design tokens:** toda la estética sale de variables CSS
     (`--fondo`, `--acento`, `--fuente-mono`, espaciado, sombra, borde, foco…).
     Hoy hay ~530 líneas de CSS con algunas variables propias; este paso las
     completa hasta que NINGÚN color/medida esté hardcodeado. Sin esto, los
     temas son forks de CSS y el tema-de-usuario es imposible.
  2. **Selector + temas built-in con persistencia local:** mínimo «Educativo»
     (DEFAULT — claridad máxima, alto contraste; el público es principiante, lo
     wow es opt-in), «Retro-CRT» (identidad del dominio) y uno experimental.
     Un tema = un conjunto de valores de los tokens, no CSS nuevo.
  3. **Tema de usuario (futuro dentro del punto o sub-punto):** editor de tokens
     en la UI o carga de un JSON/CSS propio. Gratis una vez que (1) existe.
- **Regla dura de legibilidad (innegociable):** los temas experimentales NUNCA
  rompen la lectura del código en el editor, registros, memoria ni consola —
  contraste mínimo garantizado por checklist (y, si es barato, test de ratio de
  contraste sobre los pares texto/fondo críticos). La creatividad va en croma,
  fondo, sombras, bordes, acentos — no en sacrificar el monoespaciado legible.
- **Mejoras de base que conviene absorber acá** (de la consulta del autor):
  fuente monoespaciada self-hosted de calidad (JetBrains Mono / Commit Mono,
  ~100 KB, sin riesgo de bundle) e iconos SVG inline en la toolbar (Lucide/
  Phosphor, cero dependencia) — ambos suben la estética base sobre la que los
  temas operan.
- **Proceso de diseño (pedido del autor):** al llegar a este punto, la sesión
  coordinadora produce un PROMPT para Claude Desktop / Claude Design que analice
  el proyecto (capturas, framebuffer, dominio) y proponga temas concretos, con
  imágenes de inspiración que aportará el autor. Ese prompt NO se arma ahora —
  se arma cuando RP-16 cierre y empiece RP-17.
- **Valor de tesis:** una estética propia y justificada (más un sistema de
  temas) suma en la defensa más que «usé una librería de moda»; y el tema
  «Educativo» por defecto es coherente con el posicionamiento didáctico.
- **Criterios de aceptación (a precisar en el plan):** estética 100% por tokens;
  ≥2 temas built-in con default Educativo; persistencia del tema elegido;
  legibilidad verificada por checklist en todos los temas; sin frameworks; bundle
  ≤ 30 MB. El tema-de-usuario puede quedar como sub-fase o propuesta según
  alcance al armar el plan.

### RP-24..RP-32 — Pulido «IDE profesional» (post-rediseño, 2026-06-14)
Mejoras pedidas por el autor tras evaluar el producto como herramienta de uso
profesional. Detalle de cada una en su `PLAN-RP<N>-*.md` cuando se cree.
- **RP-32 (RV64) — decisión del autor:** soporte de 64 bits para uso profesional
  real. Diseño: CONMUTABLE RV32/RV64, no reemplazo — un switch (o elegir el ancho
  al crear un programa nuevo). Es el de mayor esfuerzo (ensamblador, Unicorn modo
  RISCV64, panel de registros 64-bit, mapa de memoria, source_map). Va al final;
  worktree propio (toca el core entero, no se paraleliza con otros).

**Paralelización con git worktrees (autor quiere trabajar en simultáneo):** los
puntos que tocan ÁREAS DISJUNTAS se ejecutan en paralelo, cada uno en su worktree
(rama propia), y la coordinadora mergea + verifica integración. Mapa de áreas:
- RP-24 → `src/editor/` · RP-26 → `src/paneles/memoria.ts` · RP-28 → `src/temas/`
  · RP-29 → `src-tauri/.../elf_export.rs` (Rust) = **DISJUNTOS, paralelizables**.
- RP-25, RP-27, RP-30 → todos `main.ts` pesado = **SECUENCIALES entre sí**.
- RP-31 (Rust+frontend audio), RP-32 (core entero) = worktree propio cada uno.
Roce menor entre paralelos: claves i18n (es/en) y BITACORA — conflicto de merge
trivial; los prompts agregan claves al final y secciones propias en BITACORA.

## Fuera de este roadmap (decisión explícita)

- Datapath/pipeline: cedido a Ripes por posicionamiento de tesis. No se reabre.
- Madurez de distribución (firma/notarización, auto-updates, releases): decisión
  del autor 2026-06-12 — se verá a futuro, no entra al roadmap.
- Port Windows/Linux: diferido junto con la creación del remote (las piernas
  experimentales de la CI de RP-4 son la sonda); requiere además hardware/runner
  de esas plataformas para verificar de verdad.
- Sprites/sonido (Cap. VI §6.2.4) y gamificación de lecciones: propuestas
  registradas, sin compromiso.
- Modo docente (definición de tareas + corrección automática en lote): en
  evaluación por el autor — ver referencias en la conversación con la
  coordinadora; si se aprueba, entra como RP propio sobre la base de RP-3+RP-10.
