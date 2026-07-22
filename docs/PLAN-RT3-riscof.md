# PLAN-RT3 — Compliance formal: riscv-arch-test vía RISCOF contra Sail

Punto RT-3 de `docs/ROADMAP-PRO.md` (fase de cierre de tesis). Plan por fases
con compuertas: PROHIBIDO pasar a la siguiente con la compuerta en rojo.

**Prerrequisito:** RT-2 COMPLETO (2026-07-22, 78/78 riscv-tests). Cumplido.

**Destino inmediato:** el sello formal de compliance del ecosistema RISC-V —
RISCOF ejecuta cada test de riscv-arch-test en EmuRV (DUT) y en el modelo
dorado Sail (referencia oficial de RISC-V International), y compara las
*signatures* (volcado de memoria que cada test escribe). Reporte citable en
el Cap. V, un escalón por encima de RT-2: acá el oráculo es el modelo formal
de la especificación, no tests self-checking.

## Alcance

- SÍ: RISCOF + riscv-arch-test para **RV32 user-level: I, M, C** (y Zicsr si
  la suite del commit pineado lo separa); plugin DUT propio de EmuRV; runner
  headless en Rust (`riscof_runner`); Sail prebuilt como referencia; reporte
  y tabla citable; docs + BITACORA.
- NO: suite `privilege` completa (ejercita semántica M-mode que EmuRV
  delega en políticas didácticas — se evalúa en F4 y si choca se excluye con
  motivo, como rv32mi en RT-2); F/D (evaluación al final, solo si el costo
  marginal es trivial); ejecución de RISCOF en la CI de GitHub (toolchain
  Python+Sail pesada para el runner macOS; los resultados se documentan y el
  procedimiento queda reproducible por script — decisión análoga al
  vendorizado de RT-2).

## Diseño acordado (no re-litigar)

### Piezas y versiones pineadas
- **RISCOF** (pip, versión pineada en el script de setup) en un venv local
  `tests-riscv/.venv/` (gitignorado). Riesgo Python 3.14 → plan B: brew
  `python@3.12` para el venv.
- **Sail**: binario oficial prebuilt `sail-riscv-Mac-arm64` (release
  pineada, hoy 0.13) descargado a `tests-riscv/.tools/` (gitignorado) por
  script con sha256 verificado.
- **riscv-arch-test**: clon con commit pineado en `tests-riscv/.upstream/`
  (mismo patrón que riscv-tests en RT-2).
- **Toolchain**: riscv64-elf-gcc 16.1.0 (brew), ya presente.
- Todo orquestado por `scripts/run-riscof.sh` (setup idempotente + corrida),
  el procedimiento reproducible documentado.

### Runner DUT: `src-tauri/src/bin/riscof_runner.rs`
RISCOF exige del DUT: tomar el ELF del test, ejecutarlo y volcar la
*signature* (memoria entre los símbolos `rvtest_sig_begin`/`rvtest_sig_end`
—alias begin/end_signature—, 4 bytes por línea en hex). Binario nuevo sobre
`emurv_lib` (cargo lo descubre en `src/bin/`):

1. Parsea el ELF con el crate `object` (ya dependencia) para las direcciones
   de los símbolos de signature.
2. Carga por `elf_import` (mismo camino del producto, como RT-2) y corre la
   Machine por tramos hasta `EbreakPause` (RVMODEL_HALT = ebreak de 32 bits)
   con presupuesto acotado.
3. `read_mem` del rango de signature → archivo de salida.

Contrato didáctico intacto: cero cambios en `core/`; si algo del core no
alcanza, se documenta en BITACORA antes de tocar nada.

### Plugin RISCOF de EmuRV: `tests-riscv/riscof-plugin-emurv/`
- `riscof_emurv.py` (clase pluginTemplate: compile con gcc + env propio,
  ejecutar `riscof_runner`, dejar la signature donde RISCOF espera).
