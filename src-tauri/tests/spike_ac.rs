//! RP13 Fase 1 — spike: (A) ¿Unicorn ejecuta atómicas RV32A nativas? y
//! (C) ¿decodifica/ejecuta instrucciones comprimidas de 2 bytes, y a qué
//! costo para el source_map? (patrón spike_csr/spike_fp: Unicorn crudo y
//! evidencia ANTES de comprometer arquitectura).
//!
//! Encodings A a mano (opcode AMO 0x2F, funct3=010 para .w):
//!   amo  = (funct5<<27)|(aq<<26)|(rl<<25)|(rs2<<20)|(rs1<<15)|(2<<12)|(rd<<7)|0x2F
//!   funct5: AMOADD=0x00, AMOSWAP=0x01, LR=0x02, SC=0x03
//! Verificados contra `riscv64-elf-as -march=rv32ima` en el spike:
//!   amoadd.w t0,t1,(t2) = 0x0063a2af ; lr.w t0,(t2) = 0x1003a2af
//!   sc.w t0,t1,(t2) = 0x1863a2af
//! C (2 bytes) contra `-march=rv32imac`:
//!   c.addi a0,1 = 0x0505 ; c.nop = 0x0001 ; c.li a2,5 = 0x4615

use std::sync::mpsc::channel;
use std::time::Duration;
use unicorn_engine::{Arch, Mode, Prot, RegisterRISCV, Unicorn};

const BASE: u64 = 0x0040_0000;
const DATOS: u64 = 0x0040_0700;
// Registros x del binding: xN = N+1.
const T0: i32 = 6; // x5
const A0: i32 = 11; // x10

fn amo(funct5: u32, rs2: u32, rs1: u32, rd: u32) -> u32 {
    (funct5 << 27) | (rs2 << 20) | (rs1 << 15) | (2 << 12) | (rd << 7) | 0x2F
}
const NOP: u32 = 0x0000_0013;

fn lui(rd: u32, imm20: u32) -> u32 {
    (imm20 << 12) | (rd << 7) | 0x37
}
fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    ((imm as u32 & 0xFFF) << 20) | (rs1 << 15) | (rd << 7) | 0x13
}

fn motor_words(words: &[u32]) -> Unicorn<'static, ()> {
    let mut uc = Unicorn::new(Arch::RISCV, Mode::RISCV32).expect("unicorn");
    uc.mem_map(BASE, 0x1000, Prot::READ | Prot::WRITE | Prot::EXEC)
        .expect("map");
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    uc.mem_write(BASE, &bytes).expect("write");
    uc
}

fn motor_bytes(bytes: &[u8]) -> Unicorn<'static, ()> {
    let mut uc = Unicorn::new(Arch::RISCV, Mode::RISCV32).expect("unicorn");
    uc.mem_map(BASE, 0x1000, Prot::READ | Prot::WRITE | Prot::EXEC)
        .expect("map");
    uc.mem_write(BASE, bytes).expect("write");
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

/// A-P1: amoadd.w suma rs2 a la palabra en (rs1) y deja el VIEJO valor en rd.
#[test]
fn a_p1_amoadd_w() {
    let (res, mem, t0_old) = correr("a_p1", || {
        // t2 = DATOS; t1 = 10; amoadd.w t0, t1, (t2)
        let prog = [
            lui(7, 0x400),
            addi(7, 7, 0x700),
            addi(6, 0, 10),
            amo(0x00, 6, 7, 5), // amoadd.w t0, t1, (t2)
            NOP,
        ];
        let mut uc = motor_words(&prog);
        uc.mem_write(DATOS, &100u32.to_le_bytes()).unwrap();
        let res = uc.emu_start(BASE, BASE + 20, 0, 20);
        let mut m = [0u8; 4];
        uc.mem_read(DATOS, &mut m).unwrap();
        (res, u32::from_le_bytes(m), uc.reg_read(T0).unwrap_or(0))
    });
    eprintln!("[A-P1] res={res:?} mem={mem} t0(viejo)={t0_old}");
    assert!(res.is_ok(), "amoadd.w debe ejecutar nativo: {res:?}");
    assert_eq!(mem, 110, "memoria = 100 + 10");
    assert_eq!(t0_old, 100, "rd recibió el valor VIEJO");
}

