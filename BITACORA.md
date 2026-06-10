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

---

## Fase 2 — Puente Tauri + editor

### 2026-06-09 — Arquitectura del puente UI↔emulación

- **La máquina Unicorn no es `Send`** (usa `Rc` internamente): se construye DENTRO del hilo de emulación, con un canal de confirmación para reportar errores de creación al comando `assemble`. Esto refuerza el diseño del riesgo R4 (emulación siempre en hilo propio). [Seguro]
- **Comandos por canal + eventos Tauri**: la sesión guarda `cmd_tx` (órdenes), `input_tx` (entrada de consola) y dos flags atómicos (`stop`, `waiting_input`). "Detener" no toca Unicorn desde otro hilo: setea el flag y, si el programa espera entrada, manda `Cancel` por el canal de entrada para despertarlo. [Seguro]
- **Ejecución larga en tramos de 100 ms** (`emu_start` con timeout): entre tramos se emite `emu://registros` (~10 Hz, RNF-03) y se atienden órdenes auxiliares (breakpoints, lectura de memoria) sin frenar la corrida. Run/Step/Reset llegados a destiempo se descartan: la UI deshabilita esos botones mientras corre. [Seguro]
- **Ensamblar de nuevo = sesión nueva**: los canales viejos se cierran y los hilos anteriores mueren solos; sin estado compartido que limpiar. [Seguro]

### 2026-06-09 — Frontend

- Editor CodeMirror 6 con `StreamLanguage` propio para RV32 (riesgo R3): mnemónicos RV32IM + pseudo, registros ABI/xN, directivas, números en todas las bases, comentarios `#`. Tema `one-dark` (legibilidad sobre fondo oscuro sin definir un tema propio). [Seguro]
- i18n: todos los textos de la UI salen de `src/i18n/es.json` vía `t(clave, params)` (RNF-05); no hay strings hardcodeados en los componentes.
- Los 5 ejemplos se importan en build con `?raw` de Vite directamente desde `ejemplos/` (una sola fuente de verdad, sin copias).
- `tsconfig` subido a ES2021 (por `String.replaceAll`).

### 2026-06-09 — Gate F2: SUPERADO (verificación automatizada del puente completo)

Método: autotest temporal solo-dev que **clickea los botones reales** («Ensamblar», luego «Ejecutar») en la ventana de `cargo tauri dev`, lee el DOM de la consola y reporta a un archivo vía un comando dev-only (`autotest_report`, protegido con `#[cfg(debug_assertions)]`). Resultado observado:

```
salida=¡Hola, mundo!
|estado=Programa terminado (código 0)
```

Es decir: ensamblado desde la GUI, ejecución sobre Unicorn, syscall 4 → evento Tauri → DOM de la consola, y syscall 10 → estado final correcto. La captura de pantalla no fue posible (macOS exige permiso de grabación de pantalla, no otorgable en sesión autónoma); la verificación DOM es más precisa de todos modos. El hook de autorun se eliminó tras el gate; el comando `autotest_report` (solo debug) queda para verificar los gates F3/F4 y se elimina en F6. [Seguro]

---

## Fase 3 — Paneles de estado

### 2026-06-09 — Decisiones de los paneles

- **Registros (RF-05)**: tabla pc + x0–x31 con nombre ABI y alias xN, hex y decimal **con signo** (los estudiantes razonan los negativos en decimal; el hex ya muestra el patrón de bits). El resaltado de modificados compara contra el snapshot anterior en el frontend; el backend no necesita rastrear cambios. Tras Ensamblar/Reiniciar el snapshot se descarta para no marcar todo como cambiado. [Seguro]
- **Memoria (RF-06)**: hexdump de 16×16 bytes con columna ASCII, entrada de dirección hex y accesos rápidos .text / .data / pila (este último muestra desde `sp−0x40` para ver el tope de la pila con contexto). Se refresca SOLO al detenerse la ejecución o al navegar (RNF-03); durante ejecución larga solo se refrescan los registros con el evento de ~10 Hz. [Seguro]
- La entrada bloqueante de consola (syscall 5) ya estaba cableada desde F2 (canal input + flag `waiting_input` para Cancel); esta fase la verificó de punta a punta. 

