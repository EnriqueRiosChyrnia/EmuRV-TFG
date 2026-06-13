# PLAN-RP3 — Modo CLI / headless

Punto RP-3 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto

- **Objetivo:** ensamblar y ejecutar programas sin UI, desde terminal. Habilita
  corrección automática de tareas, scripting y CI (RP-4 depende de esto solo
  indirectamente — la CI corre `cargo test`; el CLI suma la posibilidad de testear
  ejemplos end-to-end como procesos).
- **Base ya existente (no rediseñar):**
  - `src-tauri/Cargo.toml`: paquete `emurv`, lib `emurv_lib` con `pub mod core`.
  - `assemble(src) -> Result<Program, AsmError>` (core/assembler.rs).
  - `Machine::new(program, out_tx: Sender<ConsoleEvent>, in_rx: Receiver<InputMsg>)`
    — patrón exacto en `tests/tec1_detener.rs::machine_for`. `new_with_video` existe
    pero el CLI NO lo necesita (las regiones de framebuffer están mapeadas igual;
    verificado por tests FB0).
  - `ConsoleEvent::{Output(String), InputRequest}` / `InputMsg::{Line, Cancel}`
    (core/syscalls.rs) — el puente stdin/stdout del CLI es exactamente esto.
  - `export_elf(&Program) -> Result<Vec<u8>, String>` (core/elf_export.rs).
  - `StopReason::{Exited, Breakpoint, EbreakPause, Paused, StepDone, RuntimeError}`.
  - `run_slice` / `run_slice_continuation` (tramos por count, TEC1) — el CLI usa el
    mismo bucle de tramos que producción, no `run` sin límite.
- **Ejemplos del repo (9, no 10 — el criterio del ROADMAP se ajusta):**
  01-hola-mundo, 02-suma-interactiva, 03-contador, 04-fibonacci, 05-pila-funciones
  (consola pura); 07-franjas, 08-tetris-grafico (framebuffer); 09-eco-teclado,
  10-tetris-tiempo-real (teclado MMIO en tiempo real — en headless no hay teclas:
  corren acotados, no interactivos).

## Alcance

- SÍ: binario CLI dentro del crate existente, subcomandos `asm` y `run`, puente
  stdin/stdout, códigos de salida, tests de integración como proceso, README.
- NO: refactor a workspace/crates separados (proponer en BITACORA si parece valioso),
  TUI interactiva, modo verbose/trace de instrucciones, carga de ELF externos (eso
  es futuro de RP del roadmap aparte). Parsing de argumentos a mano con `std::env`
  — nada de `clap` salvo justificación registrada en BITACORA (regla CLAUDE.md de
  dependencias).

## Diseño acordado (decisiones ya tomadas — no re-litigar)

- **Binario:** `src-tauri/src/bin/emurv-cli.rs`, target `[[bin]] name = "emurv-cli"`.
  Convive con el bin Tauri por defecto; `cargo build --bin emurv-cli` no requiere
  frontend. Si compilar el bin arrastra las dependencias de Tauri: aceptado en esta
  versión (anotar en BITACORA como deuda menor; feature-gate es propuesta futura).
- **Subcomandos:**
  - `emurv-cli asm <entrada.s> [-o <salida.elf>]` — ensambla y exporta ELF32.
    Default de salida: nombre de entrada con extensión `.elf`.
  - `emurv-cli run <entrada.s> [--max-instr N] [--timeout-s S]` — ensambla y ejecuta.
  - `emurv-cli --help` / `--version` — texto en español (los identificadores y flags
    en inglés, convención del repo).
- **Puente de consola en `run`:**
  - `ConsoleEvent::Output(s)` → stdout (sin agregar saltos de línea propios).
  - `ConsoleEvent::InputRequest` → leer UNA línea de stdin → `InputMsg::Line`.
    EOF de stdin → `InputMsg::Cancel` + mensaje didáctico a stderr + exit 1.
  - stderr es el canal de TODOS los diagnósticos (errores de ensamblado con su
    formato didáctico en español intacto, errores de runtime, avisos). stdout es
    EXCLUSIVAMENTE la salida del programa invitado — crítico para corrección
    automática por diff.
- **Semántica headless de pausas:** `EbreakPause` y `Breakpoint` no tienen UI que
  mostrar: el CLI imprime un aviso por stderr (`ebreak en 0x... — continuando`) y
  continúa con `run_slice_continuation`. Registrar en BITACORA. (Un flag futuro
  `--pause-on-ebreak` es propuesta fuera de alcance.)
- **Códigos de salida del proceso:**
  - `0` — el programa invitado terminó (`Exited`; hoy siempre code 0, syscall 10).
  - `1` — error de runtime del programa invitado (mensaje didáctico a stderr).
  - `2` — error de ensamblado (mensaje didáctico a stderr).
  - `3` — uso inválido de la línea de comandos.
  - `124` — se agotó `--max-instr` o `--timeout-s` sin terminar (convención de
    `timeout(1)`). Es el código esperado para los ejemplos 09/10 en headless.

