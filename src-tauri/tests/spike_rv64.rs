//! RP32 Fase 1 — spike: ¿Unicorn 2.1.5 soporta `Mode::RISCV64` con el mismo
//! binding que ya usamos para RV32? (patrón spike_csr/spike_mmio: PoC con
//! Unicorn CRUDO y evidencia ANTES de comprometer la arquitectura del cambio
//! u32→u64 + xlen conmutable). La tabla de resultados va a BITACORA RP32-F1.
//!
//! Lo que mide, una pregunta por test:
//!   P1  Mode::RISCV64 ejecuta una instrucción RV64-only (addiw) y un valor
//!       que NO cabe en 32 bits (slli shamt=40 → 2^40).
//!   P2  reg_read/reg_write de 64 bits hacen round-trip en RISCV64 con el
//!       MISMO binding (xN = id N+1) que RV32.
//!   P3  ld/sd mueven 8 bytes (un valor de 64 bits) por memoria.
//!   P4  Mode::RISCV32 queda IDÉNTICO: el mismo binario RV32I da el mismo
//!       resultado, y una instrucción RV64-only es ILEGAL en RV32.
//!   P5  (documental) ancho del registro host en cada modo: ¿RISCV32 trunca?
//!
//! Encodings ARMADOS A MANO contra la espec base (RV32I/RV64I, capítulos 2 y
//! 5; opcodes OP-IMM 0x13, OP-IMM-32 0x1B, LOAD 0x03, STORE 0x23). La
//! verificación palabra-a-palabra contra `riscv64-elf-as -march=rv64im` es de
//! la Fase 3 — acá basta el comportamiento observado.

use std::sync::mpsc::channel;
use std::time::Duration;
use unicorn_engine::{Arch, Mode, Prot, Unicorn};

const BASE: u64 = 0x0040_0000;
/// Zona de datos dentro de la misma página RWX (para ld/sd del P3).
const DATA: u64 = BASE + 0x800;

// Números de registro ARQUITECTÓNICOS (los que van en el encoding).
const T0: u32 = 5;
const T1: u32 = 6;
const T2: u32 = 7;
/// id del binding para reg_read/reg_write: xN == N+1 (X0 == 1). MISMO mapeo
/// que `machine.rs::xreg` — el spike confirma que vale igual en RISCV64.
fn rid(n: u32) -> i32 {
    n as i32 + 1
}

fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    ((imm as u32 & 0xFFF) << 20) | (rs1 << 15) | (rd << 7) | 0x13
}
/// addiw (RV64 OP-IMM-32, funct3=0): resultado de 32 bits SIGN-EXTENDIDO a 64.
fn addiw(rd: u32, rs1: u32, imm: i32) -> u32 {
    ((imm as u32 & 0xFFF) << 20) | (rs1 << 15) | (rd << 7) | 0x1B
}
/// slli con shamt de 6 bits (RV64, funct6=000000): shamt 0..=63. En RV32 un
/// shamt>31 (bit 25 en 1) es codificación RESERVADA → instrucción ilegal.
fn slli(rd: u32, rs1: u32, shamt: u32) -> u32 {
    (shamt << 20) | (rs1 << 15) | (1 << 12) | (rd << 7) | 0x13
}
fn add(rd: u32, rs1: u32, rs2: u32) -> u32 {
    (rs2 << 20) | (rs1 << 15) | (rd << 7) | 0x33
}
/// ld rd, off(rs1) — LOAD funct3=3 (doubleword), solo RV64.
fn ld(rd: u32, off: i32, rs1: u32) -> u32 {
    ((off as u32 & 0xFFF) << 20) | (rs1 << 15) | (3 << 12) | (rd << 7) | 0x03
}
/// sd rs2, off(rs1) — STORE funct3=3 (doubleword), solo RV64.
fn sd(rs2: u32, off: i32, rs1: u32) -> u32 {
    let off = off as u32;
    ((off >> 5) & 0x7F) << 25 | (rs2 << 20) | (rs1 << 15) | (3 << 12) | ((off & 0x1F) << 7) | 0x23
}
const NOP: u32 = 0x0000_0013;

