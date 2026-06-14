//! RP32 Fase 2 — la máquina es CONMUTABLE RV32/RV64. Como el ensamblador
//! todavía no produce RV64 (eso es la Fase 3), estos tests construyen el
//! `Program` con palabras RV64 a mano (mismos encodings del spike F1,
//! verificados por comportamiento) y comprueban:
//!   - en `Xlen::Rv64` la máquina ejecuta instrucciones de 64 bits y los
//!     registros conservan valores > 2^32 (round-trip de 64 bits);
//!   - `ld`/`sd` mueven 8 bytes por el mapa de memoria educativo (sin tocar
//!     las direcciones, que siguen siendo bajas);
//!   - la conmutación es REAL: el MISMO binario RV64-only que corre en
//!     `Rv64` es ilegal en `Rv32` (la máquina elige el `Mode` por el `xlen`).
//!
//! La Fase 2 NO toca el comportamiento RV32 (esos ~219 tests siguen verdes).

use emurv_lib::core::assembler::{Program, SourceMap};
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::{Xlen, DATA_BASE, TEXT_BASE};
use std::collections::BTreeMap;
use std::sync::mpsc::{channel, Receiver, Sender};

// Encodings RV32I/RV64I armados a mano (idénticos a tests/spike_rv64.rs).
const T0: u32 = 5;
const T1: u32 = 6;
const T2: u32 = 7;
fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    ((imm as u32 & 0xFFF) << 20) | (rs1 << 15) | (rd << 7) | 0x13
}
fn addiw(rd: u32, rs1: u32, imm: i32) -> u32 {
    ((imm as u32 & 0xFFF) << 20) | (rs1 << 15) | (rd << 7) | 0x1B
}
fn slli(rd: u32, rs1: u32, shamt: u32) -> u32 {
    (shamt << 20) | (rs1 << 15) | (1 << 12) | (rd << 7) | 0x13
}
fn lui(rd: u32, imm20: u32) -> u32 {
    (imm20 << 12) | (rd << 7) | 0x37
}
fn ld(rd: u32, off: i32, rs1: u32) -> u32 {
    ((off as u32 & 0xFFF) << 20) | (rs1 << 15) | (3 << 12) | (rd << 7) | 0x03
}
fn sd(rs2: u32, off: i32, rs1: u32) -> u32 {
    let off = off as u32;
    ((off >> 5) & 0x7F) << 25 | (rs2 << 20) | (rs1 << 15) | (3 << 12) | ((off & 0x1F) << 7) | 0x23
}
const NOP: u32 = 0x0000_0013;

/// Programa mínimo a partir de palabras crudas + ancho. El resto de los
/// campos de `Program` son los neutros (sin datos, sin símbolos): suficiente
/// para cargarlo en la máquina y steppear.
fn programa(words: &[u32], xlen: Xlen) -> Program {
    let text: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    Program {
        text,
        data: Vec::new(),
        symbols: BTreeMap::new(),
        source_map: SourceMap::default(),
        origins: Vec::new(),
        files: vec!["test.s".to_string()],
        entry: TEXT_BASE,
        from_binary: false,
        reubicable: false,
        xlen,
    }
}

fn maquina(words: &[u32], xlen: Xlen) -> (Machine<'static>, Receiver<ConsoleEvent>, Sender<InputMsg>) {
    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    let m = Machine::new(programa(words, xlen), out_tx, in_rx).expect("máquina");
    (m, out_rx, in_tx)
}

/// En RV64 la máquina ejecuta instrucciones de 64 bits y un registro retiene
/// un valor > 2^32 (round-trip de 64 bits a través de la API `registers()`).
#[test]
fn rv64_registro_supera_32_bits() {
    // t0 = 1 ; t0 <<= 40 (= 2^40) ; t1 = -1 (addiw sign-extiende a 64)
    let prog = [addi(T0, 0, 1), slli(T0, T0, 40), addiw(T1, 0, -1), NOP];
    let (mut m, _o, _i) = maquina(&prog, Xlen::Rv64);
    assert_eq!(m.step(), StopReason::StepDone);
    assert_eq!(m.step(), StopReason::StepDone);
    assert_eq!(m.step(), StopReason::StepDone);
    let r = m.registers().expect("registros");
    assert_eq!(r[5], 1u64 << 40, "el registro retuvo 2^40 (no cabe en 32 bits)");
    assert_eq!(r[6], u64::MAX, "addiw -1 sign-extendió a los 64 bits");
}

