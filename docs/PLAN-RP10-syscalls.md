# PLAN-RP10 — Syscalls 8/9/93, códigos de salida reales y limpieza de consola

Punto RP-10 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto y evidencia previa (verificada por la coordinadora, 2026-06-12)

- **Tabla oficial de RARS** (wiki del repo TheThirdOne/rars, «Environment Calls»,
  consultada 2026-06-12): ReadString=8 (a0=buffer, a1=máx chars), Sbrk=9
  (a0=bytes → a0=dirección), Exit=10, **Exit2=93** (a0=código).
  **El 17 NO existe en RARS** — era Exit2 de MARS (MIPS); RARS adoptó 93 (número
  del ABI Linux RISC-V). Esto RESUELVE la duda registrada en Cap. VI §6.2.3
  ([SIN VERIFICAR]) — registrar la resolución en BITACORA con la fuente; la
  coordinadora sincroniza el Cap. VI al cierre.
- Dispatch actual (syscalls.rs::dispatch, ~L176): servicios 1, 4, 5, 10, 11,
  30, 32. `StopReason::Exited { code }` ya existe con code siempre 0.
- CLI: emurv-cli/src/main.rs:250 y :374 — `Exited { .. } => 0` (ignora code).
- Mapa de memoria: DATA_REGION 0x1000_0000 + 0x40_0000; .data en 0x1001_0000.
  Heap RARS canónico: **0x1004_0000** (dentro de la región ✓). La pila vive en
  otra región — sin colisión heap/pila posible.
- Panel Consola (src/paneles/consola.ts): tiene `clear()` (lo usa Reiniciar)
  pero NINGÚN botón propio. `ConsoleEvent` (syscalls.rs) hoy: Output, InputRequest.
- Lección 7 (lecciones/07-syscalls/) dice explícitamente que 8/9/93 no existen
  «todavía» — este punto la amplía (F4).

## Alcance

- SÍ: syscalls 8, 9, 93 con semántica RARS; código de salida real propagado
  núcleo→UI→CLI; rechazo didáctico del 17; botón Limpiar en consola; syscall 60
  (extensión EmuRV) de limpieza; lección 7 ampliada; docs.
- NO: free/realloc (sbrk es puntero creciente, como RARS), syscalls de float
  (RP-12), redirección de archivos, formato printf.

## Diseño acordado (no re-litigar)

- **93 (Exit2):** termina con `Exited { code: a0 as i32 }`. UI: estado «terminó
  con código N» (i18n). CLI: el proceso sale con `code & 0xFF` ACOTADO a 0..=125
  (126/127/128+ tienen significado de shell; >125 → 125 y aviso por stderr;
  colisión con el 124 propio del CLI documentada en README: si el invitado
  elige 124, es indistinguible del timeout — limitación aceptada y registrada).
- **17:** mensaje didáctico «el servicio 17 era de MARS (MIPS); en RARS y EmuRV
  la salida con código es el servicio 93» — mejor que un genérico «desconocido».
  OJO RP-8: con mtvec configurado, 17 sigue siendo "desconocido" para la
  delegación → handler del guest PRIMERO si existe; el mensaje didáctico es el
  camino sin mtvec. Test de ambos caminos.
- **8 (ReadString):** semántica RARS documentada en el código: lee una línea de
  la consola (mismo canal InputRequest/InputMsg que read_int), guarda a lo sumo
  a1−1 bytes + NUL en a0; trunca sin error; a1 ≤ 0 → no escribe nada. Buffer
  validado contra regiones escribibles (reusar la validación de write_mem de
  RP-7 — mismos rechazos didácticos).