/// Motor con una página RWX en BASE y el programa cargado. Sin hooks: el
/// spike mide el comportamiento CRUDO de Unicorn en el `mode` pedido.
fn motor(mode: Mode, words: &[u32]) -> Unicorn<'static, ()> {
    let mut uc = Unicorn::new(Arch::RISCV, mode).expect("unicorn");
    uc.mem_map(BASE, 0x1000, Prot::READ | Prot::WRITE | Prot::EXEC)
        .expect("map");
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    uc.mem_write(BASE, &bytes).expect("write");
    uc
}

/// Corre `f` en un hilo con tope de tiempo (si Unicorn se colgara, el test
/// falla con mensaje en vez de bloquear la suite). Igual que spike_csr.
fn correr<T, F>(nombre: &'static str, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(10))
        .unwrap_or_else(|_| panic!("{nombre}: colgado"))
}

/// P1: Mode::RISCV64 ejecuta una instrucción RV64-only y produce un valor > 2^32.
#[test]
fn p1_rv64_ejecuta_y_supera_32_bits() {
    let (res, t0, res2, t1) = correr("p1", || {
        // t0 = 1 ; t0 <<= 40  → 2^40 (no cabe en 32 bits, slli shamt>31 es RV64)
        let prog = [addi(T0, 0, 1), slli(T0, T0, 40), NOP];
        let mut uc = motor(Mode::RISCV64, &prog);
        let res = uc.emu_start(BASE, BASE + 12, 0, 10);
        let t0 = uc.reg_read(rid(T0)).unwrap_or(u64::MAX);
        // addiw t1, x0, -1 → 0xFFFF_FFFF_FFFF_FFFF (sign-extiende 32→64)
        let prog2 = [addiw(T1, 0, -1), NOP];
        let mut uc2 = motor(Mode::RISCV64, &prog2);
        let res2 = uc2.emu_start(BASE, BASE + 8, 0, 5);
        let t1 = uc2.reg_read(rid(T1)).unwrap_or(0);
        (res, t0, res2, t1)
    });
    eprintln!("[P1] slli40: res={res:?} t0={t0:#x} | addiw: res={res2:?} t1={t1:#x}");
    assert!(res.is_ok(), "RISCV64 debe ejecutar slli shamt=40: {res:?}");
    assert_eq!(t0, 1u64 << 40, "el registro retuvo un valor > 2^32");
    assert!(res2.is_ok(), "RISCV64 debe ejecutar addiw: {res2:?}");
    assert_eq!(t1, u64::MAX, "addiw -1 sign-extiende a los 64 bits");
}

/// P2: el binding xN = id N+1 hace round-trip de 64 bits en RISCV64.
#[test]
fn p2_reg_64bits_round_trip() {
    let leido = correr("p2", || {
        let mut uc = motor(Mode::RISCV64, &[NOP]);
        let valor = 0x0123_4567_89AB_CDEFu64;
        uc.reg_write(rid(T0), valor).expect("write");
        uc.reg_read(rid(T0)).unwrap_or(0)
    });
    eprintln!("[P2] reg64 round-trip leido={leido:#x}");
    assert_eq!(leido, 0x0123_4567_89AB_CDEF, "el registro guarda los 64 bits");
}

/// P3: sd/ld mueven 8 bytes (un valor de 64 bits) por memoria.
#[test]
fn p3_ld_sd_doubleword() {
    let (res, t2, mem) = correr("p3", || {
        // sd t0, 0(t1) ; ld t2, 0(t1)
        let prog = [sd(T0, 0, T1), ld(T2, 0, T1), NOP];
        let mut uc = motor(Mode::RISCV64, &prog);
        let valor = 0xCAFE_BABE_DEAD_BEEFu64;
        uc.reg_write(rid(T1), DATA).expect("base");
        uc.reg_write(rid(T0), valor).expect("dato");
        let res = uc.emu_start(BASE, BASE + 12, 0, 10);
        let t2 = uc.reg_read(rid(T2)).unwrap_or(0);
        let mut buf = [0u8; 8];
        uc.mem_read(DATA, &mut buf).expect("leer mem");
        (res, t2, u64::from_le_bytes(buf))
    });
    eprintln!("[P3] res={res:?} t2={t2:#x} mem={mem:#x}");
    assert!(res.is_ok(), "sd/ld deben ejecutar: {res:?}");
    assert_eq!(t2, 0xCAFE_BABE_DEAD_BEEF, "ld recuperó los 64 bits");
    assert_eq!(mem, 0xCAFE_BABE_DEAD_BEEF, "sd escribió los 8 bytes");
}

