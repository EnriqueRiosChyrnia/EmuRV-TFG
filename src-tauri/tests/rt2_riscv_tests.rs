//! RT-2 — riscv-tests oficiales (riscv-software-src/riscv-tests) contra la
//! Machine real de EmuRV.
//!
//! Fixtures: ELFs vendorizados en tests/fixtures/riscv-tests/<suite>/,
//! compilados por scripts/build-riscv-tests.sh con el env propio
//! tests-riscv/env-emurv (procedencia en fixtures/riscv-tests/MANIFEST.txt).
//!
//! Contrato del env EmuRV (ver tests-riscv/env-emurv/riscv_test.h):
//! el test termina SIEMPRE en `ebreak` (StopReason::EbreakPause) con el
//! veredicto en gp (x3): 1 = PASS; impar ≠ 1 = FAIL, donde
//! gp = (nº de subtest << 1) | 1 — la convención del env `p` oficial.
//! Cada ELF entra por elf_import, el mismo camino que un ELF del usuario.

use emurv_lib::core::elf_import::import_elf;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use std::path::Path;
use std::sync::mpsc::channel;

/// Presupuesto por tramo y tope de tramos: los tests son cortos (≤ decenas
/// de miles de instrucciones); 20 × 1M es holgado y acota un test colgado.
const INSTR_POR_TRAMO: u64 = 1_000_000;
const MAX_TRAMOS: usize = 20;

/// Resultado de un ELF de la suite.
enum Veredicto {
    Pass,
    /// gp impar ≠ 1: número de subtest que falló según el test.
    Fail { subtest: u64 },
    /// La Machine no terminó en EbreakPause (error de runtime, timeout...).
    Anomalia { detalle: String },
}

fn correr_elf(bytes: &[u8]) -> Result<Veredicto, String> {
    let program = import_elf(bytes)?;
    let (out_tx, _out_rx) = channel::<ConsoleEvent>();
    let (_in_tx, in_rx) = channel::<InputMsg>();
    let mut m = Machine::new(program, out_tx, in_rx)?;

    let mut r = m.run_slice(0, INSTR_POR_TRAMO);
    for _ in 0..MAX_TRAMOS {
        if !matches!(r, StopReason::Paused) {
            break;
        }
        r = m.run_slice_continuation(0, INSTR_POR_TRAMO);
    }

    match r {
        StopReason::EbreakPause { .. } => {
            let gp = m.registers()?[3];
            match gp {
                1 => Ok(Veredicto::Pass),
                g if g % 2 == 1 => Ok(Veredicto::Fail { subtest: g >> 1 }),
                g => Ok(Veredicto::Anomalia {
                    detalle: format!("ebreak con gp par inesperado: {g}"),
                }),
            }
        }
        StopReason::Paused => Ok(Veredicto::Anomalia {
            detalle: format!("no terminó tras {MAX_TRAMOS} tramos de {INSTR_POR_TRAMO} instrucciones"),
        }),
        otro => Ok(Veredicto::Anomalia {
            detalle: format!("terminó con {otro:?} en vez de EbreakPause"),
        }),
    }
}

/// Corre la suite entera y falla con la lista completa de tests rotos
/// (no un panic opaco en el primero). `excluidos` documenta cada exclusión
/// con su motivo — la tabla de RESULTADOS-RISCV-TESTS.md sale de acá.
fn correr_suite(suite: &str, excluidos: &[(&str, &str)]) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/riscv-tests")
        .join(suite);
    assert!(
        dir.is_dir(),
        "faltan fixtures en {dir:?}: correr scripts/build-riscv-tests.sh"
    );

    let mut elfs: Vec<_> = std::fs::read_dir(&dir)
        .expect("leer dir de fixtures")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "elf"))
        .collect();
    elfs.sort();
    assert!(!elfs.is_empty(), "cero ELFs en {dir:?}");

    let mut fallos = Vec::new();
    let mut pasados = 0usize;
    let mut saltados = 0usize;

    for elf in &elfs {
        let nombre = elf.file_stem().unwrap().to_string_lossy().into_owned();
        if let Some((_, motivo)) = excluidos.iter().find(|(n, _)| *n == nombre) {
            eprintln!("{suite}/{nombre}: EXCLUIDO — {motivo}");
            saltados += 1;
            continue;
        }
        let bytes = std::fs::read(elf).expect("leer ELF");
        match correr_elf(&bytes) {
            Ok(Veredicto::Pass) => pasados += 1,
            Ok(Veredicto::Fail { subtest }) => {
                fallos.push(format!("{nombre}: FAIL (subtest {subtest})"));
            }
            Ok(Veredicto::Anomalia { detalle }) => {
                fallos.push(format!("{nombre}: ANOMALÍA — {detalle}"));
            }
            Err(e) => fallos.push(format!("{nombre}: elf_import rechazó el ELF — {e}")),
        }
    }

    eprintln!(
        "{suite}: {pasados} PASS, {} FAIL, {saltados} excluidos, {} total",
        fallos.len(),
        elfs.len()
    );
    assert!(
        fallos.is_empty(),
        "{suite}: {} de {} tests fallaron:\n  {}",
        fallos.len(),
        elfs.len(),
        fallos.join("\n  ")
    );
}

#[test]
fn rv32ui() {
    correr_suite(
        "rv32ui",
        &[(
            "fence_i",
            "código auto-modificado ejecutado desde .data (el test parchea y \
             salta a instrucciones en el segmento de datos); EmuRV solo \
             ejecuta dentro de .text (R+X, datos sin permiso de ejecución) \
             por política didáctica — el pc fuera del código es RuntimeError \
             a propósito",
        )],
    );
}

#[test]
fn rv32um() {
    correr_suite("rv32um", &[]);
}

#[test]
fn rv32ua() {
    correr_suite("rv32ua", &[]);
}

/// Exclusión compartida de rv32uf/rv32ud: defecto de Unicorn 2.1.5
/// verificado con una sonda aislada (fmax.s f2, f1, f0 con f1=1.0 y
/// f0=sNaN devuelve 0x7fc00000 —qNaN canónico— en vez de 1.0; el flag NV
/// sí se levanta). Es la semántica IEEE 754-2008 minNum/maxNum previa a la
/// spec F v2.2: QEMU la corrigió en 6.0 (754-2019 maximumNumber), pero
/// Unicorn 2.x sigue basado en QEMU 5.0.1. Subtest 20 del fmin.S oficial.
const MOTIVO_FMIN: &str =
    "Unicorn 2.1.5 (QEMU 5.0.1) implementa fmin/fmax con semántica IEEE \
     754-2008 (pre spec F v2.2): fmax(1.0, sNaN) devuelve qNaN canónico en \
     vez del operando numérico (subtest 20); corregido en QEMU 6.0, fuera \
     del alcance de EmuRV";

#[test]
fn rv32uc() {
    correr_suite(
        "rv32uc",
        &[(
            "rvc",
            "el subtest 6 escribe con c.sw sobre la etiqueta `data` definida \
             DENTRO de .text (el env `p` oficial mapea todo RWX); EmuRV mapea \
             .text R+X por política didáctica (W^X), misma causa que fence_i \
             de rv32ui",
        )],
    );
}

#[test]
fn rv32uf() {
    correr_suite("rv32uf", &[("fmin", MOTIVO_FMIN)]);
}

#[test]
fn rv32ud() {
    correr_suite("rv32ud", &[("fmin", MOTIVO_FMIN)]);
}
