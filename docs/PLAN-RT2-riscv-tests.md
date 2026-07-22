# PLAN-RT2 — riscv-tests oficiales integrados a la suite/CI

Punto RT-2 de `docs/ROADMAP-PRO.md` (fase de cierre de tesis). Plan por fases con
compuertas de verificación: PROHIBIDO pasar a la fase siguiente con la compuerta
en rojo.

**Prerrequisito:** RT-1 COMPLETO (remote GitHub, CI verde en `main`). Cumplido
2026-07-19.

**Destino inmediato:** tabla citable en la memoria — "EmuRV pasa N/N tests
oficiales de riscv-software-src/riscv-tests para RV32IMAFDC" — como evidencia
objetiva de corrección ISA (Cap. V, insumo de RT-8). Es además el escalón
previo a RT-3 (RISCOF/arch-test): si esto no pasa, RISCOF tampoco.

## Alcance

- SÍ: suites de usuario RV32 (`rv32ui`, `rv32um`, `rv32ua`, `rv32uc`, `rv32uf`,
  `rv32ud`), entorno de enlace propio (env EmuRV), binarios ELF vendorizados con
  manifiesto de procedencia, harness Rust como test de integración, job/step de
  CI, documentación + BITACORA.
- NO: suites machine-mode (`rv32mi`, `rv32si` — exigen semántica M/S completa
  que EmuRV delega en políticas didácticas propias; se documenta la exclusión
  con motivo), RISCOF (es RT-3), benchmarks (RT-4), RV64 (opcional, ver F5:
  entra solo si el costo marginal es trivial).

## Diseño acordado (no re-litigar)

### El problema de direcciones
riscv-tests enlaza por defecto en `0x8000_0000`; el mapa de EmuRV es
`TEXT_BASE=0x0040_0000`, `DATA_BASE=0x1001_0000`, stack top `0x8000_0000`
(`src-tauri/src/core/mod.rs:54-73`). Solución estándar del ecosistema: **env
propio** — riscv-tests está diseñado para esto (`riscv-tests/env` es
reemplazable). Se crea `tests-riscv/env-emurv/` con:

- `link.ld`: `.text` en `0x0040_0000`, `.data`/`.bss` en `0x1001_0000`.
- `riscv_test.h`: macros adaptadas al contrato de EmuRV:
  - `RVTEST_PASS` → `li gp, 1; ebreak`
  - `RVTEST_FAIL` → `gp = (testnum << 1) | 1; ebreak`
  - Sin HTIF/tohost: `ebreak` ya detiene la Machine limpiamente (RP-1) y `gp`
    (x3) queda legible desde el harness. Mecanismo más simple que emular HTIF
    y equivalente en poder de diagnóstico (gp codifica el nº de subtest fallado).

### Por qué las suites de usuario funcionan sin tocar el core
- Unicorn (QEMU) implementa la ISA y los CSR reales; los tests `u*` no
  dependen de la política de syscalls de EmuRV (no usan `ecall` — `scall`
  vive en `rv32mi`, excluida).
- Los tests que instalan trap handler propio (`csrw mtvec, ...`) están
  cubiertos por la política RP-8: mtvec≠0 ⇒ delegación al guest.

### Binarios vendorizados, no toolchain en CI
- `scripts/build-riscv-tests.sh`: clona riscv-tests (commit pineado), compila
  con `riscv64-elf-gcc` (brew) + env-emurv, deposita ELFs en
  `src-tauri/tests/fixtures/riscv-tests/<suite>/`, genera `MANIFEST.txt`
  (commit de riscv-tests, versión de gcc, sha256 por archivo, fecha).
- CI NO compila los tests: consume los ELFs commiteados. Motivo: determinismo,
  cero dependencia de toolchain en runners, tiempo de CI. El script queda como
  procedimiento reproducible documentado (material de tesis).
- Peso estimado: ~80 ELFs × ~10-50 KB ≈ pocos MB. No afecta RNF-04 (los
  fixtures no entran al bundle).

### Harness
`src-tauri/tests/rt2_riscv_tests.rs` (test de integración, patrón de los
`spike_*.rs` / `rp11_elf_import.rs`):
1. Enumera los ELFs de `fixtures/riscv-tests/` por suite.
2. Cada ELF entra por **`elf_import`** (el camino real del producto). Si
   `elf_import` rechaza algo legítimo de los tests (p.ej. sección en zona no
   aceptada), primero se ajusta `link.ld`; solo si es irresoluble por enlace se
   documenta en BITACORA y ese test carga por vía directa de Machine
   (decisión registrada, no silenciosa).