/// A-P2: lr.w + sc.w — store-conditional EXITOSO (mono-hart: sin otro hart
/// que rompa la reserva, sc devuelve 0 = éxito).
#[test]
fn a_p2_lr_sc_exitoso() {
    let (res, mem, sc_rc) = correr("a_p2", || {
        // t2=DATOS; lr.w t0,(t2); t1=42; sc.w t0,t1,(t2)
        let prog = [
            lui(7, 0x400),
            addi(7, 7, 0x700),
            amo(0x02, 0, 7, 5), // lr.w t0, (t2)
            addi(6, 0, 42),
            amo(0x03, 6, 7, 5), // sc.w t0, t1, (t2)  → t0 = 0 si éxito
            NOP,
        ];
        let mut uc = motor_words(&prog);
        uc.mem_write(DATOS, &7u32.to_le_bytes()).unwrap();
        let res = uc.emu_start(BASE, BASE + 24, 0, 20);
        let mut m = [0u8; 4];
        uc.mem_read(DATOS, &mut m).unwrap();
        (res, u32::from_le_bytes(m), uc.reg_read(T0).unwrap_or(u64::MAX))
    });
    eprintln!("[A-P2] res={res:?} mem={mem} sc_rc={sc_rc}");
    assert!(res.is_ok(), "lr/sc deben ejecutar: {res:?}");
    assert_eq!(mem, 42, "sc escribió el valor");
    assert_eq!(sc_rc, 0, "sc exitoso devuelve 0 (mono-hart, reserva intacta)");
}

/// C-P1: LA PREGUNTA CENTRAL — ¿Unicorn RISCV32 decodifica una C de 2
/// bytes con el modo actual, sin flag? Secuencia MIXTA 4-2-2-4:
///   addi a0,0,5 (4) ; c.addi a0,1 (2) ; c.addi a0,1 (2) ; addi a0,a0,1 (4)
/// Si C anda, a0 = 5+1+1+1 = 8 y el pc avanzó 4+2+2+4 = 12.
#[test]
fn c_p1_comprimida_mixta() {
    let (res, a0, pc) = correr("c_p1", || {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&addi(10, 0, 5).to_le_bytes()); // 4
        bytes.extend_from_slice(&0x0505u16.to_le_bytes()); // c.addi a0,1 (2)
        bytes.extend_from_slice(&0x0505u16.to_le_bytes()); // c.addi a0,1 (2)
        bytes.extend_from_slice(&addi(10, 10, 1).to_le_bytes()); // 4
        bytes.extend_from_slice(&NOP.to_le_bytes());
        let mut uc = motor_bytes(&bytes);
        let res = uc.emu_start(BASE, BASE + 12, 0, 20);
        (
            res,
            uc.reg_read(A0).unwrap_or(u64::MAX),
            uc.reg_read(RegisterRISCV::PC).unwrap_or(0),
        )
    });
    eprintln!("[C-P1] res={res:?} a0={a0} pc={pc:#x} (esperado a0=8 pc=BASE+12)");
    // DOCUMENTAL: este test NO asevera nada duro — su salida decide el
    // veredicto de C (la tabla va a BITACORA). Solo exige no colgarse.
}

/// C-P2: ¿una C suelta avanza el pc de a 2? (aísla la pregunta del avance).
#[test]
fn c_p2_avance_pc_de_a_2() {
    let (res, pc) = correr("c_p2", || {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x4615u16.to_le_bytes()); // c.li a2, 5 (2)
        bytes.extend_from_slice(&NOP.to_le_bytes());
        let mut uc = motor_bytes(&bytes);
        // correr UNA instrucción (count=1) y ver dónde quedó el pc
        let res = uc.emu_start(BASE, BASE + 6, 0, 1);
        (res, uc.reg_read(RegisterRISCV::PC).unwrap_or(0))
    });
    eprintln!("[C-P2] res={res:?} pc={pc:#x} (BASE+2 si avanzó de a 2)");
}
