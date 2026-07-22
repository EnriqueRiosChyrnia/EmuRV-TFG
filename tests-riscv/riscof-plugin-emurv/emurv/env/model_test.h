// RVMODEL_* de EmuRV para riscv-arch-test (RT-3).
//
// Contrato con riscof_runner (src-tauri/src/bin/riscof_runner.rs):
//   - HALT = `ebreak` SIEMPRE en codificación de 32 bits (lección RT-2:
//     el rescate de la Machine no reconoce c.ebreak) → EbreakPause.
//   - La signature queda entre begin_signature/end_signature en .data.
// Sin HTIF/tohost (EmuRV no lo implementa; el veredicto lo da RISCOF
// comparando signatures, no el test).

#ifndef _COMPLIANCE_MODEL_H
#define _COMPLIANCE_MODEL_H

#define RVMODEL_HALT                                                    \
        .option push;                                                   \
        .option norvc;                                                  \
        ebreak;                                                          \
        .option pop;                                                    \
        self_loop: j self_loop;

#define RVMODEL_BOOT

#define RVMODEL_DATA_BEGIN                                              \
        .align 4;                                                        \
        .global begin_signature; begin_signature:

#define RVMODEL_DATA_END                                                \
        .align 4;                                                        \
        .global end_signature; end_signature:

// E/S y interrupciones de plataforma: EmuRV no las necesita para las
// suites user-level (I/M/C) — macros vacías, igual que el template.
#define RVMODEL_IO_INIT
#define RVMODEL_IO_WRITE_STR(_R, _STR)
#define RVMODEL_IO_CHECK()
#define RVMODEL_IO_ASSERT_GPR_EQ(_S, _R, _I)
#define RVMODEL_IO_ASSERT_SFPR_EQ(_F, _R, _I)
#define RVMODEL_IO_ASSERT_DFPR_EQ(_D, _R, _I)
#define RVMODEL_SET_MSW_INT
#define RVMODEL_CLEAR_MSW_INT
#define RVMODEL_CLEAR_MTIMER_INT
#define RVMODEL_CLEAR_MEXT_INT

#endif // _COMPLIANCE_MODEL_H