- **9 (Sbrk):** HEAP_BASE = 0x1004_0000 (constante nueva en core/mod.rs,
  comentada como «canónico RARS»). Puntero creciente en MachData, alineado a 4,
  reset en load()/reset(). Agotamiento (tope de DATA_REGION) → a0 = -1 NO:
  RARS no define error… decisión didáctica: RuntimeError claro («el heap se
  agotó: pediste N bytes y quedan M») — registrar en BITACORA por qué nos
  apartamos (un -1 silencioso es trampa para principiantes). Carga del programa:
  si .data llega a 0x1004_0000, error de ensamblado/carga didáctico («tus datos
  pisan el heap») — test.
- **60 (LimpiarConsola, extensión EmuRV):** `ConsoleEvent::Clear` nuevo →
  la UI vacía el panel (Consola.clear()); el CLI imprime `\x1B[2J\x1B[H` SOLO
  si stdout es TTY (en pipe/CI no ensucia los diffs — usar IsTerminal de std).
  Documentado como extensión propia (no RARS) en Ayuda y docs.
- **Botón «Limpiar»** en el panel Consola: visible siempre, funciona con o sin
  sesión, solo borra texto (la máquina ni se entera), i18n es.json.

## Fase 1 — Exit codes (93) + rechazo didáctico del 17

Núcleo (93 + 17) + UI (estado con código) + CLI (propagación acotada).
Tests: 93 con código 0/7/125/126(→125 + aviso)/negativo (documentar: & 0xFF
estilo Unix); 17 sin mtvec → mensaje didáctico; 17 con mtvec → delegado al
handler (RP-8 intacto); CLI como proceso: exit code real; regresión syscall 10.
**Compuerta F1:** verdes + suite completa + clippy ambos.
Commit `rp10-fase-1: exit2 (93) con código real y rechazo didáctico del 17`.

## Fase 2 — ReadString (8) y Sbrk (9)

Tests: 8 con línea corta/larga (trunca + NUL)/a1=0/buffer en .text (rechazo);
9 con reservas sucesivas alineadas, agotamiento didáctico, reset limpia el
puntero, .data que invade heap → error de carga; programa combinado (lee
nombre con 8, lo guarda en heap de 9, lo imprime) por CLI con pipe.
**Compuerta F2:** verdes + suite + clippy. Commit `rp10-fase-2: readstring y sbrk`.

## Fase 3 — Limpieza de consola (botón + syscall 60)

UI + ConsoleEvent::Clear + CLI con detección TTY. Tests: núcleo emite Clear;
CLI en pipe NO emite ANSI (diff limpio — test de proceso); botón verificado a
mano (BITACORA).
**Compuerta F3:** verdes + tsc limpio. Commit `rp10-fase-3: limpiar consola (botón y syscall 60)`.

## Fase 4 — Lección 7 ampliada + docs + cierre

1. Lección 7: bloque nuevo con 8/9/93 (la nota de «no existen todavía» muere);
   test de salida actualizado.
2. Ayuda integrada: tabla de syscalls completa (8/9/93/60 incluidos, 60 marcado
   «extensión EmuRV»). README ídem. docs/TRAPS.md: nota de que 17 cae en
   «desconocido» para la delegación.
3. BITACORA: resolución de la duda 17-vs-93 con fuente, decisión sbrk-error,
   decisión TTY, niveles de confianza.
4. `docs/ROADMAP-PRO.md` y Cap. VI: NO tocar — coordinadora.
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB.
Commit `rp10-fase-4: lección 7 ampliada y docs syscalls`.

## Criterios de aceptación (del ROADMAP, ajustados por la evidencia)

- 93 → exit code real propagado por emurv-cli. ✓ F1.
- 17: NO se implementa (no existe en RARS — duda del Cap. VI resuelta con
  fuente); rechazo didáctico en su lugar. ✓ F1.
- 8 con límite de buffer y comportamiento RARS documentado. ✓ F2.
- 9 con heap simple y región validada. ✓ F2.
- Botón Limpiar funcional siempre; syscall 60 con test y sin ensuciar pipes. ✓ F3.
- Tests núcleo + proceso; Ayuda y docs actualizadas; lección 7 ampliada. ✓ F2-F4.
