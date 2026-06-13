//! RP12 Fase 1 — spike: ¿qué hace Unicorn 2.1.5 con las extensiones F/D?
//! (patrón spike_csr: PoC con Unicorn CRUDO y evidencia antes de escribir
//! una línea del ensamblador).
//!
//! La trampa anunciada por el plan: la espec privilegiada (mstatus.FS,
//! bits 14:13) exige FS != Off (00) para ejecutar instrucciones FP — si
//! el motor arranca con FS=Off, todo fadd.s da ILLEGAL.
//!
//! Encodings a mano contra la espec RV32 F/D (cap. 11-12, opcode OP-FP
//! 0x53, LOAD-FP 0x07, STORE-FP 0x27; rm=111 (dyn) en el campo funct3):
//!   flw  rd, imm(rs1)  = (imm<<20)|(rs1<<15)|(2<<12)|(rd<<7)|0x07
//!   fld  rd, imm(rs1)  = ídem con funct3=3
//!   fadd.s fd,fs1,fs2  = (0x00<<25)|(rs2<<20)|(rs1<<15)|(7<<12)|(rd<<7)|0x53
//!   fadd.d             = ídem con funct7=0x01
//!   fmv.w.x fd, rs1    = (0x78<<25)|(rs1<<15)|(0<<12)|(rd<<7)|0x53
//!   fmv.x.w rd, fs1    = (0x70<<25)|(rs1<<15)|(0<<12)|(rd<<7)|0x53
//! IDs del binding: el dato de la coordinadora (F0=57..88, FCSR=141)
//! resultó ERRADO — extraídos del bindings.rs GENERADO (OUT_DIR):
//! F0=158..F31=189, FCSR=43, MSTATUS=116 (este sí coincidía), PC=190.

use std::sync::mpsc::channel;
use std::time::Duration;
use unicorn_engine::{Arch, Mode, Prot, Unicorn};

const BASE: u64 = 0x0040_0000;
// OJO: 0x800 como imm de addi es NEGATIVO (12 bits con signo) — el
// primer intento del spike cayó en 0x3FF800 no mapeada (READ_UNMAPPED).
const DATOS: u64 = 0x0040_0700;
const MSTATUS: i32 = 116;
const F0: i32 = 158;
const F1: i32 = 159;
const F2: i32 = 160;
const FCSR: i32 = 43;
const T0X: u32 = 5; // x5 como índice de encoding
const T0: i32 = 6; // x5 en ids del binding (xN = N+1)

const NOP: u32 = 0x0000_0013;

fn lui(rd: u32, imm20: u32) -> u32 {
    (imm20 << 12) | (rd << 7) | 0x37
}
fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    ((imm as u32 & 0xFFF) << 20) | (rs1 << 15) | (rd << 7) | 0x13
}
fn flw(rd: u32, rs1: u32, imm: u32) -> u32 {
    (imm << 20) | (rs1 << 15) | (2 << 12) | (rd << 7) | 0x07
}
fn fld(rd: u32, rs1: u32, imm: u32) -> u32 {
    (imm << 20) | (rs1 << 15) | (3 << 12) | (rd << 7) | 0x07
}
fn fadd_s(rd: u32, rs1: u32, rs2: u32) -> u32 {
    (rs2 << 20) | (rs1 << 15) | (7 << 12) | (rd << 7) | 0x53
}
fn fadd_d(rd: u32, rs1: u32, rs2: u32) -> u32 {
    (0x01 << 25) | (rs2 << 20) | (rs1 << 15) | (7 << 12) | (rd << 7) | 0x53
}
fn fmv_x_w(rd: u32, rs1: u32) -> u32 {
    (0x70 << 25) | (rs1 << 15) | (rd << 7) | 0x53
}
fn csrrs_rd(csr: u32, rd: u32) -> u32 {
    (csr << 20) | (2 << 12) | (rd << 7) | 0x73
}

fn motor(words: &[u32]) -> Unicorn<'static, ()> {
    let mut uc = Unicorn::new(Arch::RISCV, Mode::RISCV32).expect("unicorn");
    uc.mem_map(BASE, 0x1000, Prot::READ | Prot::WRITE | Prot::EXEC)
        .expect("map");
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    uc.mem_write(BASE, &bytes).expect("write");
    uc
}

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

