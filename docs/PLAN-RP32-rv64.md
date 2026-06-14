# PLAN-RP32 — RV64 (soporte 64 bits, conmutable RV32/RV64)

Punto RP-32 de `docs/ROADMAP-PRO.md`. EL MÁS GRANDE del roadmap. Plan por fases
con compuertas: PROHIBIDO avanzar con la compuerta en rojo. Worktree propio
(`rp32/rv64`) — toca el core entero, no se paraleliza con otros.

**Decisión del autor:** soporte RV64 para uso profesional. CONMUTABLE RV32/RV64
(no reemplazo): el usuario elige el ancho (un selector / al crear un programa
nuevo). RV32 sigue siendo el default y debe quedar 100% intacto.

## Contexto (anclas verificadas)

- `src-tauri/src/core/machine.rs`: `Unicorn::new_with_data(Arch::RISCV,
  Mode::RISCV32, …)` (227) FIJO. `StopReason` usa `u32` (addr/value/pc); `xreg`,
  registros, todo u32.
- `src-tauri/src/core/mod.rs`: mapa de memoria en `u32` (TEXT_BASE 0x00400000,
  SP_INIT 0x7FFF_FFF0) — cabe en 64 bits.
- `src-tauri/src/core/assembler.rs`: «Ensamblador propio RV32IM», sin concepto de
  xlen. Codifica RV32.
- `src/paneles/registros.ts`: enteros a 32 bits; los F ya manejan 64 (NaN-boxing).
- Unicorn soporta `Mode::RISCV64` (verificar en spike).
- Oráculo GNU: `riscv64-elf-as -march=rv64im` para verificar encodings.

## Alcance

- SÍ: ejecutar y ensamblar RV64 además de RV32, conmutable; instrucciones
  RV64 (ld/sd, addiw/subw/sllw/srlw/sraw, addiw, slliw…); registros de 64 bits
  en la UI cuando el modo es RV64; selector de arquitectura persistido; RV32
  intacto.
- NO: RV128; extensiones nuevas más allá de las ya soportadas en 64 (IMAFDC en
  64); cambiar el mapa de memoria a direcciones altas (>32 bits) — el mapa
  educativo se mantiene bajo (cabe en 64). Modo docente/CLI dual si resulta
  caro: el CLI puede ganar un flag --rv64 (registrar).

## Diseño acordado (la arquitectura la FIJA el spike F1; el QUÉ es esto)

- **xlen como parámetro:** el ancho (32/64) viaja con el Program y la Machine.
  El núcleo opera internamente en `u64`; en RV32 los valores se mantienen en el
  rango de 32 bits (Unicorn en Mode::RISCV32 ya trunca/sign-extiende). Las
  DIRECCIONES del mapa siguen cabiendo en 32 bits (no cambia el mapa); lo que se
  ensancha a 64 son los VALORES de registro y las operaciones.
- **Machine:** `Mode::RISCV32` o `RISCV64` según xlen en `new`. `StopReason` y
  `registers()` pasan a `u64` (en RV32, valores que caben en 32). Regresión: con
  xlen=32 (default) TODO el comportamiento actual idéntico — los ~219 tests de
  src-tauri deben seguir verdes sin tocarlos (o con el cambio mecánico u32→u64
  que no altere semántica).
- **Ensamblador:** parametrizado por xlen. Acepta el repertorio RV64 (sufijos W,
  ld/sd) SOLO en modo 64; rechaza con error didáctico una instrucción RV64 en
  modo RV32 y viceversa lo que no aplique. Verificación contra GNU rv64im.
- **UI:** selector «Arquitectura RV32/RV64» (en la toolbar o al crear nuevo
  programa), persistido (localStorage). El panel de registros muestra 16 dígitos
  hex en RV64, 8 en RV32. El ancho elegido se pasa al ensamblar/crear máquina.
- **Frontend↔backend:** el comando assemble/crear-máquina recibe el xlen.

## Fase 1 — Spike (arquitectura del cambio)

`tests/spike_rv64.rs`: (1) `Unicorn::new(Arch::RISCV, Mode::RISCV64)` ejecuta una
instrucción RV64 (p. ej. `addiw`/`ld`); (2) reg_read/write de 64 bits; (3)
secuencia que usa los 64 bits (valor > 2^32); (4) confirmar que Mode::RISCV32
sigue idéntico. DECIDIR en BITACORA [Seguro]: u64-interno+xlen vs alternativa;
alcance del cambio u32→u64; cómo conmuta Machine. Si el costo resultara
desproporcionado o algo no cierra, FRENAR y reportar (regla de conflictos).
**Compuerta F1:** spike verde + arquitectura decidida y fundada en BITACORA;
suite actual intacta. Commit `rp32-fase-1: spike RV64 (Unicorn + arquitectura xlen)`.

## Fase 2 — Núcleo conmutable (Machine + valores u64), RV32 intacto

Machine acepta xlen; Mode según xlen; StopReason/registers a u64; mapa de memoria
revisado. REGRESIÓN: todos los tests de src-tauri verdes con xlen=32 (default).
**Compuerta F2:** suite completa verde (RV32 intacto) + tests nuevos de máquina
RV64 (valor de 64 bits round-trip) + clippy. Commit `rp32-fase-2: núcleo conmutable RV32/RV64`.

## Fase 3 — Ensamblador RV64 verificado contra GNU

Repertorio RV64 (W-instructions, ld/sd) parametrizado por xlen; errores
didácticos por arquitectura; encodings == `riscv64-elf-as -march=rv64im` palabra
a palabra. RV32 sigue ensamblando idéntico.
**Compuerta F3:** encodings RV64 == GNU + RV32 intacto + suite + clippy.
Commit `rp32-fase-3: ensamblador RV64 verificado contra GNU`.

## Fase 4 — UI: selector de arquitectura + registros 64-bit

Selector RV32/RV64 (persistido), pasaje del xlen al ensamblar/crear máquina,
panel de registros 64-bit en modo RV64. Ejemplo `ejemplos/15-rv64.asm` (usa
64 bits). i18n es/en. Tests UI de lo testeable.
**Compuerta F4:** conmutar arquitectura funciona end-to-end (ensamblar+ejecutar
un programa RV64, ver registros de 64 bits); RV32 intacto; tsc + suite + clippy.
Commit `rp32-fase-4: UI selector de arquitectura y registros 64-bit`.

## Fase 5 — Docs y cierre

README/ENSAMBLADOR.md (RV64, conmutación, repertorio W); SPEC nota; BITACORA
consolidada; CLI --rv64 si entró. NO tocar ROADMAP-PRO ni Cap. VI.
**Compuerta F5 (cierre):** cargo test + clippy AMBOS crates + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB + verificación manual RV32 y RV64.
Commit `rp32-fase-5: docs RV64`.

## Criterios de aceptación

- Ejecutar y ensamblar RV64, conmutable RV32/RV64, persistido. ✓ F2-F4.
- Encodings RV64 verificados contra GNU. ✓ F3.
- Registros de 64 bits en la UI en modo RV64. ✓ F4.
- RV32 100% intacto (todos los tests previos verdes, default RV32). ✓ todas.
- Si el spike revela que el alcance es desproporcionado, FRENAR y reportar para
  trocear o reconsiderar — cierre con decisión documentada también es válido. ✓ F1.
