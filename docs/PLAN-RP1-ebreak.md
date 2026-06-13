# PLAN-RP1 — Corregir bug latente de `ebreak`

Punto RP-1 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto del bug

- **Síntoma (BITACORA.md ~L282, sesión 2026-06-11, [Seguro]):** al reanudar tras un
  `ebreak`, la ejecución cae con «instrucción inválida» más allá de la instrucción
  siguiente. El pc entregado al hook de interrupción ya viene avanzado y el flujo
  post-hook avanza de nuevo → se saltea una instrucción y el flujo queda corrupto.
- **Código involucrado:**
  - `src-tauri/src/core/syscalls.rs:43` — `const EBREAK: u32 = 0x0010_0073;`
  - `src-tauri/src/core/syscalls.rs:70-79` — rama `CAUSE_BREAKPOINT` de `handle_trap`:
    lee la instrucción EN `pc` y solo si es exactamente `EBREAK` hace `pc += 4`, luego
    `emu_stop()`. Si el pc ya viene avanzado, lee la instrucción SIGUIENTE, no ajusta,
    y el doble avance queda sin compensar.
  - Contraste: la rama `CAUSE_ECALL_*` (syscalls.rs:68) asume pc avanzado y usa
    `pc.wrapping_sub(4)` — el `ebreak` necesita la misma claridad de convención.
  - `src-tauri/src/core/machine.rs` — `interpret()`: un stop por `ebreak` hoy cae en
    el caso `Ok(()) → normal` (Paused para run_slice); no hay rama propia.
- **Por qué nunca se vio:** ningún ejemplo ni test del proyecto usa `ebreak`.
- **Regla de oro:** la semántica deseada es la de RARS/gdb — `ebreak` pausa, la GUI
  muestra el estado, y al reanudar la ejecución continúa en la instrucción
  INMEDIATAMENTE SIGUIENTE al `ebreak`, sin saltear ninguna ni re-ejecutar el `ebreak`.

## Alcance

- SÍ: fix del núcleo, tests, documentación (SPEC/ayuda/BITACORA).
- NO: cambios de UI más allá del texto de estado si ya existe; nada de watchpoints ni
  breakpoints condicionales (eso es RP-7). Mejora fuera de alcance → BITACORA
  «PROPUESTAS FUERA DE ALCANCE» y seguir.

## Fase 1 — Reproducción (test rojo)

Crear `src-tauri/tests/rp1_ebreak.rs` con un test que documente el bug:

1. Programa: secuencia conocida con `ebreak` en el medio, p. ej.
   ```asm
   li t0, 1
   ebreak
   li t1, 2
   li t2, 3
   li a7, 10
   ecall
   ```
2. `run_slice` hasta el stop por `ebreak`. Afirmar:
   - el programa NO terminó (no `Exited`),
   - pc tras el stop == dirección del `ebreak` + 4 (la del `li t1, 2`),
   - t0 == 1 (lo previo se ejecutó), t1 todavía == 0 (lo posterior no).
3. Reanudar (`run_slice_continuation` o el camino real que usa commands.rs al
   continuar tras pausa). Afirmar: `Exited{code:0}` y t1 == 2, t2 == 3.

**Compuerta F1:** el test compila y FALLA por el bug real (no por errores del test).
Registrar en BITACORA los valores de pc observados (en el stop y tras reanudar) con
nivel [Seguro]. Commit `rp1-fase-1: test sonda ebreak (rojo, documenta bug)`.
Nota: suite previa debe seguir verde — el rojo permitido es SOLO el test nuevo.

## Fase 2 — Diagnóstico de causa raíz

1. Instrumentar temporalmente la rama `CAUSE_BREAKPOINT` (eprintln o test con hook
   espía): registrar el `pc` exacto que entrega Unicorn al hook, qué instrucción hay
   en `pc` y en `pc-4`, y el pc final tras `emu_stop`.
