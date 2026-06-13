# PLAN-RP8 — CSRs (Zicsr) y manejo mínimo de traps

Punto RP-8 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto y evidencia previa (verificada por la coordinadora, 2026-06-12)

- **Unicorn 2.1.5 expone los CSRs M-mode** en los bindings generados
  (target/debug/build/unicorn-engine-sys-*/out/bindings.rs): MSTATUS=116,
  MIE=120, MTVEC=121, MSCRATCH=127, MEPC=128, MCAUSE=129, MTVAL=130, MIP=131
  (y los U-mode 33-38). El host puede leerlos/escribirlos con reg_read/reg_write.
- **Incógnitas que el spike DEBE responder** (no asumir):
  1. ¿`csrrw`/`csrrs`/`csrrc` (y variantes inmediatas) ejecutan NATIVAS en el
     guest bajo Unicorn, reflejándose en esos registros del host? ¿O lanzan
     ILLEGAL_INSN?
  2. ¿`mret` ejecuta nativo (pc←mepc, ajuste de mstatus.MPP/MPIE)?
  3. Cuando el guest provoca una excepción (ecall, instrucción ilegal, acceso
     inválido) con mtvec configurado, ¿Unicorn vectoriza solo al handler del
     guest, o SIEMPRE corta a host (hook INTR / uc_error)? Recordar RP-1:
     el trap de ebreak NO llega al hook INTR — el comportamiento por causa
     puede ser heterogéneo. Medir causa por causa.
- Infraestructura existente que este punto toca: handle_trap (syscalls.rs) es
  HOY el dueño de todos los ecall; el rescate de ebreak vive en interpret()
  (RP-1); pseudo-instrucciones y encoder en assembler.rs; panel de registros
  (RP-7 lo dejó editable); las lecciones de RP-9 querrán un ejemplo de trap
  handler (lección 10) — este punto las habilita.

## Alcance

- SÍ: ensamblado Zicsr completo + nombres de CSR, mret, estado CSR visible y
  editable en la UI (modo avanzado), delegación didáctica de excepciones
  SÍNCRONAS al handler del guest, ejemplo didáctico funcionando, docs.
- NO: interrupciones ASÍNCRONAS (timer/software/externas — propuesta futura:
  exige inyectar entre tramos), modos S/U con privilegios reales (M-mode plano),
  CSRs de performance (cycle/instret — propuesta si Unicorn los mantiene),
  delegación de los syscalls RARS existentes (ecall con a7 conocido SIGUE
  siendo syscall del host — el material didáctico previo no puede romperse).

## Diseño acordado (no re-litigar; el spike puede ajustar el CÓMO, no el QUÉ)

### Ensamblador (independiente del spike)
- Instrucciones: csrrw, csrrs, csrrc, csrrwi, csrrsi, csrrci (encoding SYSTEM,
  funct3 1-3/5-7) + mret (0x30200073). Pseudo: csrr rd, csr → csrrs rd, csr, x0;
  csrw csr, rs → csrrw x0, csr, rs; csrs/csrc análogas.
- Nombres de CSR aceptados (tabla propia, número o nombre): mstatus, mie, mtvec,
  mscratch, mepc, mcause, mtval, mip (+ número crudo 0xNNN para los demás, con
  aviso si Unicorn no lo respalda — lo determina el spike). Errores didácticos:
  CSR desconocido sugiere los soportados.
- Tests de encoding contra valores de referencia (objdump de la toolchain GNU o
  tabla de la espec — citar fuente en el test).

### Traps (la forma exacta la fija el spike, la semántica es esta)
- **Política de delegación didáctica:** con mtvec == 0 (estado inicial) TODO se
  comporta como hoy (cero regresión). Con mtvec configurado por el guest:
  - ecall con a7 de syscall CONOCIDA → syscall del host (sin delegar, siempre).
  - ecall con a7 DESCONOCIDA → delegar: mepc=pc del ecall, mcause=11 (ECALL_M),
    mtval=0, pc=mtvec, el handler del guest corre, mret vuelve a mepc+4 (el
    ajuste exacto del +4 lo decide el spike según la convención que Unicorn
    entregue — documentar como en CAUSE_ECALL_*).
  - Instrucción ilegal y acceso a memoria inválido → delegar con mcause 2 / 5-7
    y mtval = instrucción/dirección. Sin mtvec: el error didáctico actual.
  - ebreak: NO se delega (mantiene la semántica RP-1 de pausa didáctica;
    delegarlo = propuesta futura con flag).
- StopReason nuevo NO hace falta a priori (el guest sigue corriendo); si el
  handler del guest a su vez falla (doble falta), cortar con RuntimeError
  didáctico («excepción dentro del manejador de excepciones») — sin loops.