### 2026-06-09 — Gate F3: SUPERADO (autotest DOM + captura de pantalla)

Con los permisos de grabación de pantalla ya otorgados, este gate tiene evidencia doble (DOM + visual, `docs/capturas/f3-fibonacci-paso.png`).

**T1 — suma interactiva punta a punta** (tipeando en el input real de la consola):

```
T1 salida=Ingresá el primer número: 8
Ingresá el segundo número: 34
La suma es: 42
T1 estado=Programa terminado (código 0)
```

**T2 — fibonacci paso a paso**: traza de la celda decimal de s0 durante 14 pasos (el `*` indica presencia de la clase CSS `cambiado`, es decir, el resaltado de RF-05):

```
T2 s0=0,0,0,0,0,0,0,0,0,0,0,1*,1,1
T2 pc=0x00400038
T2 estado=Paso ejecutado. Próxima línea: 31
```

s0 cambia exactamente en el paso 12 — el `mv s0, s1` de la primera iteración del bucle, contando la expansión de pseudo-instrucciones (`la` ocupa 2 pasos) — y el resaltado aparece solo en ese paso. La captura muestra los tres paneles poblados y el estado en español. [Seguro]

---

## Fase 4 — Breakpoints y errores didácticos

### 2026-06-09 — Decisiones

- **Breakpoints como `RangeSet` de CodeMirror**: las marcas se mueven solas con las ediciones del texto (mapeo por `tr.changes`). La UI traduce línea→dirección con el `line_to_addr` recibido de `assemble`; las líneas sin código generado (comentarios, datos, blancos) simplemente no aportan dirección. Tras re-ensamblar se reenvían (las direcciones pueden haber cambiado). [Seguro]
- **Línea actual** como `Decoration.line` con efecto propio + `scrollIntoView`: se muestra en paso/breakpoint/pausa y se limpia al terminar o fallar. [Seguro]
- **Errores anclados** con `setDiagnostics` de @codemirror/lint (subrayado + tooltip con el mensaje en español). Al editar el documento se limpian los diagnósticos viejos (quedarían corridos y confunden). [Seguro]

### 2026-06-09 — Gate F4: SUPERADO — demo documentada

**Demo (automatizada sobre la GUI real; reproducible a mano con los mismos pasos):** ejemplo Contador; click en el margen de la línea 16 (`mv a0, t0`, cuerpo del bucle) → aparece el punto rojo; «Ejecutar».

```
marcador_bp=true
parada1: estado=Detenido en breakpoint (línea 16) t0=1 linea_resaltada=true
parada2: estado=Detenido en breakpoint (línea 16) t0=2      ← tras «Ejecutar» (continuar)
parada3: estado=Detenido en breakpoint (línea 16) t0=3      ← ídem
final:   estado=Programa terminado (código 0) salida=1·2·3·…·10·   ← tras quitar el bp
```

Cada parada ocurre ANTES de ejecutar la instrucción marcada (t0 ya incrementado por la iteración anterior, salida aún sin imprimir el valor), la reanudación no re-dispara el mismo breakpoint y al quitarlo el programa corre libre hasta terminar. Captura: `docs/capturas/f4-breakpoint-bucle.png` (punto rojo en el margen + línea resaltada + t0 destacado en el panel).

**RF-02 (errores anclados), verificado en la misma sesión** con un fuente con 2 errores:

```
subrayados=2    ← decoraciones cm-lintRange-error en las líneas 2 y 4
estado=El programa tiene 2 error(es). Revisá las líneas marcadas.
consola=línea 2, col 5: instrucción desconocida: «sumar». EmuRV soporta RV32IM …
        línea 4, col 5: «addi» espera 3 operandos (registro, registro, inmediato); hay 2
```

[Seguro — evidencia DOM + captura]

---

## Fase 5 — Export ELF y ayuda integrada

### 2026-06-09 — Decisiones