/// P4: RISCV32 queda IDÉNTICO. Mismo binario RV32I → mismo resultado en
/// ambos modos; una instrucción RV64-only (addiw) es ILEGAL en RV32.
#[test]
fn p4_rv32_intacto() {
    let (t2_32, t2_64, res_addiw_32, res_slli_32) = correr("p4", || {
        // li t0,5 ; li t1,37 ; add t2,t0,t1  → 42 (RV32I puro)
        let prog = [addi(T0, 0, 5), addi(T1, 0, 37), add(T2, T0, T1), NOP];
        let mut uc32 = motor(Mode::RISCV32, &prog);
        let _ = uc32.emu_start(BASE, BASE + 16, 0, 10);
        let t2_32 = uc32.reg_read(rid(T2)).unwrap_or(0);
        let mut uc64 = motor(Mode::RISCV64, &prog);
        let _ = uc64.emu_start(BASE, BASE + 16, 0, 10);
        let t2_64 = uc64.reg_read(rid(T2)).unwrap_or(0);
        // addiw en RV32: opcode 0x1B reservado → instrucción ilegal.
        let mut uc_a = motor(Mode::RISCV32, &[addiw(T0, 0, -1), NOP]);
        let res_addiw_32 = uc_a.emu_start(BASE, BASE + 8, 0, 5);
        // slli shamt=40 en RV32: codificación reservada → ilegal.
        let mut uc_s = motor(Mode::RISCV32, &[addi(T0, 0, 1), slli(T0, T0, 40), NOP]);
        let res_slli_32 = uc_s.emu_start(BASE, BASE + 12, 0, 10);
        (t2_32, t2_64, res_addiw_32, res_slli_32)
    });
    eprintln!(
        "[P4] add 5+37: rv32 t2={t2_32} rv64 t2={t2_64} | addiw@rv32={res_addiw_32:?} \
         slli40@rv32={res_slli_32:?}"
    );
    assert_eq!(t2_32, 42, "RV32I da el resultado de siempre en RISCV32");
    assert_eq!(t2_64, 42, "el MISMO binario RV32I da lo mismo en RISCV64");
    assert!(res_addiw_32.is_err(), "addiw es ilegal en RISCV32");
    assert!(res_slli_32.is_err(), "slli shamt=40 es ilegal en RISCV32");
}

/// P5 (documental): ¿el registro host trunca a 32 bits en RISCV32? Define si
/// la conmutación necesita enmascarar a mano o si Unicorn lo hace. Sin assert
/// duro: el dato observado se vuelca a BITACORA.
#[test]
fn p5_ancho_registro_por_modo() {
    let (rv32, rv64) = correr("p5", || {
        let alto = 0x1_0000_0001u64;
        let mut uc32 = motor(Mode::RISCV32, &[NOP]);
        uc32.reg_write(rid(T0), alto).expect("w32");
        let rv32 = uc32.reg_read(rid(T0)).unwrap_or(0);
        let mut uc64 = motor(Mode::RISCV64, &[NOP]);
        uc64.reg_write(rid(T0), alto).expect("w64");
        let rv64 = uc64.reg_read(rid(T0)).unwrap_or(0);
        (rv32, rv64)
    });
    eprintln!("[P5] escrito=0x100000001 → leido rv32={rv32:#x} rv64={rv64:#x}");
    assert_eq!(rv64, 0x1_0000_0001, "RISCV64 conserva los 64 bits escritos");
}