/// `sd`/`ld` mueven 8 bytes por el mapa educativo (.data sigue en 0x1001_0000,
/// una dirección BAJA: el mapa no cambia). El valor tiene parte alta NO trivial
/// (0x100 en los bits 32+): un `sw`/`lw` de 4 bytes la perdería.
#[test]
fn rv64_ld_sd_doubleword() {
    let valor: u64 = (1u64 << 40) | 7; // 0x0000_0100_0000_0007
    // t0 = 2^40 + 7 ; t2 = DATA_BASE ; sd t0,0(t2) ; ld t1,0(t2)
    let prog = [
        addi(T0, 0, 1),
        slli(T0, T0, 40),
        addi(T0, T0, 7),
        lui(T2, DATA_BASE >> 12), // DATA_BASE = 0x1001_0000 → imm20 = 0x10010
        sd(T0, 0, T2),
        ld(T1, 0, T2),
        NOP,
    ];
    let (mut m, _o, _i) = maquina(&prog, Xlen::Rv64);
    for _ in 0..6 {
        assert_eq!(m.step(), StopReason::StepDone);
    }
    let r = m.registers().expect("registros");
    assert_eq!(r[6], valor, "ld recuperó los 8 bytes (parte alta incluida)");
    let mem = m.read_mem(DATA_BASE, 8).expect("leer 8 bytes de .data");
    assert_eq!(mem, valor.to_le_bytes(), "sd escribió los 8 bytes en .data");
}

/// La conmutación es REAL: el MISMO binario RV64-only es ilegal en RV32. La
/// máquina elige `Mode::RISCV32`/`RISCV64` según el `xlen` del programa.
#[test]
fn conmutacion_rv32_vs_rv64_misma_palabra() {
    let prog = [addiw(T0, 0, -1), NOP]; // addiw: opcode 0x1B (RV64-only)
    // En RV64: ejecuta y deja t0 = 0xFFFF_FFFF_FFFF_FFFF.
    let (mut m64, _o, _i) = maquina(&prog, Xlen::Rv64);
    assert_eq!(m64.step(), StopReason::StepDone, "addiw es legal en RV64");
    assert_eq!(m64.registers().expect("regs")[5], u64::MAX);
    // En RV32: la misma palabra es instrucción ilegal → error didáctico.
    let (mut m32, _o, _i) = maquina(&prog, Xlen::Rv32);
    match m32.step() {
        StopReason::RuntimeError { message } => {
            assert!(message.contains("inválida"), "mensaje didáctico: {message}");
        }
        other => panic!("addiw debía ser ilegal en RV32, hubo {other:?}"),
    }
}

/// END-TO-END (F4): el ejemplo `ejemplos/15-rv64.asm` ensambla en RV64 con
/// `assemble_with_xlen`, corre, imprime el resultado de `addiw` y deja un valor
/// > 2^32 en un registro (el panel lo mostraría con 16 dígitos hex).
#[test]
fn ejemplo_15_rv64_end_to_end() {
    use emurv_lib::core::assembler::assemble_with_xlen;
    let src = include_str!("../../ejemplos/15-rv64.asm");
    let program = assemble_with_xlen(src, Xlen::Rv64).expect("el ejemplo ensambla en RV64");
    assert_eq!(program.xlen, Xlen::Rv64);
    let (out_tx, out_rx) = channel();
    let (_in_tx, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina RV64");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    let mut salida = String::new();
    while let Ok(ev) = out_rx.try_recv() {
        if let ConsoleEvent::Output(t) = ev {
            salida.push_str(&t);
        }
    }
    assert!(salida.contains("= 93"), "addiw 100 + (-7) = 93; salida: {salida:?}");
    let r = m.registers().expect("regs");
    assert_eq!(r[5], 0x0000_0100_0000_007B, "t0 = 2^40 + 0x7B (no cabe en 32 bits)");
    assert_eq!(r[7], 0x0000_0100_0000_007B, "t2 = round-trip por sd/ld de 8 bytes");
    assert_eq!(r[28], 93, "t3 = addiw");
}

/// El MISMO ejemplo RV64 NO ensambla en RV32 (usa instrucciones de 64 bits):
/// confirma que el selector importa y que el error es didáctico.
#[test]
fn ejemplo_15_rv64_no_ensambla_en_rv32() {
    use emurv_lib::core::assembler::assemble_with_xlen;
    let src = include_str!("../../ejemplos/15-rv64.asm");
    let err = assemble_with_xlen(src, Xlen::Rv32).expect_err("debe rechazarse en RV32");
    assert!(
        err.iter().any(|e| e.msg.contains("RV64")),
        "se esperaba un error didáctico de arquitectura: {err:?}"
    );
}

/// Regresión explícita de la regla dura: un programa RV32 (xlen por defecto)
/// se ejecuta exactamente como antes a través de la máquina conmutable.
#[test]
fn rv32_default_sigue_intacto() {
    use emurv_lib::core::assembler::assemble;
    let program = assemble("li t0, 7\nli t1, 35\nadd t2, t0, t1\nli a7, 10\necall")
        .expect("ensambla RV32");
    assert_eq!(program.xlen, Xlen::Rv32, "el ensamblador produce RV32 por defecto");
    let (out_tx, _out_rx) = channel();
    let (_in_tx, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina RV32");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    assert_eq!(m.registers().expect("regs")[7], 42, "t2 = 7 + 35");
}
