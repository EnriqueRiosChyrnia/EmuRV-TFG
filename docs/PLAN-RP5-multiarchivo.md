# PLAN-RP5 — Ensamblador multi-archivo (.include, .equ, .macro, %hi/%lo)

Punto RP-5 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

Es el punto de mayor riesgo técnico del roadmap: el `source_map` (línea↔dirección)
ancla breakpoints y resaltado de línea, y tiene que sobrevivir a includes y a la
expansión de macros. Cada fase cierra con la suite COMPLETA verde — las regresiones
en tests TEC/FB/RP1/RNF existentes son la señal de que el invariante se rompió.

## Estado actual (anclas en código)

- `assemble(source: &str) -> Result<Program, Vec<AsmError>>` (assembler.rs:470),
  dos pasadas (Pass1 layout/símbolos, Pass2 codificación).
- `SourceMap { addr_to_line: HashMap<u32,u32>, line_to_addr: BTreeMap<u32,u32> }`
  (assembler.rs:50) — las líneas son del ÚNICO fuente.
- `AsmError { line, col, msg }` (assembler.rs:22) — sin campo de archivo.
- `%hi`/`%lo` hoy se rechazan con error didáctico (assembler.rs:168) — quitarlo
  recién cuando el soporte exista.
- Directivas existentes: .text .data .globl .align .space .word .half .byte
  .ascii .asciiz .string (assembler.rs:655-760).
- El frontend NO guarda la ruta del archivo abierto (main.ts::abrirArchivo lee el
  contenido y lo descarta) — `.include` necesita esa ruta como base de resolución.
- Consumidores de `assemble`: comando Tauri (src-tauri/src/commands.rs),
  CLI (emurv-cli/src/main.rs), ~20 archivos de tests.

## Diseño acordado (no re-litigar)

### Arquitectura: preprocesador con tabla de procedencia
Capa NUEVA antes de Pass1 — Pass1/Pass2 casi no se tocan:

```
preprocess(fuente_principal, base_dir, nombre_archivo)
  -> Result<Preprocessed, Vec<AsmError>>

Preprocessed {
    lines: Vec<PpLine>,        // fuente aplanado
    files: Vec<String>,        // [0] = archivo principal
}
PpLine {
    text: String,
    file: u16,                 // índice en files
    line: u32,                 // línea en SU archivo (1-based)
    main_line: u32,            // línea del archivo PRINCIPAL a la que se
                               // atribuye para el source_map (regla abajo)
}
```

- **Regla de atribución `main_line` (el corazón del invariante):**
  - Línea del archivo principal → su propia línea.
  - Línea traída por `.include` (directo o anidado) → la línea de la directiva
    `.include` DEL ARCHIVO PRINCIPAL que la introdujo.
  - Línea expandida de macro → la línea de la INVOCACIÓN (atribuida a su vez por
    la misma regla si la invocación vino de un include).
  - Consecuencia: `SourceMap` sigue hablando SOLO de líneas del archivo principal
    — la UI actual (breakpoints, resaltado, editor de un solo buffer) funciona sin
    cambios. Varias instrucciones pueden compartir `main_line` (ya pasa hoy con
    pseudo-instrucciones de 2 words: assembler.rs:47).
- **Errores:** `AsmError` gana `file: Option<String>` (None = archivo principal;
  el formato de display antepone «nombre.s:» solo cuando es Some). Los mensajes
  dentro de includes/macros reportan archivo y línea REALES (file/line de PpLine),
  no la atribución.
- **`Program` gana procedencia completa** para el futuro (UI multi-pestaña, RP
  visual): `origins: Vec<(addr: u32, file: u16, line: u32)>` + `files: Vec<String>`.
  Nadie de la UI actual lo consume todavía — es barato y evita re-ensamblar el
  diseño después.

### Sintaxis (compatibilidad RARS/MARS — el material didáctico existente manda)
- `.include "ruta/relativa.s"` — relativa al directorio del archivo que la
  contiene. Ciclos detectados con pila de inclusión (error didáctico mostrando la
  cadena A → B → A); profundidad máxima 16; archivo inexistente → error con la
  ruta que se intentó.