3. Ejecuta con presupuesto acotado (count de instrucciones tope, ~1M) hasta
   detención por `ebreak`.
4. Lee `gp`: `1` = PASS; impar ≠1 = FAIL con nº de subtest; tope agotado =
   TIMEOUT (falla).
5. Un `#[test]` por suite (rv32ui, rv32um, ...) que reporta la lista de
   fallos con nombre de test y gp — no un mega-test opaco.

## Fases

### F1 — env-emurv + script de build + primer lote (rv32ui)
- Crear `tests-riscv/env-emurv/` (`link.ld`, `riscv_test.h`).
- `scripts/build-riscv-tests.sh` (commit pineado de riscv-tests, brew
  `riscv64-elf-gcc`; `-march=rv32i...` por suite, `-mabi=ilp32`/`ilp32d`).
- Compilar y vendorizar `rv32ui` completo + MANIFEST.
- **Compuerta:** los ELFs existen, `file` los reconoce como ELF32 RISC-V,
  entry dentro de `[0x0040_0000, ...)`, MANIFEST completo.

### F2 — harness Rust contra rv32ui
- `rt2_riscv_tests.rs` según diseño (enumeración, elf_import, run, gp).
- **Compuerta:** `cargo test --test rt2_riscv_tests` verde con rv32ui
  completo (o exclusiones puntuales documentadas en BITACORA con causa raíz
  analizada — un fallo de Unicorn upstream se registra, no se tapa).

### F3 — resto de suites RV32
- Vendorizar y activar `rv32um`, `rv32ua`, `rv32uc`, `rv32uf`, `rv32ud`
  (F/D con `-mabi=ilp32d` y `mstatus.FS` inicializado — verificar contra la
  init de RP-12).
- **Compuerta:** las 6 suites verdes (mismas reglas de exclusión documentada).

### F4 — CI + tabla de resultados
- Step en el job `test-macos` de `.github/workflows/ci.yml` (los tests de
  integración ya corren con `cargo test`; verificar que el tiempo agregado sea
  aceptable, si no, `--test rt2_riscv_tests` como step separado con timing).
- `docs/RESULTADOS-RISCV-TESTS.md`: tabla suite × pasados/total, commit de
  riscv-tests, exclusiones con motivo — redactada como material citable del
  Cap. V.
- **Compuerta:** CI verde en GitHub con los tests incluidos; tabla completa.

### F5 — (opcional, decisión en el momento) rv64u*
- Solo si F1-F4 dejaron la maquinaria lista y el costo es re-ejecutar el
  script con otra `-march`: vendorizar `rv64ui/um/ua/uc/uf/ud` contra la
  Machine RV64 (RP-32). Si aparece CUALQUIER fricción de mapa de memoria o
  harness, se difiere a trabajo futuro y se anota en ROADMAP-PRO.
- **Compuerta:** verde o diferido explícito documentado.

### F6 — cierre
- BITACORA: decisiones (env propio, gp+ebreak vs HTIF, vendorizado, exclusiones).
- ROADMAP-PRO: RT-2 → COMPLETO con fecha.
- Commits `rt2-fase-M: descripción` (sin trailer Co-Authored-By).
- **Compuerta:** `cargo clippy --all-targets -- -D warnings` limpio, CI verde,
  docs sincronizadas.

## Riesgos y plan B

| Riesgo | Plan B |
|---|---|
| brew no tiene `riscv64-elf-gcc` funcional para rv32 | `riscv-gnu-toolchain` vía otro tap, o toolchain xPack precompilada; se documenta la elegida en MANIFEST |
| `elf_import` rechaza secciones legítimas de los tests | Ajustar `link.ld` primero; si no alcanza, carga directa vía Machine documentada en BITACORA |
| Tests F/D fallan por `mstatus.FS` sin inicializar | Macro `RVTEST_CODE_BEGIN` del env-emurv lo inicializa (mismo patrón que RP-12) |
| Fallo real de Unicorn upstream en algún test | Se registra como hallazgo (¡material de tesis!), test excluido con issue referenciada, cuenta como "N-1/N con causa documentada" |
| Timeout en CI por ~80 tests × 2 builds | count tope bajo (los tests son cortos); si aun así pesa, step separado con cache |
