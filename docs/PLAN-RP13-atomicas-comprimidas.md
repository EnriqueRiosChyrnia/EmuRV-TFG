# PLAN-RP13 — Extensiones A (atómicas) y C (comprimidas)

Punto RP-13 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

**Decisión de alcance del autor (2026-06-13):** este punto cubre A + C. RV64
queda DIFERIDO (es esencialmente otro emulador: registros de 64 bits en toda la
UI, syscalls y mapa de memoria — decisión separada del autor, no entra acá).
La extensión C es CONDICIONAL: la Fase 1 es un spike de viabilidad; si C rompe
el invariante del source_map / breakpoints / desensamblador sin solución limpia,
se implementa SOLO A y C se difiere con evidencia.

## Contexto y evidencia previa (verificada por la coordinadora, 2026-06-13)

- **Invariante en riesgo (C):** el ensamblador asume hoy que toda instrucción
  ocupa 4 bytes. `text_off` avanza de a 4 (assembler.rs); `cur_addr` = TEXT_BASE
  + text_off; el source_map mapea dirección↔línea; los breakpoints (RP-7), el
  resaltado de línea, el step y el desensamblador (RP-11) dependen de ese
  tamaño fijo. C introduce instrucciones de 2 bytes → el supuesto cae. ESTE es
  el riesgo que el spike F1 mide.
- **A es de bajo riesgo:** no cambia tamaños (todas las atómicas son de 4 bytes);
  es agregar encodings + ejecución. Unicorn ejecuta A nativo (verificar en spike).
- Oráculo GNU disponible (RP-11/RP-12): `riscv64-elf-as -march=rv32imac` /
  `rv32ima` verifica encodings palabra a palabra. Capstone (RP-11) decodifica
  A y C → el desensamblador debería cubrirlas (verificar, no asumir — C es
  justo donde el disasm de 2 bytes importa).
- Semántica mono-hart: lr/sc y amo* en un solo hart son secuencialmente
  consistentes triviales — SE DOCUMENTA como tal (no se promete concurrencia).
- Mode actual: `Mode::RISCV32` (machine.rs:227). A y C no cambian el modo;
  C puede requerir un flag de Unicorn para habilitar decodificación comprimida
  — el spike lo determina.

## Alcance

- SÍ: extensión A completa (lr/sc + amo*) en ensamblador y ejecución; extensión
  C CONDICIONADA al spike (ensamblado, ejecución, y compatibilidad con
  source_map/breakpoints/disasm); verificación contra GNU; docs.
- NO: RV64 (diferido, decisión del autor); multi-hart / concurrencia real;
  generación automática de instrucciones C por el ensamblador (compresión
  automática de instrucciones de 32→16 bits — eso es optimización de tamaño,
  no didáctica; si C entra, es porque el usuario ESCRIBE c.* explícitas o se
  IMPORTAN en un ELF de RP-11, no porque el assembler comprima solo).

## Fase 1 — Spike: A nativo + viabilidad de C en el source_map

`tests/spike_ac.rs`:
1. **A:** amoadd.w / lr.w+sc.w codificadas a mano ejecutan en Unicorn; resultado
   correcto; round-trip host. Si algo falla, registrar (A es el piso — no debería).
2. **C — la pregunta central:** ¿Unicorn decodifica y ejecuta una instrucción
   comprimida de 2 bytes (p. ej. c.addi) con el modo actual? ¿Necesita flag?
   ¿El pc avanza de a 2 tras una C? Medir con una secuencia mixta 4-2-4 bytes.
3. **C en el source_map (decisión de diseño):** evaluar el costo real de
   permitir tamaños variables. El text_off ya es un contador de bytes — avanzar
   de a 2 es mecánico; lo que hay que auditar es: ¿breakpoints por dirección
   siguen exactos? ¿el resaltado línea↔addr aguanta? ¿el disasm de RP-11 sabe
   que en .text puede haber instrucciones de 2 bytes (Capstone necesita saber
   dónde empieza cada una)? Listar CADA consumidor del supuesto-4-bytes.
