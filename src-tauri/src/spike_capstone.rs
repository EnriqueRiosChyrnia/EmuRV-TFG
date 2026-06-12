//! SPIKE RP11-F1 (rama descartable): fuerza el enlace real de capstone en
//! el binario de la app para medir el peso VERDADERO del bundle.
use capstone::prelude::*;

pub fn spike_disasm_len(bytes: &[u8]) -> usize {
    let Ok(cs) = Capstone::new()
        .riscv()
        .mode(arch::riscv::ArchMode::RiscV32)
        .build()
    else {
        return 0;
    };
    cs.disasm_all(bytes, 0x0040_0000).map(|i| i.len()).unwrap_or(0)
}
