//! RT-3 — Runner headless para RISCOF (DUT EmuRV).
//!
//! Contrato de RISCOF con el DUT: recibir el ELF de un test de
//! riscv-arch-test, ejecutarlo y volcar la *signature* — la región de
//! memoria entre los símbolos `begin_signature`/`end_signature` (alias
//! `rvtest_sig_begin`/`rvtest_sig_end`) — como texto, una palabra de 4
//! bytes por línea en hex minúscula (granularidad 4, la misma que se le
//! pide a Sail).
//!
//! El ELF entra por `elf_import` y corre en la Machine real, el mismo
//! camino que un programa del usuario (patrón RT-2). El test termina en
//! `ebreak` (RVMODEL_HALT del plugin, siempre en codificación de 32 bits
//! — lección RT-2: el rescate de la Machine no reconoce c.ebreak).
//!
//! Uso: riscof_runner <test.elf> --signature <salida> [--max-instr N]

use emurv_lib::core::elf_import::import_elf;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use object::read::elf::ElfFile32;
use object::{Endianness, Object, ObjectSymbol};
use std::process::ExitCode;
use std::sync::mpsc::channel;

const INSTR_POR_TRAMO: u64 = 1_000_000;
const MAX_INSTR_DEFAULT: u64 = 100_000_000;

fn main() -> ExitCode {
    match correr(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("riscof_runner: {e}");
            ExitCode::FAILURE
        }
    }
}

fn correr(args: Vec<String>) -> Result<(), String> {
    // --- Argumentos (sin clap, como emurv-cli) ---
    let mut elf_path = None;
    let mut sig_path = None;
    let mut max_instr = MAX_INSTR_DEFAULT;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--signature" => {
                sig_path = Some(it.next().ok_or("--signature requiere un archivo")?);
            }
            "--max-instr" => {
                max_instr = it
                    .next()
                    .ok_or("--max-instr requiere un número")?
                    .parse()
                    .map_err(|_| "--max-instr: número inválido".to_string())?;
            }
            _ if elf_path.is_none() => elf_path = Some(a),
            otro => return Err(format!("argumento desconocido: {otro}")),
        }
    }
    let elf_path = elf_path.ok_or("uso: riscof_runner <test.elf> --signature <salida>")?;
    let sig_path = sig_path.ok_or("falta --signature <salida>")?;

    // --- Símbolos de la signature (antes de cargar: el error es más claro) ---
    let bytes =
        std::fs::read(&elf_path).map_err(|e| format!("no se pudo leer {elf_path}: {e}"))?;
    let (sig_ini, sig_fin) = simbolos_signature(&bytes)?;
    if sig_ini > sig_fin || (sig_fin - sig_ini) % 4 != 0 {
        return Err(format!(
            "región de signature inválida: [0x{sig_ini:08X}, 0x{sig_fin:08X})"
        ));
    }

    // --- Carga y ejecución (camino del producto: elf_import + Machine) ---
    let program = import_elf(&bytes)?;
    let (out_tx, _out_rx) = channel::<ConsoleEvent>();
    let (_in_tx, in_rx) = channel::<InputMsg>();
    let mut m = Machine::new(program, out_tx, in_rx)?;

    let tramos = max_instr.div_ceil(INSTR_POR_TRAMO);
    let mut r = m.run_slice(0, INSTR_POR_TRAMO);
    for _ in 1..tramos {
        if !matches!(r, StopReason::Paused) {
            break;
        }
        r = m.run_slice_continuation(0, INSTR_POR_TRAMO);
    }
    match r {
        StopReason::EbreakPause { .. } => {}
        StopReason::Paused => {
            return Err(format!(
                "el test no terminó en {max_instr} instrucciones (¿bucle infinito?)"
            ));
        }
        otro => return Err(format!("el test terminó con {otro:?} en vez de EbreakPause")),
    }

    // --- Volcado de la signature ---
    let cuerpo = m.read_mem(sig_ini, (sig_fin - sig_ini) as usize)?;
    let mut sig = String::with_capacity(cuerpo.len() / 4 * 9);
    for palabra in cuerpo.chunks_exact(4) {
        let v = u32::from_le_bytes([palabra[0], palabra[1], palabra[2], palabra[3]]);
        sig.push_str(&format!("{v:08x}\n"));
    }
    std::fs::write(&sig_path, sig).map_err(|e| format!("no se pudo escribir {sig_path}: {e}"))
}

/// Direcciones de la región de signature. `begin_signature`/`end_signature`
/// (las RVMODEL_DATA_* del env) van PRIMERO: son las que Sail vuelca y las
/// que RISCOF compara — incluyen el padding de alineación final.
/// `rvtest_sig_begin/end` (labels internas de arch_test.h) son el fallback:
/// terminan ANTES de ese padding y desalinean la comparación (verificado en
/// sub-01: 594 vs 596 palabras).
fn simbolos_signature(bytes: &[u8]) -> Result<(u32, u32), String> {
    let file: ElfFile32<Endianness> =
        ElfFile32::parse(bytes).map_err(|e| format!("el ELF no se pudo interpretar: {e}"))?;
    let buscar = |nombre: &str| {
        file.symbols()
            .find(|s| s.name() == Ok(nombre))
            .map(|s| s.address() as u32)
    };
    for (ini, fin) in [
        ("begin_signature", "end_signature"),
        ("rvtest_sig_begin", "rvtest_sig_end"),
    ] {
        if let (Some(a), Some(b)) = (buscar(ini), buscar(fin)) {
            return Ok((a, b));
        }
    }
    Err("el ELF no tiene símbolos de signature (rvtest_sig_begin/end ni \
         begin/end_signature)"
        .to_string())
}
