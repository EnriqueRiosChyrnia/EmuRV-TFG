# PLAN-RP6 — Stub GDB remoto (protocolo gdbserver)

Punto RP-6 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto y evidencia previa (verificada 2026-06-12 por la sesión coordinadora)

- **udbserver 0.3.0** (crates.io, actualizado 2026-03): stub GDB para Unicorn.
  Deps: `gdbstub ^0.7.9`, `unicorn-engine ^2.1.4` → COMPATIBLE con el
  `unicorn-engine = "2.1.5"` del proyecto (semver unifica: una sola compilación
  nativa de Unicorn, sin conflicto de versiones). Repo: github.com/bet4it/udbserver.
- **ELF exportado SIN símbolos** (elf_export.rs no escribe .symtab; `file` lo
  reporta «stripped») — una sesión gdb verá direcciones, no nombres. Mejora
  opcional en F3.
- Cultura de spikes del proyecto: tests/spike_mmio.rs, tests/spike_count.rs —
  este plan arranca igual (PoC antes de comprometer arquitectura).
- `Machine` encapsula el `Unicorn<'_, MachData>`; udbserver necesita `&mut Unicorn`.
- La app Tauri NO participa: el stub vive en el CLI. El bundle de la app no puede
  crecer (gate RNF-04 lo vigila — lección de RP-3).

## Alcance

- SÍ: feature opcional en la lib, subcomando `emurv-cli debug`, tests RSP
  automatizados sin gdb externo, sesión gdb real documentada con transcript,
  docs.
- NO: integración del depurador en la UI Tauri (fase visual; registrar propuesta),
  DWARF/info de líneas (solo direcciones y, si es barato, símbolos), watchpoints
  vía stub más allá de lo que udbserver dé gratis (RP-7 cubre watchpoints
  nativos), soporte multi-hilo del protocolo.

## Diseño acordado (no re-litigar)

- **Dependencia:** `udbserver = "0.3"` como dependencia OPCIONAL de la lib
  `emurv` (src-tauri/Cargo.toml) detrás de feature `gdb`, sin default. El crate
  Tauri compila sin la feature (su build y bundle no cambian un byte).
  `emurv-cli` depende de `emurv` con `features = ["gdb"]`.
- **Acceso al motor:** método `#[cfg(feature = "gdb")] Machine::unicorn_for_debug
  (&mut self) -> &mut Unicorn<'static, MachData>` — documentado como puerta
  exclusiva del stub (comentario: NO usarlo para otra cosa; los invariantes de
  Machine se saltean bajo responsabilidad del depurador).
- **Subcomando:** `emurv-cli debug <archivo.s> [--port 3333]` — ensambla (con
  includes RP-5), carga la máquina, instala udbserver y ejecuta esperando al
  cliente. stdout sigue siendo exclusivo del programa invitado (convención RP-3);
  los mensajes del stub («esperando conexión gdb en :3333») van por stderr.
- **Tests sin gdb externo:** los tests de integración hablan RSP (Remote Serial
  Protocol) crudo por TCP desde Rust — sin depender de que el runner tenga un
  gdb riscv. Mínimo cubierto: `qSupported`, `?` (halt reason), `g` (leer
  registros), `m` (leer memoria), `Z0`/`z0` (breakpoint por software), `c`
  (continue), `s` (step). La sesión gdb REAL (humana) se documenta aparte con
  transcript pegado en BITACORA y docs.
- **Plan B (si el spike de F1 falla):** si udbserver resulta inviable con los
  hooks propios (teclado MMIO, syscalls INTR), el plan alternativo es un mini
  servidor RSP propio sobre `gdbstub 0.7` directamente (el protocolo mínimo de
  arriba, ~300 líneas). La decisión la toma la evidencia del spike y queda en
  BITACORA con nivel [Seguro]. NO empezar por el plan B.

## Fase 1 — Spike de viabilidad (PoC)

1. Agregar la feature `gdb` + dep opcional en la lib; `cargo build` del crate
   Tauri SIN feature debe quedar idéntico (verificar que `cargo tree` no arrastre
   udbserver sin la feature).
2. `tests/spike_gdb.rs` (en emurv-cli o en la lib con la feature, donde menos
   fricción haya): máquina con programa trivial, `udbserver::udbserver(...)` en
   un puerto efímero, cliente TCP del propio test: `qSupported` responde, `g`
   devuelve registros con el pc esperado, `m` lee una palabra conocida de .text.
3. Preguntas que el spike DEBE responder (BITACORA, con evidencia):
   - ¿udbserver convive con los hooks ya instalados (INTR de syscalls, MEM_READ
     del teclado)? ¿Se pisan? ¿Orden de instalación importa?
   - ¿Cómo interactúa con `emu_start` por tramos (count)? ¿El stub necesita su
     propio modelo de ejecución (continue bloqueante) y por tanto el subcomando
     debug NO usa run_slice? (Hipótesis: el stub maneja su propio loop; el modo
     debug es un modo de ejecución distinto — confirmarlo o refutarlo.)
   - ¿`Z0` (breakpoint) funciona sobre RISC-V RV32 en udbserver 0.3?
   - ¿Costo de compilación y licencia de udbserver/gdbstub? (verificar MIT/Apache;
     registrar en BITACORA — la lib es la misma que usa la app GPL-compatible.)