## Fase 1 — Esqueleto + subcomando `asm`

1. Alta del `[[bin]]` en Cargo.toml y `src/bin/emurv-cli.rs` con parsing manual de
   argumentos, `--help`/`--version` en español, exit 3 ante uso inválido.
2. `asm`: leer archivo, `assemble()`, `export_elf()`, escribir salida. Errores de
   lectura/ensamblado/escritura → stderr + exit 2 (ensamblado) o 1 (E/S).
3. Tests `src-tauri/tests/rp3_cli.rs` usando `env!("CARGO_BIN_EXE_emurv-cli")` +
   `std::process::Command`: (a) `asm` de 01-hola-mundo produce archivo con magic
   ELF (`\x7fELF`) y class 32-bit; (b) fuente con error de sintaxis → exit 2 y
   stderr contiene el mensaje en español con número de línea; (c) sin argumentos →
   exit 3 y uso por stderr.

**Compuerta F1:** tests nuevos verdes + suite completa verde + clippy limpio.
Commit `rp3-fase-1: bin emurv-cli con subcomando asm`.

## Fase 2 — Subcomando `run` (consola)

1. Bucle de ejecución: `Machine::new` + `run_slice`/`run_slice_continuation` en
   tramos (mismos valores de count/timeout que producción), drenando `out_rx`
   tras cada tramo y atendiendo `InputRequest` con el puente de stdin.
2. Mapeo de `StopReason` a códigos de salida según el diseño acordado.
3. Tests como proceso: (a) 01-hola-mundo → stdout exacto esperado, exit 0;
   (b) 02-suma-interactiva con stdin por pipe (`echo "5\n7"`) → suma correcta;
   (c) 03, 04 y 05 → stdout esperado (tomar los valores del propio ejemplo);
   (d) EOF prematuro en 02 → exit 1 con diagnóstico en stderr;
   (e) fuente con bucle infinito inline + `--max-instr 1000000` → exit 124 en
   tiempo acotado (el test debe tener su propio guard de timeout, patrón
   `con_guardia` de tec1).

**Compuerta F2:** todo verde + clippy. Commit `rp3-fase-2: subcomando run con puente de consola`.

## Fase 3 — Bordes headless

1. Ejemplos gráficos y de tiempo real, acotados: 07 y 08 con `--max-instr`
   razonable → corren sin pánico y terminan (07/08 terminan solos o por límite —
   documentar cuál); 09 y 10 → exit 124 limpio por límite, sin colgarse (en
   headless la cola MMIO está siempre vacía).
2. `ebreak` headless: fuente con `ebreak` en el medio → aviso por stderr,
   continúa, exit 0; salida del programa intacta en stdout.
3. `--timeout-s` con programa que duerme (syscall 32) → corta limpio en exit 124.
4. Rutas de archivo inexistentes / sin permiso de escritura en `-o` → exit 1 con
   mensaje claro, sin pánico ni backtrace.

**Compuerta F3:** todo verde + clippy. Commit `rp3-fase-3: bordes headless del CLI`.

## Fase 4 — Documentación y cierre

1. README.md: sección «Uso por línea de comandos» con los subcomandos, flags,
   códigos de salida y dos ejemplos reales (asm → ELF; run con pipe de entrada).
   Mencionar el caso de uso de corrección automática.
2. BITACORA.md: decisiones registradas (parsing manual vs clap, semántica headless
   de ebreak/breakpoints, código 124, deuda del bin que arrastra deps de Tauri si
   aplica), niveles de confianza.
3. `docs/ROADMAP-PRO.md`: NO tocar — lo gestiona la sesión coordinadora.

**Compuerta F4 (cierre):** `cargo test` verde + `cargo clippy` sin warnings +
`cargo build --release --bin emurv-cli` exitoso + `cargo tauri build` exitoso
(la app de escritorio no debe romperse por el bin nuevo).
Commit `rp3-fase-4: docs CLI`.

## Criterios de aceptación (del ROADMAP, ajustados al repo real)

- Los 5 ejemplos de consola (01–05) corren desde terminal con salidas correctas,
  verificado por tests como proceso. ✓ Fase 2.
- Entrada estándar conectada a la syscall 5 (vía pipe). ✓ Fase 2.
- Exit code ≠ 0 ante error de ensamblado (=2) y de runtime (=1). ✓ Fases 1–2.
- Ejemplos gráficos/tiempo real (07–10) corren acotados sin colgarse en headless. ✓ Fase 3.
- Documentado en README. ✓ Fase 4.
- Nota: el ROADMAP decía «los 10 ejemplos»; el repo tiene 9 (no existe 06) y los de
  tiempo real no son interactivos sin UI — este plan fija el criterio honesto.