- El panel de estado de la UI muestra «trap delegado al programa (mcause=N)»
  como evento informativo si es barato (evento de consola ya existente).

### UI
- Panel de registros: sección plegable «CSRs (avanzado)» — mstatus, mie, mtvec,
  mscratch, mepc, mcause, mtval, mip; lectura siempre, edición con máquina
  pausada (reusar write de RP-7 con los ids de CSR del binding). Oculta por
  defecto (los principiantes no la necesitan; i18n es.json).

## Fase 1 — Spike de comportamiento de Unicorn

`tests/spike_csr.rs` (patrón spike_mmio/spike_count). Responder con evidencia:
1. csrrw/csrrs/csrrc nativos: escribir mscratch desde el guest, leer del host.
   Variantes inmediatas. ¿Algún CSR de la lista falla?
2. mret: setear mepc/mstatus desde el guest, mret, ¿pc llega a mepc?
3. Excepciones con mtvec seteado por el guest: ecall / instrucción ilegal /
   acceso inválido — ¿quién recibe el control? (hook INTR, uc_error, o vector
   guest). Tabla causa→comportamiento en BITACORA, nivel [Seguro], espejo del
   método RP-1/RP-6.
4. Interacción con el hook INTR ya instalado de syscalls: ¿el hook puede
   reinyectar al guest (escribir mepc/mcause/pc y devolver) sin emu_stop?

**Compuerta F1:** tabla completa en BITACORA + decisión del mecanismo de
delegación fundamentada. Commit `rp8-fase-1: spike CSRs y traps en Unicorn`.

## Fase 2 — Zicsr en el ensamblador

Encoding + nombres + pseudos + errores didácticos + tests de referencia (incl.
round-trip: ensamblar → correr en Unicorn → CSR refleja el valor si el spike
confirmó soporte nativo; si NO lo confirmó, implementar la emulación por hook
ILLEGAL_INSN que el spike haya validado y testearla igual).

**Compuerta F2:** tests verdes + suite completa intacta + clippy.
Commit `rp8-fase-2: ensamblado Zicsr y mret`.

## Fase 3 — Delegación de traps

Implementar la política del diseño con el mecanismo elegido en F1. Tests:
- ecall desconocida con mtvec → handler corre, mret vuelve, programa termina OK.
- ecall conocida (print) con mtvec → syscall host normal (NO delegada).
- Instrucción ilegal con mtvec → handler lee mcause=2 y decide; sin mtvec →
  error didáctico actual (test de regresión).
- Acceso inválido con mtvec → mcause/mtval correctos.
- Doble falta → RuntimeError didáctico, sin colgarse.
- ebreak con mtvec → sigue pausando (RP-1 intacto).
- Convivencia: breakpoint DENTRO del handler funciona; watchpoint sobre dato
  que el handler escribe dispara (RP-7 intacto).

**Compuerta F3:** todo verde + clippy. Commit `rp8-fase-3: delegación didáctica de traps`.

## Fase 4 — UI, ejemplo y documentación

1. Sección CSRs del panel de registros (plegable, editable pausado, i18n).
2. Ejemplo `ejemplos/12-trap-handler.asm`: instala handler en mtvec, dispara
   una ecall personalizada (a7 fuera de tabla), el handler imprime algo y
   retorna con mret; comentado a fondo (es el germen de la lección 10 de RP-9).
   Test CLI con salida esperada.
3. docs: sección nueva en ENSAMBLADOR.md (instrucciones Zicsr + tabla de CSRs),
   docs/TRAPS.md o sección en DEPURACION.md (la política de delegación completa,
   el porqué didáctico, límites — sin interrupciones asíncronas), Ayuda, README.
4. BITACORA consolidada. `docs/ROADMAP-PRO.md`: NO tocar.

**Compuerta F4 (cierre):** cargo test + clippy verdes AMBOS crates + tsc limpio +
ci-local.sh --skip-bundle verde + cargo tauri build + bundle ≤ 30 MB.
Commit `rp8-fase-4: panel CSRs, ejemplo trap handler y docs`.

## Criterios de aceptación (del ROADMAP)

- csrr/csrw/csrrs/csrrc en el ensamblador. ✓ F2.
- CSRs básicos (mstatus, mtvec, mepc, mcause) + mret. ✓ F2/F3.
- Ejemplo didáctico de trap handler funcionando (ecall inválido → handler →
  mret). ✓ F4 (criterio del ROADMAP, literal).
- Panel de registros muestra CSRs relevantes en modo avanzado. ✓ F4.
- Extra fijado por este plan: cero regresión con mtvec sin configurar (todo el
  material didáctico previo intacto — tests de regresión explícitos). ✓ F3.