- `env/model_test.h`: macros RVMODEL_* de EmuRV — RVMODEL_HALT = `ebreak`
  32 bits (lección RT-2), RVMODEL_DATA_BEGIN/END delimitan la signature,
  IO/interrupt macros vacíos (EmuRV no los necesita para user-level).
- `env/link.ld`: mapa de EmuRV (TEXT_BASE / DATA_BASE), heredado de RT-2.
- `emurv_isa.yaml` + `emurv_platform.yaml`: RV32IMC_Zicsr, misa, etc.
  (riscv-config los valida).
- La referencia Sail corre con su propio env/linker (0x8000_0000): las
  signatures son independientes de las direcciones por diseño de la suite.

## Fases

### F1 — Infraestructura: venv + Sail + arch-test pineados
- `scripts/run-riscof.sh` con subcomando `setup`: venv con riscof pineado,
  descarga Sail Mac-arm64 (sha256), clon de riscv-arch-test pineado.
- **Compuerta:** `riscof --version` OK; `riscv_sim_RV32 --help` (o
  equivalente del binario 0.13) OK; arch-test en el commit pineado.

### F2 — riscof_runner (Rust)
- Binario según diseño; test unitario mínimo con un ELF de RT-2 adaptado o
  fixture propia (signature de contenido conocido).
- **Compuerta:** `cargo test` + `cargo clippy --all-targets -- -D warnings`
  verdes; runner vuelca una signature correcta de una fixture conocida.

### F3 — Plugin + primer suite (rv32i_m/I)
- Plugin completo + YAMLs; `riscof run` con la suite I contra Sail.
- **Compuerta:** reporte de RISCOF con la suite I — objetivo 100% match;
  cualquier mismatch se analiza a causa raíz (¿EmuRV? ¿env? ¿Unicorn
  upstream como fmin en RT-2?) antes de seguir.

### F4 — Suites M y C (+ evaluación Zicsr/privilege/F)
- Igual que F3 para M y C. Zicsr/privilege/F: correr una vez y DECIDIR —
  entra si pasa o si el fallo es analizable y documentable; se excluye con
  motivo si ejercita política didáctica de EmuRV (ecall/M-mode).
- **Compuerta:** I+M+C con veredicto claro por test; exclusiones (si las
  hay) con causa raíz en BITACORA.

### F5 — Resultados y cierre
- `docs/RESULTADOS-RISCOF.md`: tabla suite × tests × match/mismatch/excl.,
  versiones pineadas (RISCOF, Sail, arch-test, gcc), procedimiento de
  reproducción; el reporte HTML de RISCOF archivado (`docs/evidencia/` o
  similar, decidir peso).
- BITACORA (decisiones + hallazgos), ROADMAP-PRO RT-3 → COMPLETO.
- Commits `rt3-fase-M: ...` (sin trailer co-autor). CI verde (el runner
  nuevo compila en CI aunque RISCOF no corra ahí).
- **Compuerta:** docs sincronizadas, CI verde en GitHub.

## Riesgos y plan B

| Riesgo | Plan B |
|---|---|
| riscof no instala en Python 3.14 (paquete 2021-2023) | venv con brew `python@3.12`; última opción: contenedor Docker solo para riscof |
| Binario Sail 0.13 con CLI distinta a la que espera el plugin sail de riscof | Plugin de referencia propio (es ~el mismo esqueleto que el DUT) o ajustar flags en el YAML del plugin |
| arch-test I usa traps/CSRs que chocan con la política de syscalls (ecall) | Mismo análisis que RT-2: si el test mete `ecall` conocida, se documenta y excluye; el resto de la suite sigue |
| elf_import rechaza secciones del layout arch-test | Ajustar link.ld del plugin primero; carga directa documentada como última opción |
| Mismatch real DUT vs Sail | Análisis a causa raíz con la signature diff + trace; si es Unicorn upstream se registra como hallazgo (patrón fmin de RT-2) |
| Gatekeeper macOS bloquea el binario Sail descargado | `xattr -d com.apple.quarantine` en el script de setup, documentado |