- **`object::write::elf::Writer`** (API de bajo nivel del crate `object` 0.39, conforme al stack cerrado de ARQUITECTURA.md): la API de alto nivel (`write::Object`) solo emite relocatables ET_REL; para un ejecutable ET_EXEC con segmentos PT_LOAD hay que usar el Writer directo. [Seguro]
- **Estructura del ELF**: ET_EXEC, EM_RISCV, little-endian, e_entry = 0x00400000. Dos PT_LOAD (uno si no hay .data): .text R+X en 0x00400000 y .data R+W en 0x10010000, offsets alineados a página (p_offset ≡ p_vaddr mod 0x1000, requisito de los cargadores). Tabla de secciones .text/.data/.shstrtab incluida para readelf/Ripes. [Seguro]
- **El export usa una copia del programa guardada en la sesión** (no toca el hilo de emulación): exportar no interfiere con una máquina en uso. [Seguro]
- Ayuda integrada (RF-07): overlay modal con ciclo de trabajo, tabla de syscalls con ejemplo de uso, mapa de memoria, directivas, ejemplos y breakpoints. Todo el contenido sale de `es.json`. [Seguro]
- El diálogo de guardado usa `tauri-plugin-dialog` (permiso `dialog:default` en capabilities). El autotest invoca el comando `export_elf` directamente con ruta fija: el diálogo nativo de macOS no es automatizable sin permisos de accesibilidad, y no es código nuestro. [Seguro]

### 2026-06-09 — Gate F5: SUPERADO

- **3 tests unitarios del header** (`elf_export.rs`): magic/clase/endianness, ET_EXEC + EM_RISCV + e_entry, segmentos PT_LOAD con direcciones del mapa, contenido de segmentos == bytes ensamblados, congruencia offset/vaddr, y caso sin .data con un solo segmento. Total de la suite: 34/34 verdes.
- **`file` sobre el ELF exportado desde la GUI** (hola mundo, vía botón→comando real):

```
/tmp/emurv_gate.elf: ELF 32-bit LSB executable, UCB RISC-V, soft-float ABI,
version 1 (SYSV), statically linked, stripped
```

- Validación con qemu-riscv32: **no aplicable en macOS** (el modo de usuario de QEMU es solo-Linux; documentado en F0). La validación equivalente queda cubierta por los tests de header + `file` + verificación de contenido de segmentos byte a byte.
- Ayuda verificada por DOM: overlay visible, 8 filas de tablas (5 syscalls + 3 zonas de memoria). Captura: `docs/capturas/f5-ayuda.png`.

---

## Fase 6 — Pulido y entrega

### 2026-06-10 — Cierre de la fase

- Eliminado todo el andamiaje de autotests (comando `autotest_report`, gancho `__emurvTest`): el binario de entrega no contiene código de prueba.
- Último string hardcodeado del frontend movido a `es.json` (`error.posicion`); barrido de i18n completo (RNF-05).
- Icono propio (chip con la "V" de RISC-V) generado programáticamente (PNG 1024² en Python puro, sin dependencias) y procesado con `cargo tauri icon` para todos los tamaños. Fuente: `docs/icono-fuente.png`. [Seguro]
- README.md de usuario en la raíz: instalación (bundle y fuente, incluida la nota de cmake+pkg-config), primer programa en 5 minutos, tabla de syscalls, estructura del repo.

### 2026-06-10 — Mediciones de RNF (hardware: Apple Silicon, build release)

Tests reproducibles en `src-tauri/tests/rnf_rendimiento.rs` (`cargo test --release --test rnf_rendimiento -- --ignored --nocapture`):

| RNF | Requisito | Medido | Margen |
|-----|-----------|--------|--------|
| RNF-02 | ensamblar 500 líneas < 200 ms | 502 líneas → **0.37 ms** | ~540× |
| RNF-03 | ≥ 10 M instr/s | **1459.9 M instr/s** (sin breakpoints) | ~146× |
| RNF-03 (adverso) | — | **108.7 M instr/s** con hook de breakpoints activo | ~11× |