/// Programa común: t0 ← DATOS; flw f0/f1; fadd.s f2 = f0+f1.
fn prog_fadd_s() -> Vec<u32> {
    vec![
        lui(T0X, 0x400),
        addi(T0X, T0X, 0x700),
        flw(0, T0X, 0),
        flw(1, T0X, 4),
        fadd_s(2, 0, 1),
        NOP,
    ]
}

/// P1: estado CRUDO — ¿fadd.s ejecuta o da ILLEGAL con mstatus por defecto?
#[test]
fn p1_fadd_s_estado_crudo() {
    let (res, mstatus, f2) = correr("p1", || {
        let mut uc = motor(&prog_fadd_s());
        uc.mem_write(DATOS, &1.5f32.to_le_bytes()).unwrap();
        uc.mem_write(DATOS + 4, &0.25f32.to_le_bytes()).unwrap();
        let mstatus = uc.reg_read(MSTATUS).unwrap_or(u64::MAX);
        let res = uc.emu_start(BASE, BASE + 24, 0, 20);
        (res, mstatus, uc.reg_read(F2).unwrap_or(u64::MAX))
    });
    eprintln!("[P1] mstatus inicial={mstatus:#x} res={res:?} f2={f2:#x}");
    // Documental: la tabla va a BITACORA. Sin assert duro sobre res.
}

/// P2: si P1 falla, ¿alcanza con FS=Initial (mstatus |= 1<<13) escrito por
/// el HOST antes de arrancar? Este es el mecanismo candidato para load().
#[test]
fn p2_fadd_s_con_fs_inicializado() {
    let (res, f2, mstatus_post) = correr("p2", || {
        let mut uc = motor(&prog_fadd_s());
        uc.mem_write(DATOS, &1.5f32.to_le_bytes()).unwrap();
        uc.mem_write(DATOS + 4, &0.25f32.to_le_bytes()).unwrap();
        let mstatus = uc.reg_read(MSTATUS).unwrap_or(0);
        uc.reg_write(MSTATUS, mstatus | (1 << 13)).expect("FS=Initial");
        let res = uc.emu_start(BASE, BASE + 24, 0, 20);
        (
            res,
            uc.reg_read(F2).unwrap_or(u64::MAX),
            uc.reg_read(MSTATUS).unwrap_or(u64::MAX),
        )
    });
    eprintln!("[P2] res={res:?} f2={f2:#x} mstatus_post={mstatus_post:#x}");
    assert!(res.is_ok(), "con FS=Initial fadd.s debe ejecutar: {res:?}");
    // 1.5 + 0.25 = 1.75 = 0x3FE00000 en f32; el registro es de 64 bits.
    assert_eq!(
        f32::from_bits(f2 as u32),
        1.75f32,
        "no es el patrón de bits de 1.75f32: {f2:#x}"
    );
}

/// P3: round-trip host↔guest de los registros f por id crudo (158..189):
/// el host escribe F0/F1, el guest suma, el host lee F2.
#[test]
fn p3_round_trip_host_guest() {
    let (res, f2) = correr("p3", || {
        let prog = [fadd_s(2, 0, 1), NOP];
        let mut uc = motor(&prog);
        let mstatus = uc.reg_read(MSTATUS).unwrap_or(0);
        uc.reg_write(MSTATUS, mstatus | (1 << 13)).unwrap();
        // NaN-boxing manual: single en registro de 64 = altos en unos.
        uc.reg_write(F0, 0xFFFF_FFFF_0000_0000 | u64::from(2.5f32.to_bits()))
            .expect("write F0");
        uc.reg_write(F1, 0xFFFF_FFFF_0000_0000 | u64::from(0.5f32.to_bits()))
            .expect("write F1");
        let res = uc.emu_start(BASE, BASE + 8, 0, 10);
        (res, uc.reg_read(F2).unwrap_or(u64::MAX))
    });
    eprintln!("[P3] res={res:?} f2={f2:#x}");
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(f2 as u32, 3.0f32.to_bits(), "2.5 + 0.5 = 3.0 exacto");
}

