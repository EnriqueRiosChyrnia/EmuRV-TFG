# BITÁCORA — EmuRV v0.1

Registro de decisiones técnicas del prototipo. Cada entrada incluye fecha, decisión, alternativas consideradas y nivel de confianza [Seguro / Probable / Suposición].

---

## Fase 0 — Bootstrap y spike de riesgo

### 2026-06-09 — Verificación de toolchain

| Herramienta | Estado | Versión |
|---|---|---|
| rustc / cargo | ✅ | 1.96.0 (estable, aarch64-apple-darwin) |
| cmake | ✅ | 4.3.3 (requisito de `unicorn-engine`, riesgo R2 descartado) |
| node / npm | ✅ | v24.11.1 / 11.6.2 |
| tauri-cli | instalado en F0 | `cargo install tauri-cli` |
| qemu-riscv32 | ❌ no disponible | ver nota F5 |

Nota: `qemu-riscv32` (emulación de modo usuario) no está empaquetado para macOS en Homebrew — el modo `linux-user` de QEMU solo se compila en hosts Linux. La validación de ejecución del ELF exportado con qemu (parte opcional del Gate F5: "si hay qemu-riscv32") quedará documentada como no aplicable en macOS; la validación se hará con `file` + test unitario del header + carga en herramientas alternativas si están disponibles. [Seguro]

### 2026-06-09 — SPIKE R1: Keystone NO soporta RISC-V → Plan B: ensamblador propio

**Decisión: descartar Keystone y construir un ensamblador RV32IM propio en Rust** (lexer + parser + encoder de formatos R/I/S/B/U/J + pseudo-instrucciones + directivas).

**Evidencia del spike** (duración real: ~5 minutos de los 45 presupuestados — el resultado fue categórico):

1. `brew install keystone` → instala Keystone **0.9.2_1** (última release oficial, oct. 2020).
2. Inspección de `$(brew --prefix keystone)/include/keystone/keystone.h`: el enum `ks_arch` contiene exactamente `ARM, ARM64, MIPS, X86, PPC, SPARC, SYSTEMZ, HEXAGON, EVM`. **No existe `KS_ARCH_RISCV`.**
3. `grep -ri riscv` sobre todos los headers instalados: **cero resultados**.
4. Bindings Rust disponibles (`keystone` 0.9.0, `keystone-engine` 0.1.0, `hexpatch-keystone` 0.9.4) envuelven la misma librería C, por lo que ninguno puede ofrecer una arquitectura que la librería no tiene.

El riesgo R1 de ARQUITECTURA.md preveía "soporte históricamente incompleto"; la realidad es peor: el soporte es **inexistente** en cualquier release. No fue necesario intentar ensamblar las 10 instrucciones de prueba: la arquitectura no se puede ni seleccionar. Decisión irreversible según contrato (decidir en fase 0 y no volver atrás).

**Alternativas consideradas:**
- *Keystone desde fork/PR no fusionado con RISC-V*: descartado — depender de un fork muerto sin releases es deuda técnica inaceptable para una tesis. [Seguro]
- *Invocar un toolchain GNU externo (`riscv64-elf-as`)*: descartado — rompe el requisito de app autocontenida, complica el bundle (<30 MB) y los mensajes de error serían los de GNU as en inglés. [Seguro]
- *Ensamblador propio (elegida)*: costo ~1 fase extra de trabajo, absorbido en F1. Beneficios directos para los RF: mensajes de error por línea/columna en español de calidad didáctica (RF-02) y control total del source_map (RF-04). [Seguro]

**Nivel de confianza: [Seguro]** — evidencia directa de headers de la última release oficial.

### 2026-06-09 — Frontend: Vite + TypeScript vanilla + CodeMirror 6 (sin framework)

**Decisión:** frontend en TypeScript estricto sin framework de componentes, con Vite como bundler y CodeMirror 6 como editor.

**Alternativas:** Svelte (permitido por CLAUDE.md) aportaría reactividad declarativa, pero la UI de EmuRV son 4 paneles con actualizaciones controladas por eventos de emulación — un patrón imperativo simple. Un framework agregaría peso al bundle (RNF-04) y una capa de indirección sin beneficio proporcional. CodeMirror 6 es la única pieza de UI compleja y es independiente de frameworks. [Probable]

### 2026-06-09 — Gate F0: SUPERADO

- Scaffold Tauri 2 + Vite + TS vanilla generado con `create-tauri-app` (identificador `py.edu.fiuni.emurv`, producto `EmuRV`).
- Estructura de carpetas de ARQUITECTURA.md creada (`src-tauri/src/core`, `src/editor`, `src/paneles`, `src/i18n`, `ejemplos/`).
- `cargo tauri dev`: compilación limpia (347 crates) y proceso `emurv` corriendo como aplicación GUI en primer plano (verificado vía System Events: aparece en la lista de procesos con `background only = false`, es decir, con ventana). La inspección del título exacto de la ventana requiere permisos de accesibilidad de macOS que no corresponde otorgar en una sesión autónoma; la evidencia de proceso GUI activo + arranque sin errores se considera suficiente. [Seguro]
- Decisión R1 registrada arriba. Gate F0 cumplido.