La diferencia 1460→109 M instr/s con hook activo confirma la decisión de ARQUITECTURA.md (hook solo si hay breakpoints): el JIT de Unicorn pierde un orden de magnitud con hook de código, pero incluso así supera el requisito. [Seguro — números medidos, no estimados]

### 2026-06-10 — Bundle de release

- `cargo tauri build`: compilación release limpia; **EmuRV.app generado (21 MB)**.
- **Bug atrapado por la captura de verificación del bundle**: el overlay de ayuda quedaba siempre visible — el `display: flex` del CSS pisa al atributo `hidden` (la regla de autor gana a la del UA). Fix: regla `#ayuda-overlay[hidden] { display: none; }` (mayor especificidad). Rebundle posterior verificado contra el `dist/` embebido. Lección para la tesis: la verificación visual del artefacto final encuentra lo que los tests de DOM con click no ven (el autotest de F5 chequeaba "visible tras click", que era cierto — estaba visible desde antes). [Seguro]
- **DMG**: el paso de estilado del DMG de Tauri (`bundle_dmg.sh`) usa AppleScript sobre Finder y falla sin el permiso de Automation (no otorgable en sesión autónoma). Plan B aplicado: `hdiutil create -format UDZO` directo → **EmuRV_0.1.0_aarch64.dmg (7.9 MB)**, mismo contenido, sin la decoración de fondo del instalador. [Seguro]
- **RNF-01 (arranque)**: 0.34 s desde `open` hasta que el proceso aparece como aplicación con ventana (el primer sondeo ya lo encontró: la cota real es ≤ un roundtrip de osascript, ~0.3 s). Muy por debajo de los 2 s exigidos. [Seguro]

### 2026-06-10 — GATE FINAL: checklist RF-01..RF-10 con evidencia

| RF | Requisito | Evidencia |
|----|-----------|-----------|
| RF-01 | Editor con resaltado y números de línea | CodeMirror 6 + `StreamLanguage` RV32 propio (`src/editor/riscv.ts`); capturas `docs/capturas/f3-*.png`, `f4-*.png` |
| RF-02 | Errores por línea/columna en español | Tests `errores_en_espanol_con_linea_y_columna`, `reporta_multiples_errores_ordenados`; verificación DOM F4: 2 subrayados anclados + mensajes en consola |
| RF-03 | Ejecutar y Paso sobre Unicorn | Tests `paso_a_paso_actualiza_registros`, `ecall_avanza_pc_y_no_se_repite`; gates F2 (hola mundo GUI) y F3 (fibonacci paso a paso) |
| RF-04 | Breakpoints por línea + línea actual resaltada | Test `breakpoint_detiene_sin_ejecutar_y_reanuda`; demo F4 (3 paradas en bucle con t0=1,2,3, `linea_resaltada=true`, captura) |
| RF-05 | Registros ABI hex/dec con resaltado de cambios | Panel `registros.ts`; evidencia F3-T2: traza `s0=...,1*` (resaltado solo en el paso del cambio) |
| RF-06 | Memoria navegable + accesos .text/.data/sp | Panel `memoria.ts` (hexdump 16×16 + ASCII); captura F3 mostrando .data en 0x10010000 |
| RF-07 | Consola con syscalls RARS (1,4,5,10,11) | `syscalls.rs` + tests; F3-T1: suma interactiva punta a punta tipeando en la consola; reintento de entrada inválida testado; tabla en la Ayuda |
| RF-08 | Export ELF32 RISC-V estándar | 3 tests de formato en `elf_export.rs`; `file`: "ELF 32-bit LSB executable, UCB RISC-V" (F5) |
| RF-09 | ≥5 ejemplos en español en menú | `ejemplos/*.asm` (5), menú «Ejemplos» verificado en gates F3/F4/F5; RNF-06: 6 tests de integración |
| RF-10 | Reiniciar al estado post-ensamblado | Test `reset_vuelve_al_estado_post_ensamblado` (registros y .data restaurados); botón verificado en GUI (F3) |