- `.eqv NOMBRE valor` Y `.equ NOMBRE, valor` (aceptar ambas grafías; RARS usa
  `.eqv`, GNU `.equ`) — constante simbólica numérica, definida antes de usarse,
  redefinición = error didáctico. Usable en todo operando inmediato y como
  dirección en `la`/`lw`/`sw`.
- `.macro NOMBRE (%p1, %p2)` … `.end_macro` (sintaxis RARS; aceptar también
  `.endm` como cierre). Parámetros `%nombre` sustituidos textualmente en la
  expansión. Invocar macros dentro de macros: permitido (límite de expansión 64
  para cortar recursión con error didáctico). DEFINIR macro dentro de macro:
  error. Macros definidas en includes: visibles después del punto de inclusión.
- `%hi(simbolo_o_constante)` / `%lo(...)` — en operandos inmediatos de `lui`,
  `addi`, `ori`, loads/stores; resolución en Pass2 (referencias hacia adelante
  funcionan igual que las etiquetas). Semántica estándar RISC-V con ajuste de
  signo (%hi = (val + 0x800) >> 12, %lo = sign-extended low 12) — documentar que
  `lui+addi` con %hi/%lo da el mismo resultado que `la` (test que lo demuestre).

### Entrada de la ruta base (resolución de `.include`)
- Núcleo: `assemble_with_includes(source, base_dir: Option<&Path>, main_name: &str)`
  — nueva. `assemble(source)` queda como wrapper con `base_dir: None` (CERO
  cambios en los ~20 archivos de tests existentes). Con `base_dir: None`, un
  `.include` produce error didáctico: «.include necesita que el archivo esté
  guardado en disco».
- CLI: base_dir = directorio del archivo de entrada. Gratis.
- UI: el frontend guarda la ruta al abrir/guardar (`abrirArchivo`/`guardarArchivo`
  en main.ts) y la manda como argumento opcional nuevo del comando de ensamblar;
  buffer nunca guardado → None → error didáctico si usa `.include`. El comando
  Tauri agrega el parámetro opcional SIN romper la firma para llamadas existentes.

### Criterio de breakpoints (ajuste honesto del ROADMAP)
El ROADMAP pedía «breakpoints en el archivo incluido»; el editor es de UN buffer.
Criterio real de este plan: (a) breakpoint sobre la línea del `.include` detiene
en la primera instrucción ejecutable traída por ese include (consecuencia directa
de la regla main_line); (b) errores dentro de includes reportan archivo:línea
reales; (c) breakpoints POR archivo incluido quedan para la fase visual (editor
multi-pestaña), registrado como propuesta. El ROADMAP se actualizará al cierre
(lo hace la sesión coordinadora).

## Fase 1 — `.eqv`/`.equ` + `%hi`/`%lo` (sin tocar el flujo de archivos)

1. Constantes: tabla en Pass1, validación de redefinición y de uso-antes-de-definir,
   integración en los parsers de operandos inmediatos y de direcciones.
2. `%hi`/`%lo` en Pass2 con la semántica del diseño; quitar el rechazo didáctico
   de assembler.rs:168 (el mensaje viejo muere recién acá).
3. Tests (en el módulo de tests del assembler): constantes encadenadas, redefinición
   → error, `%hi`/`%lo` equivalente a `la` (lui+addi vs la, mismo binario),
   referencia hacia adelante, %lo con valor negativo (ajuste de signo), uso en
   `.word`.

**Compuerta F1:** tests nuevos + suite COMPLETA verde (ambos crates) + clippy.
Commit `rp5-fase-1: constantes .eqv/.equ y operadores %hi/%lo`.

## Fase 2 — Preprocesador + `.include`

1. Módulo nuevo `core/preprocess.rs` con `Preprocessed`/`PpLine` y la regla de
   atribución main_line. `assemble_with_includes` lo invoca; Pass1/Pass2 consumen
   `PpLine` (cambio mecánico: donde había número de línea, ahora hay PpLine con
   line para errores y main_line para source_map).
2. `.include`: resolución relativa al archivo contenedor, pila anti-ciclos con
   cadena en el error, límite de profundidad, error de archivo inexistente.
