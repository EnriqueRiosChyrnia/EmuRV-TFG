// Entorno de enlace EmuRV para riscv-software-src/riscv-tests (RT-2).
//
// Reemplaza al env `p` de riscv-test-env con el contrato de EmuRV:
//   - .text en TEXT_BASE (0x0040_0000) y .data en DATA_BASE (0x1001_0000),
//     el mapa real de la Machine (src-tauri/src/core/mod.rs), vía link.ld.
//   - Sin HTIF/tohost: PASS/FAIL terminan en `ebreak`, que detiene la
//     Machine limpiamente (StopReason::EbreakPause, RP-1). El resultado
//     queda en gp (x3): 1 = PASS; impar ≠ 1 = FAIL con el número de subtest
//     codificado como (testnum << 1) | 1 — la misma convención del env `p`.
//   - Trap inesperado: _start instala trap_vector en mtvec; el handler
//     codifica gp como FAIL y ejecuta `ebreak`.
//
// Superficie de macros que consumen los tests (verificada contra el commit
// pineado en scripts/build-riscv-tests.sh): RVTEST_RV32U / RVTEST_RV64U /
// RVTEST_RV32UF / RVTEST_RV64UF, RVTEST_CODE_BEGIN/END,
// RVTEST_DATA_BEGIN/END, RVTEST_PASS/FAIL y TESTNUM.

#ifndef _ENV_EMURV_TEST_H
#define _ENV_EMURV_TEST_H

#define TESTNUM gp

// `ebreak` SIEMPRE en su codificación de 32 bits: el rescate de la Machine
// (BITACORA RP1-F2) compara la palabra en pc contra EBREAK_WORD (0x00100073)
// y NO reconoce c.ebreak (0x9002), que es lo que gas emite bajo -march=...c.
#define EBREAK_32                                                       \
  .option push;                                                         \
  .option norvc;                                                        \
  ebreak;                                                               \
  .option pop

//-----------------------------------------------------------------------
// Selección de arquitectura: cada variante define la macro gas `init`,
// que RVTEST_CODE_BEGIN invoca tras instalar el trap vector (mismo
// patrón que el env `p` oficial).
//-----------------------------------------------------------------------

#define RVTEST_RV32U                                                    \
  .macro init;                                                          \
  .endm

#define RVTEST_RV64U RVTEST_RV32U

#define MSTATUS_FS 0x00006000

#define RVTEST_FP_ENABLE                                                \
  li t0, MSTATUS_FS;                                                    \
  csrs mstatus, t0;                                                     \
  csrwi fcsr, 0

#define RVTEST_RV32UF                                                   \
  .macro init;                                                          \
  RVTEST_FP_ENABLE;                                                     \
  .endm

#define RVTEST_RV64UF RVTEST_RV32UF

//-----------------------------------------------------------------------
// Esqueleto de código y datos
//-----------------------------------------------------------------------

#define RVTEST_CODE_BEGIN                                               \
        .section .text.init;                                            \
        .align  6;                                                      \
        .globl _start;                                                  \
_start:                                                                 \
        la t0, trap_vector;                                             \
        csrw mtvec, t0;                                                 \
        init;                                                           \
        li TESTNUM, 0;                                                  \
        j test_begin;                                                   \
        .align 2;                                                       \
trap_vector:                                                            \
        slli TESTNUM, TESTNUM, 1;                                       \
        ori TESTNUM, TESTNUM, 1;                                        \
        EBREAK_32;                                                      \
        j trap_vector;                                                  \
test_begin:

#define RVTEST_CODE_END                                                 \
        unimp

#define RVTEST_PASS                                                     \
        fence;                                                          \
        li TESTNUM, 1;                                                  \
        EBREAK_32

#define RVTEST_FAIL                                                     \
        fence;                                                          \
        slli TESTNUM, TESTNUM, 1;                                       \
        ori TESTNUM, TESTNUM, 1;                                        \
        EBREAK_32

#define RVTEST_DATA_BEGIN                                               \
        .data;                                                          \
        .align 4;

#define RVTEST_DATA_END                                                 \
        .align 4;

#endif
