# Ensamblador RV32IM propio — diseño y capacidades

> Documento técnico. Componente desarrollado durante la implementación de EmuRV v0.1.
> Estado: implementado y verificado (35/35 tests al cierre de v0.1, clippy 0 warnings).
> **Actualización 2026-06-11:** tras la fase framebuffer, la suite completa del proyecto cierra en **55 tests verdes + 4 marcados `#[ignore]`** (59 totales); ver nota en §5.

## 1. Origen de la decisión

El stack inicialmente previsto (ver ARQUITECTURA.md) contemplaba **Keystone** como ensamblador. Durante el spike de riesgo R1 (fase 0) se verificó, con evidencia directa de los headers de la última release oficial de Keystone (0.9.2, octubre 2020), que **Keystone no soporta la arquitectura RISC-V en absoluto**: el enum `ks_arch` de la librería contiene ARM, ARM64, MIPS, X86, PPC, SPARC, SYSTEMZ, HEXAGON y EVM, pero no `KS_ARCH_RISCV`. Un `grep -ri riscv` sobre todos los headers instalados arrojó cero resultados. Ningún binding de Rust puede exponer una arquitectura que la librería C subyacente no implementa.

Ante esto se descartaron tres caminos —depender de un fork no fusionado y sin releases, invocar un toolchain GNU externo (rompiendo la condición de aplicación autocontenida y produciendo errores en inglés), o construir un ensamblador propio— y se optó por el tercero.

## 2. Herramienta de desarrollo

El ensamblador fue **diseñado e implementado con asistencia de Claude Fable 5** (modelo de Anthropic, familia Claude 5, nivel Mythos), operando de forma autónoma sobre el contrato de trabajo definido en SPEC.md, ARQUITECTURA.md y PLAN.md. La verificación de cada codificación se realizó contra la especificación oficial RV32I calculada bit a bit en los propios tests (sin oráculo externo disponible, dado que ni Keystone ni un toolchain GNU estaban accesibles), y los resultados se documentan en la BITACORA del proyecto.

Esta declaración de autoría responde a un principio de honestidad metodológica: el componente no se tomó de una librería existente ni se copió de terceros, sino que se generó asistido por IA, y eso debe constar de forma explícita en cualquier documentación derivada.

## 3. Arquitectura del ensamblador

Ensamblador de **dos pasadas** escrito en Rust, sin dependencias externas para el núcleo de ensamblado:

- **Pasada 1**: análisis léxico y sintáctico, validación de la forma de los operandos, construcción de la tabla de símbolos (etiquetas) y cálculo de tamaños. La validación de forma en esta pasada garantiza que la segunda solo pueda fallar por símbolos no definidos o por rangos fuera de límite, lo que mantiene sincronizados los tamaños calculados con la codificación final (y por tanto con el `source_map`).
- **Pasada 2**: resolución de símbolos y emisión de la codificación binaria.

El ensamblado produce, además de los bytes, una tabla bidireccional **línea ↔ dirección** (el `source_map`): la pieza que permite anclar los breakpoints del editor al `pc` del emulador y resaltar la línea en ejecución.

## 4. Capacidades

### 4.1. Formatos de instrucción RV32IM

Codifica los seis formatos de la especificación base más la extensión M (multiplicación/división):

- **R** (registro-registro): add, sub, and, or, xor, sll, srl, sra, slt, sltu, y la extensión M (mul, mulh, div, rem, etc.).
- **I** (inmediato): addi, andi, ori, xori, slti, sltiu, slli, srli, srai, las cargas (lw, lh, lb, lhu, lbu) y jalr.
- **S** (almacenamiento): sw, sh, sb.
- **B** (salto condicional): beq, bne, blt, bge, bltu, bgeu.
- **U** (inmediato superior): lui, auipc.
- **J** (salto incondicional): jal.

### 4.2. Pseudo-instrucciones

Soporta el repertorio habitual de pseudo-instrucciones que el material didáctico en español emplea, expandiéndolas a instrucciones reales:

- **`li`** (load immediate): expande a una sola instrucción (`addi`) si el inmediato cabe en 12 bits con signo, o a dos (`lui` + `addi` con compensación de signo) en caso contrario. La decisión se toma en la pasada 1 porque `li` solo admite literales.
- **`la`** (load address): carga la dirección de una etiqueta.
- **`mv`, `j`, `call`, `tail`, `nop`, `ret`** y comparaciones/saltos derivados.
- **Cargas y almacenamientos con etiqueta** al estilo RARS (`lw t0, dato` / `sw t0, dato, t1`), que expanden a `lui` + operación. El material didáctico en español los usa, de ahí su inclusión.

Nota de diseño: `call`/`tail` se expanden a `jal`/`j` simples (alcance ±1 MB) en lugar del par `auipc`+`jalr` de GNU as. Los programas educativos no se acercan a ese límite, y el paso a paso muestra una sola instrucción por llamada, más legible para el estudiante. Si un programa excediera el alcance, el error en español lo explica.

### 4.3. Directivas

`.text`, `.data`, `.word`, `.half`, `.byte`, `.asciiz`, `.space`, con alineación automática del segmento de datos. Las etiquetas en la misma línea de un dato se definen *después* de alinear, de modo que `w: .word 7` apunte al word y no al relleno de alineación (comportamiento capturado por un test).

### 4.4. Diagnóstico de errores didáctico (RF-02)

Es la ventaja diferencial frente a un ensamblador genérico, y la razón por la que el plan B resultó ser una mejora y no solo un reemplazo:

- **Todos los errores**, no solo el primero, ordenados por línea y columna.
- **Mensajes en español** redactados para un principiante, con la posición exacta (línea, columna) para que el frontend los ancle como subrayados en el editor.
- Ejemplos reales del comportamiento verificado:
  - `línea 2, col 5: instrucción desconocida: «sumar». EmuRV soporta RV32IM …`
  - `línea 4, col 5: «addi» espera 3 operandos (registro, registro, inmediato); hay 2`

Este nivel de control sobre el mensaje de error es imposible de obtener delegando en un ensamblador externo, y constituye uno de los argumentos de la herramienta frente a alternativas.

## 5. Verificación

Las codificaciones se validaron contra la especificación RV32I calculadas a mano en los tests, dado que no había oráculo externo disponible. Ejemplos documentados en la suite:

- `addi x1, x0, 5` → `0x00500093`
- `beq x1, x2, +8` → `0x00208463`

La suite completa del proyecto (ensamblador + máquina + syscalls + export ELF + integración de los 6 ejemplos) cierra en **35/35 tests verdes**, con `cargo clippy --all-targets` sin advertencias.

> **Nota (2026-06-11):** el «35/35 tests verdes» y «los 6 ejemplos» del párrafo anterior describen el estado al cierre de v0.1 y se conservan como registro histórico. Tras la fase framebuffer, la suite completa del proyecto cierra en **55 tests verdes + 4 marcados `#[ignore]`** (benchmarks de rendimiento que se ejecutan solo en release, con `cargo test --release -- --ignored`), es decir **59 tests totales**; y los ejemplos en `ejemplos/` son **7**: el `06-tetris` ASCII fue reemplazado por `08-tetris-grafico` (FB3) y se añadió `07-franjas` (FB2).

## 6. Relación con el stack documentado

Este documento registra una **divergencia respecto de ARQUITECTURA.md original**, que debe leerse junto con la corrección correspondiente en ese archivo:

| Componente | Stack previsto | Estado real |
|---|---|---|
| Ensamblador | Keystone (GPLv2) | **Ensamblador propio RV32IM** (sin dependencia externa) |
| Desensamblador | Capstone (BSD) | **No incorporado en v0.1** — ningún RF lo requiere (RF-06 es hexdump). Queda designado en el stack para uso futuro si se necesita desensamblado. |

La sustitución de Keystone por un ensamblador propio tiene además una **implicación de licenciamiento favorable**: elimina una dependencia GPLv2 del componente de ensamblado, lo que simplifica la situación de propiedad intelectual del proyecto (punto a verificar de todos modos contra el reglamento institucional para el resto del stack: Unicorn sigue siendo GPLv2).