3. `AsmError.file` + display «archivo.s:línea» + `Program.origins/files`.
4. CLI: pasar base_dir (un cambio de una línea en emurv-cli).
5. Tests: include simple (símbolos visibles en ambas direcciones), include anidado,
   ciclo A→B→A con cadena en el mensaje, archivo faltante, include en .data,
   `assemble()` sin base_dir + .include → error didáctico, e INVARIANTE: programa
   con include, breakpoint por dirección en línea del .include detiene en la
   primera instrucción del include (test a nivel Machine, patrón machine_for).

**Compuerta F2:** todo verde (la suite TEC/FB/RP1 intacta es la prueba del
invariante) + clippy. Commit `rp5-fase-2: preprocesador e .include con procedencia`.

## Fase 3 — Macros

1. Recolección de definiciones en el preprocesador (`.macro`/`.end_macro`/`.endm`),
   sustitución de parámetros `%nombre`, expansión con atribución a la línea de
   invocación, límite de 64 expansiones, errores didácticos (definición anidada,
   macro no definida si algo parece invocación de macro conocida-casi, parámetros
   de más/de menos).
2. Tests: macro simple, macro con parámetros, macro que usa constantes .eqv,
   macro definida en un include e invocada en el principal, macro dentro de macro
   (invocación), recursión → error con límite, source_map: N invocaciones de la
   misma macro → N grupos de direcciones atribuidos a las N líneas de invocación
   (verificar line_to_addr), step a través de una expansión no rompe el resaltado.
3. Medición RNF-02 sigue cumpliendo: ensamblar 500 líneas (ahora con un include y
   una macro en el mix) < 200 ms — extender el test rnf02 existente o agregar
   variante.

**Compuerta F3:** todo verde + clippy + rnf02 extendido verde.
Commit `rp5-fase-3: macros RARS con atribución a la invocación`.

## Fase 4 — UI, ejemplo, documentación y cierre

1. Frontend: guardar ruta en abrir/guardar; comando de ensamblar con parámetro
   opcional de ruta base; tsc estricto limpio. Error didáctico visible en el panel
   de errores cuando .include sin archivo guardado.
2. Ejemplo nuevo `ejemplos/11-multiarchivo/` (carpeta: main.s + utiles.s +
   constantes.s): usa .include, .eqv, una macro y %hi/%lo, imprime algo
   verificable. Integrarlo donde corresponda (menú de ejemplos de la UI carga
   archivos de disco — si la UI solo embebe strings, el ejemplo va por CLI y se
   registra la limitación para la fase visual).
3. Test CLI: `emurv-cli run ejemplos/11-multiarchivo/main.s` → salida esperada,
   y `emurv-cli asm` → ELF válido.
4. Docs: ENSAMBLADOR.md (secciones nuevas: includes, constantes, macros, %hi/%lo,
   la regla de atribución del source_map con un diagrama de texto), ayuda integrada
   (panel: mencionar las directivas nuevas), README (tabla de directivas),
   BITACORA (decisiones + niveles de confianza).
5. `docs/ROADMAP-PRO.md`: NO tocar — lo gestiona la sesión coordinadora.

**Compuerta F4 (cierre):** `cargo test` verde en AMBOS crates + clippy ambos +
`tsc --noEmit` limpio + `scripts/ci-local.sh --skip-bundle` verde +
`cargo tauri build` exitoso + gate de bundle ≤ 30 MB.
Commit `rp5-fase-4: UI, ejemplo multiarchivo y docs`.

## Criterios de aceptación (del ROADMAP, ajustados)

- `.include`, `.eqv`/`.equ`, `.macro`/`.end_macro`, `%hi`/`%lo` funcionando con
  sintaxis RARS. ✓ F1–F3.
- Errores con archivo:línea de origen real dentro de includes/macros. ✓ F2–F3.
- source_map sobrevive: suite existente intacta + tests de invariante específicos
  (breakpoint vía .include, resaltado a línea de invocación de macro). ✓ F2–F3.
- Ejemplo multi-archivo corriendo por CLI (y por UI si el menú de ejemplos lo
  permite sin rediseño). ✓ F4.
- Two-pass con macros anidadas y .eqv encadenados testeados. ✓ F1/F3.
- Breakpoints por archivo incluido: DIFERIDO a la fase visual (editor
  multi-pestaña) — registrado como propuesta, no es deuda de este plan.
