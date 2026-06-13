//! Desensamblador RV32IM+Zicsr (RP11) — Capstone encapsulado.
//!
//! Decisión del spike F1 (BITACORA, medido): capstone 0.14 con
//! default-features=false y features ["std","full","arch_riscv"] suma
//! ~100 KB al bundle (vs ~6,9 MB con todas las arquitecturas; el modo
//! diet pesa nada pero elimina los mnemónicos — inservible). Esta
//! interfaz es la ÚNICA puerta al desensamblador: si la decisión
//! cambiara, se reemplaza solo este módulo.

use capstone::arch::riscv::{ArchExtraMode, ArchMode};
use capstone::arch::{BuildsCapstone, BuildsCapstoneExtraMode};
use capstone::Capstone;
use serde::Serialize;

/// Una línea del listado: dirección, palabra cruda y texto decodificado.
#[derive(Debug, Clone, Serialize)]
pub struct DisasmLine {
    pub addr: u32,
    pub word: u32,
    pub texto: String,
}

/// Desensambla palabras de 32 bits desde `base`. Una palabra que no
/// decodifica (datos en .text, extensión fuera de RV32IM+Zicsr) se
/// muestra como `.word 0x...` — sin pánico: el emulador decidirá en
/// ejecución (plan RP11, alcance).
pub fn disasm(words: &[u32], base: u32) -> Vec<DisasmLine> {
    let cs = Capstone::new().riscv().mode(ArchMode::RiscV32).build();
    words
        .iter()
        .enumerate()
        .map(|(i, &word)| {
            let addr = base.wrapping_add((i * 4) as u32);
            let texto = cs
                .as_ref()
                .ok()
                .and_then(|cs| {
                    let insns = cs.disasm_count(&word.to_le_bytes(), u64::from(addr), 1).ok()?;
                    let insn = insns.iter().next()?;
                    let mn = insn.mnemonic()?;
                    Some(match insn.op_str() {
                        Some(ops) if !ops.is_empty() => format!("{mn} {ops}"),
                        _ => mn.to_string(),
                    })
                })
                .unwrap_or_else(|| format!(".word 0x{word:08X}"));
            DisasmLine { addr, word, texto }
        })
        .collect()
}

/// Las palabras de un `.text` en bytes (little-endian; el resto de un
/// múltiplo de 4 se ignora — el ensamblador siempre emite words).
pub fn words_de(text: &[u8]) -> Vec<u32> {
    text.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Desensambla `.text` recorriéndolo por LONGITUD VARIABLE (RP13): una
/// instrucción comprimida (C) ocupa 2 bytes — se reconoce porque sus dos
/// bits bajos NO son `11`; el resto ocupa 4. Cada línea trae su dirección
/// real, así el modo binario y los breakpoints sobre el listado quedan
/// exactos con C de por medio. Para `.text` de solo-4-bytes da el mismo
/// resultado que `disasm(words_de(...))`.
pub fn disasm_bytes(text: &[u8], base: u32) -> Vec<DisasmLine> {
    // RiscVC (extra_mode) habilita la decodificación de instrucciones
    // comprimidas; sin él, capstone devuelve una C como dato (RP13-F3).
    let cs = Capstone::new()
        .riscv()
        .mode(ArchMode::RiscV32)
        .extra_mode([ArchExtraMode::RiscVC].iter().copied())
        .build();
    let mut out = Vec::new();
    let mut off = 0usize;
    while off < text.len() {
        let comprimida = text[off] & 0b11 != 0b11;
        let len = if comprimida { 2 } else { 4 };
        if off + len > text.len() {
            break; // resto incompleto: no inventar una instrucción
        }
        let addr = base.wrapping_add(off as u32);
        let bytes = &text[off..off + len];
        let word = if comprimida {
            u32::from(u16::from_le_bytes([bytes[0], bytes[1]]))
        } else {
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        };
        let texto = cs
            .as_ref()
            .ok()
            .and_then(|cs| {
                let insns = cs.disasm_count(bytes, u64::from(addr), 1).ok()?;
                let insn = insns.iter().next()?;
                let mn = insn.mnemonic()?;
                Some(match insn.op_str() {
                    Some(ops) if !ops.is_empty() => format!("{mn} {ops}"),
                    _ => mn.to_string(),
                })
            })
            .unwrap_or_else(|| {
                if comprimida {
                    format!(".half 0x{word:04X}")
                } else {
                    format!(".word 0x{word:08X}")
                }
            });
        out.push(DisasmLine { addr, word, texto });
        off += len;
    }
    out
}

/// Listado completo formateado (UI modo binario y `emurv-cli disasm`):
/// `dirección  palabra  instrucción`, una línea por palabra.
pub fn listado(lines: &[DisasmLine]) -> String {
    let mut s = String::with_capacity(lines.len() * 40);
    for l in lines {
        s.push_str(&format!("{:08x}:  {:08x}  {}\n", l.addr, l.word, l.texto));
    }
    s
}
