# PLAN-RP29 — ELF reubicable (independiente de la dirección de carga)

Punto RP-29 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas: PROHIBIDO
avanzar con la compuerta en rojo. Se ejecuta en su WORKTREE (rama
`rp29/elf-reubicable`), área `src-tauri/src/core/` (Rust) — disjunta del lote
(los otros 3 son frontend). Origen: Cap. VI §6.2.3.

## Contexto

- `src-tauri/src/core/elf_export.rs`: `export_elf(&Program) -> Vec<u8>` arma un
  ELF32 RISC-V con direcciones ABSOLUTAS del mapa RARS (.text 0x00400000, .data
  0x10010000). Hoy el binario asume cargarse en esas direcciones fijas.
- `src-tauri/src/core/assembler.rs`: el ensamblador; `la`/cargas de dirección y
  saltos se codifican con direcciones del mapa fijo (lui+addi con %hi/%lo, o
  absolutas). El acceso a datos depende de la dirección absoluta de .data.
- Oráculo GNU disponible (riscv64-elf-as/ld/objdump) para verificar.

## Alcance

- SÍ: poder generar un ELF cuyo CÓDIGO no dependa de cargarse en una dirección
  fija — referencias a datos/símbolos vía PC-relativo (`auipc`+offset) en vez de
  absoluto, de modo que el binario sea reubicable. Verificación con la toolchain
  GNU (cargar en otra dirección y que funcione, o comparar el patrón auipc).
- NO: PIE/relocations dinámicas completas del runtime loader; cambiar el mapa de
  memoria por defecto de la app (el modo normal sigue absoluto). El reubicable es
  una OPCIÓN de exportación, no el default — el flujo educativo no cambia.

## Diseño (un spike primero — el alcance técnico es incierto)

- **Spike (F1):** determinar la vía más limpia. Opciones a evaluar:
  1. Que el ensamblador emita las referencias a símbolos de datos con
     `auipc rd, %pcrel_hi(sym)` + `addi rd, rd, %pcrel_lo` (PC-relativo estándar
     RISC-V) en un MODO reubicable, en vez de `lui`+`%hi/%lo` absoluto.
  2. O exportar con relocations en el ELF (R_RISCV_PCREL_HI20/LO12) y dejar que
     un linker las resuelva — más cercano a un .o que a un ejecutable.
  La opción 1 (PC-relativo directo) es la más autocontenida (no depende de
  linker externo) y la más didáctica. El spike confirma viabilidad y costo, y
  responde en BITACORA con evidencia (objdump del binario reubicable cargado en
  dos direcciones distintas dando el mismo resultado).
- **Activación:** una opción de exportación «ELF reubicable» (flag en el comando
  de export / CLI `emurv-cli asm --reubicable`), separada del export normal.
- **Cuidado:** el `source_map` y la ejecución normal en la app NO cambian (siguen
  con el mapa absoluto); esto solo afecta el ARTEFACTO exportado bajo la opción.

## Fase 1 — Spike de viabilidad

`tests/spike_reubicable.rs` (patrón de spikes del proyecto): probar el patrón
auipc/%pcrel para una referencia a dato; verificar contra GNU as/objdump que el
encoding es correcto y que cargar en otra base da el mismo comportamiento.
Decisión (vía 1 vs 2) en BITACORA [Seguro] con evidencia.
**Compuerta F1:** viabilidad demostrada + decisión fundada; suite intacta;
clippy. Commit `rp29-fase-1: spike ELF reubicable (auipc/pcrel)`.

## Fase 2 — Export reubicable + verificación GNU

Implementar la opción de export reubicable según F1 (ensamblador en modo
PC-relativo y/o relocations en el ELF). Tests: el binario reubicable, cargado/
enlazado en una dirección distinta a 0x00400000, produce el mismo resultado
(verificación con la toolchain GNU como oráculo); el export normal (absoluto)
sigue idéntico (regresión). CLI: flag `--reubicable` en `emurv-cli asm`.
**Compuerta F2:** reubicable verificado contra GNU + export normal intacto +
suite + clippy AMBOS crates.
Commit `rp29-fase-2: export ELF reubicable verificado contra GNU`.

## Fase 3 — Docs y cierre

README/ENSAMBLADOR.md (la opción reubicable, cuándo usarla, flag CLI); BITACORA
(decisión técnica, limitaciones). NO tocar ROADMAP-PRO ni Cap. VI.
**Compuerta F3 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB.
Commit `rp29-fase-3: docs ELF reubicable`.

## Criterios de aceptación

- Opción de export que produce un ELF reubicable (no atado a 0x00400000),
  verificado con la toolchain GNU. ✓ F1/F2.
- Export normal (absoluto) y ejecución en la app intactos. ✓ F2.
- Flag en emurv-cli. ✓ F2.
- Si el spike muestra que la vía elegida es desproporcionada, FRENAR y reportar
  (regla de conflictos del ROADMAP) — un cierre con la decisión documentada y
  alcance reducido también es válido. ✓ F1.