**Compuerta F1:** spike verde con las preguntas respondidas en BITACORA
[Seguro/Probable] + decisión udbserver-vs-plan-B tomada y fundamentada + crate
Tauri sin feature compila idéntico. Commit `rp6-fase-1: spike udbserver (PoC RSP)`.

## Fase 2 — Subcomando `debug` + protocolo mínimo verificado

1. `emurv-cli debug <archivo.s> [--port N]` según el diseño. Manejo limpio de:
   puerto ocupado (error claro, exit 1), Ctrl-C (cortar sin pánico), cliente que
   desconecta (terminar con mensaje, no colgar).
2. Tests RSP de integración (proceso real, patrón CARGO_BIN_EXE de RP-3):
   - `g`: registros coherentes tras N steps (`s`) — comparar con la semántica
     conocida del programa (li/addi conocidos).
   - `m`: leer .data y verificar contra el fuente.
   - `Z0` + `c`: detiene exactamente en la dirección del breakpoint (usar
     Program.symbols para calcular la dirección esperada de una etiqueta).
   - `z0` + `c`: tras quitar el breakpoint, corre hasta el final.
3. Syscalls durante la sesión: programa que imprime con ecall — la salida llega
   a stdout del proceso debug mientras el cliente está conectado (test lo
   verifica). Si el spike reveló incompatibilidad (p. ej. el hook INTR del stub
   pisa el de syscalls), aplicar la solución decidida en F1 y testearla.

**Compuerta F2:** tests RSP verdes + suite completa de ambos crates verde +
clippy. Commit `rp6-fase-2: subcomando debug con protocolo RSP verificado`.

## Fase 3 — Convivencia con EmuRV real + símbolos (si baratos)

1. Teclado MMIO: sesión debug sobre un programa que lee 0xFFFF0000 — el hook
   MEM_READ sigue funcionando bajo el stub (test: inyectar no es posible sin UI,
   pero leer CONTROL=0 con `m` y que el programa vea 0 al hacer lb, sí).
2. Programa multi-archivo (RP-5): debug de ejemplos/11-multiarchivo/main.s —
   breakpoint en dirección de una etiqueta definida en un include.
3. **Símbolos en el ELF exportado (opcional, solo si es barato con el crate
   `object` ya presente):** escribir .symtab con Program.symbols (etiquetas
   globales al menos). Beneficio directo: `b etiqueta` y `info symbol` en gdb
   real. Si `object` con `write_core` no lo permite sin pelea, registrar como
   propuesta fuera de alcance y seguir — NO es bloqueante.
4. Sesión gdb REAL documentada: con un gdb riscv local (`brew install
   riscv64-elf-gdb` o el que haya), transcript completo: conectar, `info
   registers`, breakpoint, continue, step, leer memoria, desconectar. Pegar en
   BITACORA. Si no hay gdb instalable en la máquina, hacerlo con lldb o
   documentar el bloqueo honestamente (los tests RSP cubren el protocolo igual).

**Compuerta F3:** tests de convivencia verdes + transcript real (o bloqueo
documentado) en BITACORA. Commit `rp6-fase-3: convivencia MMIO/multiarchivo y sesión real`.

## Fase 4 — Documentación y cierre

1. `docs/DEPURACION-GDB.md`: qué es, cómo arrancar (`emurv-cli debug`), cómo
   conectar gdb/lldb/VS Code (launch.json de ejemplo para la extensión nativa de
   VS Code con `miDebuggerServerAddress`), el transcript de la sesión real, la
   tabla del protocolo soportado, limitaciones (sin DWARF; direcciones o símbolos
   según F3.3; un cliente a la vez).
2. README: sección breve enlazando a DEPURACION-GDB.md.
3. BITACORA: decisiones consolidadas (udbserver vs plan B, feature-gate,
   licencias, hallazgos de convivencia), niveles de confianza.
4. `docs/ROADMAP-PRO.md`: NO tocar — lo gestiona la sesión coordinadora.

**Compuerta F4 (cierre):** `cargo test` + clippy verdes en AMBOS crates +
`scripts/ci-local.sh --skip-bundle` verde + `cargo tauri build` + gate de bundle
≤ 30 MB (la app NO debe crecer: la feature gdb no entra en su build) + tsc limpio.
Commit `rp6-fase-4: docs depuración GDB`.

## Criterios de aceptación (del ROADMAP)

- Sesión gdb real contra un ejemplo del repo: breakpoint, step, lectura de
  registros y memoria. ✓ F3 (transcript) + F2 (protocolo automatizado).
- Documentar el flujo en docs/. ✓ F4.
- Extra de este plan: protocolo RSP testeado en CI sin gdb externo (los tests no
  dependen del entorno del runner). ✓ F2.
- La app Tauri no cambia ni un byte (feature-gate verificado + gate de bundle). ✓ F1/F4.
