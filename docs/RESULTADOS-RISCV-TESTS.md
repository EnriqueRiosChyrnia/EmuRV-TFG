# Resultados: riscv-tests oficiales sobre EmuRV (RT-2)

Ejecución de las suites user-level RV32 de
[riscv-software-src/riscv-tests](https://github.com/riscv-software-src/riscv-tests)
contra la Machine real de EmuRV, con cada ELF entrando por `elf_import`
(el mismo camino que un ELF del usuario). Material citable del Capítulo V.

- **Commit de riscv-tests:** `34e6b6d1e7936b526075432fb730d89148623484` (2026-06-03)
- **Toolchain:** riscv64-elf-gcc (GCC) 16.1.0 (Homebrew), `-nostdlib -nostartfiles -mcmodel=medany`
- **Entorno de enlace:** `tests-riscv/env-emurv/` (propio; reemplaza al env `p`
  oficial — ver §Método)
- **Harness:** `src-tauri/tests/rt2_riscv_tests.rs`, corre en `cargo test` y por
  lo tanto en el job `test-macos` de la CI
- **Regeneración de fixtures:** `scripts/build-riscv-tests.sh` (procedencia y
  sha256 en `src-tauri/tests/fixtures/riscv-tests/MANIFEST.txt`)
- **Fecha de ejecución:** 2026-07-22

## Tabla de resultados

| Suite | ISA (`-march`) | Tests | Pasan | Excluidos | Fallan |
|---|---|---:|---:|---:|---:|
| rv32ui | rv32i_zicsr_zifencei | 42 | 41 | 1 (`fence_i`) | 0 |
| rv32um | rv32im_zicsr | 8 | 8 | 0 | 0 |
| rv32ua | rv32ia_zicsr | 10 | 10 | 0 | 0 |
| rv32uc | rv32ic_zicsr | 1 | 0 | 1 (`rvc`) | 0 |
| rv32uf | rv32if_zicsr | 11 | 10 | 1 (`fmin`) | 0 |
| rv32ud | rv32ifd_zicsr | 10 | 9 | 1 (`fmin`) | 0 |
| **Total** | | **82** | **78** | **4** | **0** |

**78/78 de los tests ejecutables pasan (100 %).** Las 4 exclusiones tienen
causa raíz analizada y verificada (abajo); ninguna es un fallo de corrección
de EmuRV sobre programas válidos dentro de su modelo de ejecución.

## Exclusiones (causa raíz verificada)

### `fence_i` (rv32ui) y `rvc` (rv32uc) — política W^X de EmuRV

Ambos tests requieren **escribir y/o ejecutar código fuera de `.text`**:

- `fence_i.S` parchea instrucciones con `sh` y salta a código ubicado en el
  segmento de datos (código auto-modificado, es lo que el test prueba).
- `rvc.S` subtest 6 hace `c.sw` sobre la etiqueta `data` definida *dentro* de
  la sección de código.

El env `p` oficial enlaza todo en un único segmento RWX en `0x8000_0000`.
EmuRV, por decisión didáctica deliberada, mapea `.text` como R+X (sin
escritura) y los datos sin permiso de ejecución, y reporta «el pc saltó fuera
del código del programa» como error pedagógico. Los tests fallan por la
política de memoria, no por decodificación o semántica de instrucciones: el
resto de `rvc.S` (expansión de todas las comprimidas) está cubierto por los
tests propios de RP-13 y el oráculo GNU.

### `fmin` (rv32uf y rv32ud) — defecto conocido del motor upstream

Verificado con una sonda aislada sobre la Machine: `fmax.s` con operandos
`(1.0, sNaN)` devuelve `0x7fc00000` (qNaN canónico) en vez de `1.0`; el flag
NV sí se levanta. Es la semántica IEEE 754-2008 minNum/maxNum previa a la
spec RISC-V F v2.2: QEMU la corrigió en 6.0 (754-2019
minimumNumber/maximumNumber), pero Unicorn 2.x —el motor de ejecución de
EmuRV— sigue basado en QEMU 5.0.1. El subtest 20 de `fmin.S` detecta
exactamente ese caso. Hallazgo registrado como limitación heredada del motor;
irrelevante para el uso educativo de EmuRV (RV32IM es el núcleo curricular).

## Método

riscv-tests separa los tests (portables) del entorno de ejecución (`env`,
reemplazable por diseño). Se construyó `tests-riscv/env-emurv/`:

- `link.ld`: `.text` en `0x0040_0000` (TEXT_BASE) y datos en `0x1001_0000`
  (DATA_BASE) — el mapa real de la Machine, en vez del `0x8000_0000` del env
  `p` (fuera del mapa de EmuRV).
- `riscv_test.h`: sin HTIF/tohost — `RVTEST_PASS`/`RVTEST_FAIL` y el trap
  handler terminan en `ebreak` (siempre en codificación de 32 bits: el
  rescate de la Machine no reconoce `c.ebreak`), que detiene la Machine
  limpiamente. El veredicto queda en `gp` (x3) con la convención del env
  `p`: `1` = PASS; `(subtest << 1) | 1` = FAIL.

El harness carga cada ELF por `elf_import`, ejecuta por tramos acotados
(tope 20 × 1M instrucciones) y clasifica: PASS / FAIL con subtest / anomalía
(cualquier terminación distinta de `EbreakPause`).

## Alcance y trabajo futuro

- Suites machine-mode (`rv32mi`) excluidas del alcance: ejercen semántica
  M-mode completa que EmuRV delega en políticas didácticas propias
  (BITACORA RP-8).
- Suites RV64 diferidas: `elf_import` es ELF32 por diseño (RP-11) y los
  rv64 tests producen ELF64 — entra como trabajo futuro junto con el resto
  del soporte RV64 extendido.
- Escalón siguiente de compliance: RISCOF/riscv-arch-test contra Sail (RT-3).