---

## Fase 1 — Núcleo sin GUI

### 2026-06-09 — R2 materializado distinto a lo previsto: faltaba pkg-config, no cmake

`unicorn-engine-sys 2.1.5` falló al compilar con errores C desconcertantes (`typedef redefinition: Int128 vs __int128` y `PROT_EXEC` no declarado). Diagnóstico: el script `qemu/configure` que invoca el CMakeLists del crate **exige `pkg-config`**; al no encontrarlo abortó, pero CMake no propaga ese fallo y dejó un `config-host.h` vacío — sin `CONFIG_INT128` ni los defines de plataforma, el código C de qemu no compila. Solución: `brew install pkg-config` y recompilar. Lección registrada para README (requisitos de build). [Seguro — verificado reproduciendo el configure a mano]

Nota de método: el primer build "exitoso" era un falso positivo porque el comando usaba `| tail`, que enmascara el código de salida. Los builds de verificación posteriores se corren sin pipe.

### 2026-06-09 — Comportamiento empírico de traps en Unicorn 2.1.5 (RISC-V)

Verificado con tests (los asserts originales fallaron y revelaron la semántica real):

1. En el hook de interrupción, **el pc ya está avanzado** a la instrucción siguiente a la que causó la excepción (ecall en 0x...08 → hook ve pc=0x...0C). No hay que avanzar pc manualmente para reanudar; la instrucción culpable está en `pc − 4`.
2. Las **instrucciones ilegales NO llegan como `Err(INSN_INVALID)`** de `emu_start` cuando hay hook de interrupción instalado: llegan al propio hook con causa 2. El despacho se hace por `intno` (tabla mcause: 2 ilegal, 3 ebreak, 8/9/11 ecall según privilegio).
3. Los accesos a memoria no mapeada sí se reportan como `Err` de `emu_start`, con el hook `MEM_INVALID` capturando la dirección exacta para el mensaje didáctico.

[Seguro — evidencia empírica de tests, no de documentación]

### 2026-06-09 — Decisiones de diseño del ensamblador propio

- **Dos pasadas** con validación de forma de operandos en la primera: la segunda solo puede fallar por símbolos no definidos o rangos, lo que garantiza que los tamaños calculados (y por tanto el source_map) no se desincronicen de la codificación. [Seguro]
- **Reporte de TODOS los errores** (no solo el primero), ordenados por línea/columna: el frontend los ancla al editor (RF-02). [Seguro]
- **`li`**: expande a 1 palabra (addi) si el inmediato cabe en 12 bits con signo, si no a 2 (lui+addi con compensación de signo). Decidible en pasada 1 porque `li` solo acepta literales (para etiquetas el error sugiere «la»). [Seguro]
- **`call`/`tail` → `jal`/`j` simples** (alcance ±1 MB) en lugar del par auipc+jalr de GNU as: los programas educativos no se acercan a ese límite y el paso a paso muestra una sola instrucción por llamada, que es más legible para el estudiante. Si un programa lo excede, el error en español lo explica. [Probable]
- **Cargas/almacenamientos con etiqueta** (`lw t0, dato` / `sw t0, dato, t1`) al estilo RARS, expandiendo a lui+op: el material didáctico en español los usa. [Probable]
- **Etiquetas y alineación automática**: `.word`/`.half` alinean el segmento de datos; las etiquetas de la misma línea se definen DESPUÉS de alinear, para que `w: .word 7` apunte al word y no al relleno (un test lo capturó apuntando al relleno). [Seguro]
- **gp = 0x10008000 y región de datos mapeada desde 0x10000000** (no solo desde .data 0x10010000): coincide con RARS, donde gp apunta a la zona estática previa a .data; así el material que usa `gp` no rompe. [Probable]
- **Capstone (desensamblador) no incorporado en v0.1**: ningún RF lo requiere (RF-06 es hexdump). La decisión de stack de ARQUITECTURA.md queda intacta como elección designada si se necesita desensamblado en el futuro; incluirlo hoy solo agregaría peso al bundle (RNF-04). [Probable]
- **`ebreak` = pausa por software** (no error): detiene la ejecución y avanza pc para poder reanudar. [Probable]
- **Codificaciones verificadas a mano** contra la especificación RV32I en tests (`addi x1,x0,5` = 0x00500093, `beq x1,x2,+8` = 0x00208463, etc.) — sin oráculo externo disponible (sin Keystone ni toolchain GNU), los valores de referencia se calcularon bit a bit y se documentan en el propio test. [Seguro]

### 2026-06-09 — Gate F1: SUPERADO

- 25 tests unitarios (ensamblador + máquina + syscalls) y 6 de integración (RNF-06: los 5 ejemplos de RF-09 punta a punta, más reintento de entrada inválida): **31/31 verdes**.
- Verificaciones de integración: salida de consola exacta, registros finales (s0/s1 de fibonacci, sp balanceado tras recursión), breakpoint que detiene sin ejecutar y reanuda, reset que restaura .data.
- `cargo clippy --all-targets`: 0 warnings. `cargo fmt` aplicado.