2. Determinar sin ambigüedad: ¿Unicorn entrega pc YA avanzado (= ebreak+4) o no?
   ¿Quién hace el segundo avance (el `pc+4` condicional del hook, o Unicorn tras
   retornar del hook de interrupción)?
3. Verificar también el caso `step()` sobre un `ebreak` (machine.rs:507) — usa
   `suppress_bp` y `count=1`; puede tener interacción propia.

**Compuerta F2:** causa raíz escrita en BITACORA con evidencia (valores reales de pc)
y nivel [Seguro]. Quitar toda instrumentación temporal. Commit
`rp1-fase-2: diagnóstico causa raíz ebreak`.

## Fase 3 — Fix del núcleo

Diseño según lo que confirme la Fase 2 (no adivinar antes):

- Detección robusta de la dirección del `ebreak`: leer `pc` y `pc-4`; la dirección
  real del `ebreak` es donde esté la palabra `0x0010_0073`. Fijar pc final =
  `dir_ebreak + 4` EXACTAMENTE una vez, cualquiera sea la convención de Unicorn.
- Si el doble avance lo causa Unicorn después del hook: compensar (fijar pc después
  de `emu_stop`, o restar el avance espurio) y dejar comentario explicando la
  convención, espejo del comentario de `CAUSE_ECALL_*`.
- Caso borde: `ebreak` como ÚLTIMA instrucción — al reanudar debe caer en el mensaje
  didáctico existente de «el programa siguió de largo» (CAUSE_ILLEGAL_INSN), no en un
  pánico ni en un error críptico.
- Evaluar si `interpret()` necesita distinguir el stop por `ebreak` (p. ej. para que
  la UI muestre «pausa por ebreak» en vez de «Pausado»). Si requiere tocar el enum
  `StopReason` y la API hacia el frontend, registrar la decisión en BITACORA; si es
  invasivo, dejarlo como propuesta fuera de alcance.

**Compuerta F3:** test de Fase 1 VERDE + suite completa verde + `cargo clippy` sin
warnings. Commit `rp1-fase-3: fix doble avance de pc en ebreak`.

## Fase 4 — Cobertura de bordes

Agregar a `rp1_ebreak.rs`:

1. `ebreak` como última instrucción → reanudar produce el error didáctico de
   «siguió de largo», no pánico.
2. Dos `ebreak` consecutivos → dos pausas, una por cada uno, sin saltear.
3. `ebreak` dentro de un bucle que itera N veces → exactamente N pausas.
4. `step()` sobre la línea del `ebreak` → un solo paso, pc queda en la siguiente,
   reanudar funciona.
5. `ebreak` conviviendo con un breakpoint de UI en otra línea → ambos frenan, ninguno
   interfiere con el otro (atención a `suppress_bp`).

**Compuerta F4:** todo verde + clippy limpio. Commit `rp1-fase-4: cobertura bordes ebreak`.

## Fase 5 — Documentación y cierre

1. SPEC.md: declarar `ebreak` como soportado (pausa por software) — recién ahora que
   el fix existe.
2. Ayuda integrada (panel de ayuda) y ENSAMBLADOR.md: mencionar `ebreak` si el resto
   de instrucciones del repertorio están listadas ahí.
3. BITACORA.md: entrada de cierre — decisión final, convención de pc documentada,
   nivel [Seguro], referencia a los tests.
4. Opcional (si cuesta < 15 min): ejemplo didáctico nuevo que use `ebreak`.
5. `docs/ROADMAP-PRO.md`: NO tocar — el estado lo actualiza la sesión coordinadora.

**Compuerta F5 (cierre):** `cargo test` verde + `cargo clippy` sin warnings +
`cargo tauri build` exitoso. Commit `rp1-fase-5: docs ebreak soportado`.

## Criterios de aceptación (del ROADMAP)

- Test que ensambla `ebreak`, reanuda y sigue ejecutando correctamente. ✓ Fases 1+3.
- `ebreak` documentado como soportado en SPEC/ayuda SOLO después del fix. ✓ Fase 5.