4. **DECISIÓN registrada en BITACORA [Seguro]:** C es viable si y solo si el
   source_map y todos sus consumidores soportan tamaño variable con cambios
   acotados y sin romper un solo test existente. Si el costo es desproporcionado
   o rompe invariantes → SOLO A, C diferida con la lista de razones. Bajo la
   regla de conflictos del ROADMAP: ante duda, frenar y reportar al autor.

**Compuerta F1:** spike de A verde + veredicto de C (VIABLE / DIFERIDA) con
evidencia en BITACORA. Commit `rp13-fase-1: spike A nativo y viabilidad de C`.

## Fase 2 — Extensión A (siempre se hace)

Ensamblador: lr.w, sc.w, amoswap.w, amoadd.w, amoand.w, amoor.w, amoxor.w,
amomin.w, amomax.w, amominu.w, amomaxu.w (con aq/rl opcionales — aceptar `.aq`,
`.rl`, `.aqrl` como sufijos; sin sufijo = ambos en cero). Errores didácticos
(amo* necesita formato `rd, rs2, (rs1)`). Verificación palabra-a-palabra contra
`riscv64-elf-as -march=rv32ima`. Ejecución: amoadd sobre una palabra de .data,
lr/sc store-conditional exitoso y fallido (documentar la semántica mono-hart en
el test). Disasm de A (capstone).
**Compuerta F2:** encodings == GNU + suite completa + clippy ambos.
Commit `rp13-fase-2: extensión A (atómicas) verificada contra GNU`.

## Fase 3 — Extensión C (SOLO si F1 dictaminó VIABLE)

Si F1 = DIFERIDA: saltar a Fase 4 (el plan se cierra con A; el commit de F3 no
existe; documentar el salto). Si F1 = VIABLE:
1. source_map y consumidores adaptados a tamaño variable (el cambio acotado que
   el spike validó); CADA test existente de breakpoints/resaltado/disasm sigue
   verde (es la prueba del invariante).
2. Ensamblado de las C más didácticas: c.addi, c.mv, c.li, c.jr, c.nop, c.lw,
   c.sw, c.beqz, c.bnez, c.j (las que un alumno escribiría a mano para ver
   compresión). Verificación contra GNU `-march=rv32imac`.
3. Ejecución de secuencia mixta 4/2 bytes; breakpoint sobre una C detiene
   exacto; disasm de RP-11 lista C correctamente (longitud 2 reconocida);
   importar un ELF de RP-11 con C funciona.
**Compuerta F3:** todo verde + NINGÚN test previo roto + clippy.
Commit `rp13-fase-3: extensión C compatible con source_map y breakpoints`.

## Fase 4 — Docs, ejemplo y cierre

1. Si A solo: ejemplo `ejemplos/14-atomicas.asm` (contador con amoadd,
   comentado, mono-hart explicado). Si A+C: agregar demo de compresión en el
   mismo o uno aparte. Test CLI.
2. ENSAMBLADOR.md: sección A (+ C si entró), tabla de instrucciones, nota de
   semántica mono-hart, nota del oráculo GNU. Si C quedó diferida: registrar en
   ENSAMBLADOR.md y en BITACORA por qué (para que un lector futuro no lo tome
   como olvido). Ayuda + README.
3. BITACORA consolidada. ROADMAP-PRO y Cap. VI: NO tocar (coordinadora —
   actualizará el nombre del punto según A-solo o A+C, y registrará RV64 diferido).
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB.
Commit `rp13-fase-4: ejemplo atómicas y docs A/C`.

## Criterios de aceptación

- Extensión A completa, encodings == GNU as, semántica mono-hart documentada,
  ejecutándose por UI y CLI. ✓ F2.
- Extensión C: implementada SI viable sin romper invariantes (F1 dictamina);
  si no, diferida con evidencia escrita — ambos desenlaces son cierre VÁLIDO
  de este punto. ✓ F1/F3.
- RV64 explícitamente fuera (decisión del autor 2026-06-13). ✓.
- Cero regresión: ni un test de RP-1..RP-12 roto (en especial breakpoints,
  resaltado, disasm si C entra). ✓ F2/F3.