**RNF**: RNF-01 arranque 0.34 s ✓ · RNF-02 ensamblado 500 líneas 0.37 ms ✓ · RNF-03 1460 M instr/s (109 M con bp) ✓ · RNF-04 bundle 21 MB (.app) / 7.9 MB (.dmg) < 30 MB ✓ · RNF-05 i18n `es.json` completo ✓ · RNF-06 un test de integración por ejemplo (6 en total) ✓

**Estado final**: `cargo test` 34/34 verdes (+3 benchmarks RNF ejecutables aparte en release) · `cargo clippy --all-targets` 0 warnings · `cargo tauri build` exitoso · bundle en `src-tauri/target/release/bundle/`.

**Pendiente de verificación presencial** (la sesión autónoma no puede desbloquear la pantalla): re-captura visual del bundle final con el fix del overlay — la evidencia actual es la regla CSS presente en el `dist/` embebido y la determinismo del selector. Pasos: abrir `EmuRV.app`, confirmar que la ayuda NO está abierta al inicio y que el botón «Ayuda» la abre/cierra.

---

## Post-entrega — Ejemplo 6: Tetris por turnos (pedido del autor, 2026-06-10)

**Pedido:** integrar a los ejemplos el Tetris 8086 (`tetris_3.asm`, 1528 líneas, emu8086) del Proyecto 2 de Microprocesadores del autor.

**Análisis del original:** juego en tiempo real para DOS — escribe directo en la memoria de video (B800h), lee teclado sin bloquear (INT 16h AH=01h), gravedad por timer del BIOS (INT 1Ah, ~18.2 Hz), bocina por INT 21h. Lógica: tablero 20×10 como array de bytes, 7 tetriminos × 4 rotaciones como bitmaps de 16 bits (4×4, bit 15 = celda superior izquierda), LCG de 16 bits (`semilla = semilla·25173 + 13849`, pieza = semilla mod 7), colisión sobre estado hipotético (`test_*`), eliminación de líneas por desplazamiento, puntaje 100/300/500/800.

**Decisión de port [Seguro]:** EmuRV v0.1 no tiene video direccionable, teclado no bloqueante ni timer (todos explícitamente fuera de alcance en SPEC.md), así que una traducción 1:1 es imposible. El port (`ejemplos/06-tetris.asm`, RV32IM, ~430 líneas) **conserva intacta la lógica del juego** — mismos bitmaps, mismas constantes del LCG y misma semilla, mismo algoritmo de colisión y de eliminación de líneas, misma tabla de puntos — y reemplaza la capa de E/S: juego **por turnos** en consola (tablero ASCII por syscall 11, rotación y columna por syscall 5 con validación y reintento, caída dura hasta apoyar, -1 para salir). Game over cuando una pieza queda apoyada tocando la fila 0. Las diferencias con el original están comentadas en el encabezado del archivo.

**Verificación:** test de integración `ejemplo_06_tetris_por_turnos` — la secuencia de piezas es determinista por el LCG (J, O, I…), lo que permite verificar el port del generador contra el original; se colocan J en col 0 y O en col 4 y se comprueba el estado exacto de las 3 filas inferiores del tablero (`|##..##....|` etc.), SCORE 0 y salida limpia. Suite completa: 35/35 verdes, clippy 0. Bundle regenerado (21 MB / DMG 7.9 MB).

Valor didáctico: es el ejemplo "grande" — arrays 2D en memoria, manipulación de bits, convención de llamada con marcos de pila reales y subrutinas anidadas, exactamente el puente emu8086→RISC-V que la tesis propone.

### PROPUESTAS FUERA DE ALCANCE (registradas durante el desarrollo, NO implementadas)

- Sugerencias "¿quisiste decir…?" por distancia de edición en errores de mnemónicos.
- Desensamblado de .text en la vista de memoria (capstone, ya designado en el stack).
- Soporte de `%hi`/`%lo` y `.equ` en el ensamblador.
- Resaltado del operando exacto del error (hoy se subraya desde la columna al fin de línea).
- Syscalls RARS adicionales (8 leer cadena, 17 exit2 con código).


