# Resultados: compliance formal RISCOF — EmuRV vs Sail (RT-3)

Ejecución de [riscv-arch-test](https://github.com/riscv-non-isa/riscv-arch-test)
con [RISCOF](https://riscof.readthedocs.io), comparando la *signature* de cada
test entre EmuRV (DUT) y el **modelo dorado Sail** — el modelo formal oficial
de RISC-V International. A diferencia de RT-2 (tests self-checking), acá el
veredicto lo da la comparación contra la referencia de la especificación.
Material citable del Capítulo V.

- **RISCOF:** 1.25.3 · **Sail:** binario oficial prebuilt 0.13 (Mac-arm64,
  sha256 verificado) · **riscv-arch-test:** `ctp-release-e9514aa-2025-12-28`
  (commit `281d71ef3d61e32111217b20305ea3ef9b1582e2`)
- **Toolchain:** riscv64-elf-gcc (GCC) 16.1.0 (Homebrew)
- **DUT:** `riscof_runner` (src-tauri/src/bin/) — carga por `elf_import`, corre
  la Machine real hasta `EbreakPause`, vuelca la signature (granularidad 4)
- **Plugin:** `tests-riscv/riscof-plugin-emurv/` (DUT + referencia adaptada a
  la CLI de Sail 0.13) · **Reproducción:** `scripts/run-riscof.sh setup && run <suite>`
- **ISA declarada del DUT:** RV32IMC_Zicsr_Zifencei (misa 0x40001104)
- **Reportes HTML de RISCOF archivados:** `docs/evidencia/riscof/report-*.html`
- **Fecha de ejecución:** 2026-07-22

## Tabla de resultados

| Suite (rv32i_m/) | Tests | Match | Mismatch |
|---|---:|---:|---:|
| I | 38 | 38 | 0 |
| M | 8 | 8 | 0 |
| C | 27 | 26 | 1 (`cebreak-01`) |
| Zifencei | 1 | 0 | 1 (`Fencei`) |
| **Total** | **74** | **72** | **2** |

**72/72 de los tests compatibles con el modelo de ejecución de EmuRV producen
signatures idénticas a Sail (100 %).** Los 2 mismatches no son errores de
semántica ISA: ambos chocan con decisiones de diseño didácticas deliberadas
de EmuRV, verificadas a causa raíz (abajo).

## Mismatches (causa raíz verificada)

### `cebreak-01` (C) — `ebreak` es del depurador, no del guest

El test ejecuta `c.ebreak` esperando la excepción Breakpoint (mcause=3)
delegada a su trap handler, que la registra en la signature. En EmuRV,
`ebreak` está **redefinido como pausa del depurador** (decisión RP-1: es el
mecanismo de breakpoints por software de la UI) y nunca se delega al guest.
El runner reporta la terminación anómala y el test queda Failed por
signature-marcador. Incompatibilidad por diseño, no defecto.

### `Fencei` (Zifencei) — código auto-modificado vs W^X

El test escribe una instrucción en `.text` (dirección 0x0040020C) y ejecuta
`fence.i`. EmuRV mapea `.text` como R+X sin escritura (política didáctica
W^X) y detiene con error pedagógico «el programa intentó escribir en una
dirección que no pertenece a .data…». Misma familia que las exclusiones
`fence_i`/`rvc` de RT-2 (`docs/RESULTADOS-RISCV-TESTS.md`).

## Alcance y suites no ejecutadas

- **privilege**: fuera de alcance declarado (PLAN-RT3) — ejercita semántica
  M-mode (ecall/ebreak/traps de privilegio) que EmuRV delega en políticas
  didácticas propias (BITACORA RP-8: ecall conocida = syscall del host,
  jamás delegada; ebreak = pausa).
- **F/D**: diferidas — exigirían extender la ISA declarada y el ABI del
  plugin, y la corrección F/D ya quedó evidenciada en RT-2 (31/33 con el
  hallazgo fmin/fmax de Unicorn 2.1.5 documentado); costo no trivial sin
  información nueva esperable.
- **RV64**: mismo diferimiento que RT-2 (`elf_import` es ELF32).

## Método (resumen)

RISCOF compila cada test dos veces — una con el env del DUT (linker en el
mapa de EmuRV: TEXT_BASE 0x0040_0000 / DATA_BASE 0x1001_0000, HALT =
`ebreak` de 32 bits) y otra con el env de la referencia (mapa estándar
0x8000_0000 + HTIF para Sail) — y compara los volcados de la región
begin_signature/end_signature. Las signatures son independientes de las
direcciones por diseño de la suite. Hallazgo de implementación: el runner
debe volcar exactamente `begin_signature`/`end_signature` (incluye el
padding final de alineación); las labels internas `rvtest_sig_begin/end`
terminan antes del padding y desalinean la comparación.