/// P4: NaN-boxing — ¿flw deja los 32 altos en unos (como exige la espec
/// D para singles en registros de 64)?
#[test]
fn p4_nan_boxing_de_flw() {
    let (res, f0, f2) = correr("p4", || {
        let mut uc = motor(&prog_fadd_s());
        uc.mem_write(DATOS, &1.5f32.to_le_bytes()).unwrap();
        uc.mem_write(DATOS + 4, &0.25f32.to_le_bytes()).unwrap();
        let mstatus = uc.reg_read(MSTATUS).unwrap_or(0);
        uc.reg_write(MSTATUS, mstatus | (1 << 13)).unwrap();
        let res = uc.emu_start(BASE, BASE + 24, 0, 20);
        (
            res,
            uc.reg_read(F0).unwrap_or(u64::MAX),
            uc.reg_read(F2).unwrap_or(u64::MAX),
        )
    });
    eprintln!("[P4] res={res:?} f0={f0:#x} f2={f2:#x}");
    assert!(res.is_ok());
    assert_eq!(f0 >> 32, 0xFFFF_FFFF, "flw NaN-boxea: altos en unos");
    // HALLAZGO (medido, va a BITACORA): Unicorn 2.1.5 NO nan-boxea el
    // RESULTADO de las operaciones .s — los 32 altos quedan en CERO
    // (apartamiento de la espec D, que exige boxear). Regla derivada para
    // el panel: alto==0xFFFFFFFF O (alto==0 y bajo≠0) → interpretar f32.
    assert_eq!(
        f2 >> 32,
        0,
        "si esto cambia (Unicorn boxeando resultados), revisar la regla \
         de interpretación del panel F"
    );
}

/// P5: fadd.d con valores exactos; el registro trae el f64 crudo.
#[test]
fn p5_fadd_d() {
    let (res, f2) = correr("p5", || {
        let prog = [
            lui(T0X, 0x400),
            addi(T0X, T0X, 0x700),
            fld(0, T0X, 0),
            fld(1, T0X, 8),
            fadd_d(2, 0, 1),
            NOP,
        ];
        let mut uc = motor(&prog);
        uc.mem_write(DATOS, &1.5f64.to_le_bytes()).unwrap();
        uc.mem_write(DATOS + 8, &0.25f64.to_le_bytes()).unwrap();
        let mstatus = uc.reg_read(MSTATUS).unwrap_or(0);
        uc.reg_write(MSTATUS, mstatus | (1 << 13)).unwrap();
        let res = uc.emu_start(BASE, BASE + 24, 0, 20);
        (res, uc.reg_read(F2).unwrap_or(u64::MAX))
    });
    eprintln!("[P5] res={res:?} f2={f2:#x}");
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(f2, 1.75f64.to_bits(), "1.5 + 0.25 = 1.75 en f64 exacto");
}

/// P6: FCSR accesible por id host (141) y por csrr del guest (0x003);
/// fmv.x.w mueve bits de f a x.
#[test]
fn p6_fcsr_y_fmv() {
    let (res, t0, fcsr_host) = correr("p6", || {
        let prog = [
            fmv_x_w(T0X, 0),       // t0 ← bits bajos de f0
            csrrs_rd(0x003, T0X),  // t0 ← fcsr (lo pisa: probamos ambos)
            NOP,
        ];
        let mut uc = motor(&prog);
        let mstatus = uc.reg_read(MSTATUS).unwrap_or(0);
        uc.reg_write(MSTATUS, mstatus | (1 << 13)).unwrap();
        uc.reg_write(F0, 0xFFFF_FFFF_0000_0000 | u64::from(1.5f32.to_bits()))
            .unwrap();
        // primero solo el fmv para capturar t0
        let res1 = uc.emu_start(BASE, BASE + 4, 0, 5);
        let t0 = uc.reg_read(T0).unwrap_or(u64::MAX);
        let res2 = uc.emu_start(BASE + 4, BASE + 8, 0, 5);
        let fcsr_host = uc.reg_read(FCSR).unwrap_or(u64::MAX);
        (res1.and(res2), t0, fcsr_host)
    });
    eprintln!("[P6] res={res:?} t0={t0:#x} fcsr_host={fcsr_host:#x}");
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(t0 as u32, 1.5f32.to_bits(), "fmv.x.w trajo los bits");
    assert_ne!(fcsr_host, u64::MAX, "FCSR legible por id 141");
}
