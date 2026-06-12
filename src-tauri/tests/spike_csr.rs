//! RP8 Fase 1 — spike: ¿qué hace Unicorn 2.1.5 con Zicsr, mret y traps
//! cuando el guest configura mtvec? (patrón spike_mmio/spike_count: PoC con
//! Unicorn CRUDO y evidencia antes de comprometer arquitectura).
//!
//! Encodings ensamblados A MANO y verificados contra la espec RV32 Zicsr
//! (capítulo 9, formato I sobre opcode SYSTEM 0x73):
//!   csrrw  x0, csr, rs1 = (csr<<20)|(rs1<<15)|(1<<12)|(0<<7)|0x73
//!   csrrs  rd, csr, x0  = (csr<<20)|(0<<15)|(2<<12)|(rd<<7)|0x73
//!   csrrwi x0, csr, imm = (csr<<20)|(imm<<15)|(5<<12)|(0<<7)|0x73
//!   mret = 0x30200073 · ecall = 0x00000073
//! CSR ids del binding (verificados por la coordinadora): MSTATUS=116,
//! MTVEC=121, MSCRATCH=127, MEPC=128, MCAUSE=129, MTVAL=130.

use std::sync::mpsc::channel;
use std::time::Duration;
use unicorn_engine::{Arch, HookType, Mode, Prot, RegisterRISCV, Unicorn};

const BASE: u64 = 0x0040_0000;
// Registros x del binding: xN = N+1 (X0 == 1).
const T1: i32 = 7; // x6
const T2: i32 = 8; // x7
                   // CSRs (ids crudos del binding).
const MSCRATCH: i32 = 127;
const MTVEC: i32 = 121;
const MEPC: i32 = 128;
const MCAUSE: i32 = 129;
const MTVAL: i32 = 130;

fn csrrw_x0(csr: u32, rs1: u32) -> u32 {
    (csr << 20) | (rs1 << 15) | (1 << 12) | 0x73
}
fn csrrs_rd(csr: u32, rd: u32) -> u32 {
    (csr << 20) | (2 << 12) | (rd << 7) | 0x73
}
fn csrrwi_x0(csr: u32, imm5: u32) -> u32 {
    (csr << 20) | (imm5 << 15) | (5 << 12) | 0x73
}
fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    ((imm as u32 & 0xFFF) << 20) | (rs1 << 15) | (rd << 7) | 0x13
}
fn lui(rd: u32, imm20: u32) -> u32 {
    (imm20 << 12) | (rd << 7) | 0x37
}
const MRET: u32 = 0x3020_0073;
const ECALL: u32 = 0x0000_0073;
const NOP: u32 = 0x0000_0013;

/// Motor con .text mapeado y el programa cargado. Sin hooks de Machine:
/// este spike mide el comportamiento CRUDO primero.
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

/// P1: ¿csrrw/csrrs/csrrwi ejecutan NATIVOS y se reflejan en el host?
#[test]
fn p1_csr_nativos() {
    let (res, scratch, t1) = correr("p1", || {
        // li t0, 0x2A; csrw mscratch, t0; csrr t1, mscratch; nop
        let prog = [
            addi(5, 0, 0x2A),
            csrrw_x0(0x340, 5),
            csrrs_rd(0x340, 6),
            NOP,
        ];
        let mut uc = motor(&prog);
        let res = uc.emu_start(BASE, BASE + 16, 0, 10);
        let scratch = uc.reg_read(MSCRATCH).unwrap_or(u64::MAX);
        let t1 = uc.reg_read(T1).unwrap_or(u64::MAX);
        (res, scratch, t1)
    });
    eprintln!("[P1] res={res:?} mscratch={scratch:#x} t1={t1:#x}");
    assert!(res.is_ok(), "csrrw/csrrs deben ejecutar: {res:?}");
    assert_eq!(scratch, 0x2A, "el host ve lo que el guest escribió");
    assert_eq!(t1, 0x2A, "csrr devolvió el valor");
}

/// P1b: variante inmediata.
#[test]
fn p1b_csrrwi() {
    let (res, scratch) = correr("p1b", || {
        let prog = [csrrwi_x0(0x340, 21), NOP];
        let mut uc = motor(&prog);
        let res = uc.emu_start(BASE, BASE + 8, 0, 5);
        (res, uc.reg_read(MSCRATCH).unwrap_or(u64::MAX))
    });
    eprintln!("[P1b] res={res:?} mscratch={scratch}");
    assert!(res.is_ok());
    assert_eq!(scratch, 21);
}

/// P2: mret — pc ← mepc.
#[test]
fn p2_mret() {
    let (res, pc) = correr("p2", || {
        // lui t0, 0x400; addi t0, t0, 0x100 (=0x400100); csrw mepc, t0; mret
        let prog = [lui(5, 0x400), addi(5, 5, 0x100), csrrw_x0(0x341, 5), MRET];
        let mut uc = motor(&prog);
        // destino con un nop para que el run pueda terminar ahí
        uc.mem_write(BASE + 0x100, &NOP.to_le_bytes()).unwrap();
        let res = uc.emu_start(BASE, BASE + 0x104, 0, 10);
        (res, uc.reg_read(RegisterRISCV::PC).unwrap_or(0))
    });
    eprintln!("[P2] res={res:?} pc={pc:#x}");
    assert!(res.is_ok(), "mret debe ejecutar: {res:?}");
    assert_eq!(pc, BASE + 0x104, "mret saltó a mepc y siguió");
}

/// P3a: ecall con mtvec configurado y SIN hook INTR: ¿vectoriza al guest?
#[test]
fn p3a_ecall_sin_hook() {
    let (res, pc, mcause, mepc) = correr("p3a", || {
        // handler en BASE+0x200: csrr t2, mcause; nop (queda ahí)
        // main: csrw mtvec = BASE+0x200; ecall
        let prog = [lui(5, 0x400), addi(5, 5, 0x200), csrrw_x0(0x305, 5), ECALL];
        let mut uc = motor(&prog);
        let handler = [csrrs_rd(0x342, 7), NOP];
        let hbytes: Vec<u8> = handler.iter().flat_map(|w| w.to_le_bytes()).collect();
        uc.mem_write(BASE + 0x200, &hbytes).unwrap();
        let res = uc.emu_start(BASE, BASE + 0x208, 0, 20);
        (
            res,
            uc.reg_read(RegisterRISCV::PC).unwrap_or(0),
            uc.reg_read(MCAUSE).unwrap_or(u64::MAX),
            uc.reg_read(MEPC).unwrap_or(u64::MAX),
        )
    });
    eprintln!("[P3a ecall sin hook] res={res:?} pc={pc:#x} mcause={mcause} mepc={mepc:#x}");
    // Sin assert duro: este test DOCUMENTA el comportamiento (la tabla va a
    // BITACORA). Lo único exigido: no colgarse.
}

/// P3b: ecall con mtvec Y hook INTR instalado: ¿el hook gana? ¿puede
/// REINYECTAR al guest (escribir mepc/mcause/pc y devolver SIN emu_stop)?
#[test]
fn p3b_ecall_con_hook_reinyeccion() {
    let (res, pc, t2, hook_visto) = correr("p3b", || {
        let prog = [
            lui(5, 0x400),
            addi(5, 5, 0x200),
            csrrw_x0(0x305, 5),
            ECALL,
            addi(6, 0, 7), // tras el mret del handler debería correr esto
            NOP,
        ];
        let mut uc = motor(&prog);
        // handler: csrr t2, mcause ; mret
        let handler = [csrrs_rd(0x342, 7), MRET];
        let hbytes: Vec<u8> = handler.iter().flat_map(|w| w.to_le_bytes()).collect();
        uc.mem_write(BASE + 0x200, &hbytes).unwrap();

        let visto = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let visto_hook = std::sync::Arc::clone(&visto);
        uc.add_intr_hook(move |uc, intno| {
            visto_hook.store(true, std::sync::atomic::Ordering::SeqCst);
            // REINYECCIÓN: simular la entrada al trap del guest a mano.
            let pc = uc.reg_read(RegisterRISCV::PC).unwrap_or(0);
            // (convención RP-1: pc ya avanzado tras ecall → mepc = pc-4)
            let _ = uc.reg_write(MEPC, pc.wrapping_sub(4));
            let _ = uc.reg_write(MCAUSE, u64::from(intno));
            let mtvec = uc.reg_read(MTVEC).unwrap_or(0);
            let _ = uc.reg_write(RegisterRISCV::PC, mtvec);
            // SIN emu_stop: ¿la emulación sigue en el handler?
        })
        .expect("intr hook");
        let res = uc.emu_start(BASE, BASE + 0x18, 0, 50);
        (
            res,
            uc.reg_read(RegisterRISCV::PC).unwrap_or(0),
            uc.reg_read(T2).unwrap_or(u64::MAX),
            visto.load(std::sync::atomic::Ordering::SeqCst),
        )
    });
    eprintln!(
        "[P3b ecall con hook] res={res:?} pc={pc:#x} t2(mcause leído)={t2} hook={hook_visto}"
    );
    // Documental: lo crítico es saber si t2 == 8/11 (handler corrió) y si
    // el mret volvió (pc avanzó más allá del ecall).
}

/// P3c: instrucción ilegal con mtvec, sin hooks: ¿vectoriza o uc_error?
#[test]
fn p3c_ilegal_sin_hook() {
    let (res, pc, mcause) = correr("p3c", || {
        let prog = [
            lui(5, 0x400),
            addi(5, 5, 0x200),
            csrrw_x0(0x305, 5),
            0xFFFF_FFFF, // ilegal
        ];
        let mut uc = motor(&prog);
        let handler = [csrrs_rd(0x342, 7), NOP];
        let hbytes: Vec<u8> = handler.iter().flat_map(|w| w.to_le_bytes()).collect();
        uc.mem_write(BASE + 0x200, &hbytes).unwrap();
        let res = uc.emu_start(BASE, BASE + 0x208, 0, 20);
        (
            res,
            uc.reg_read(RegisterRISCV::PC).unwrap_or(0),
            uc.reg_read(MCAUSE).unwrap_or(u64::MAX),
        )
    });
    eprintln!("[P3c ilegal sin hook] res={res:?} pc={pc:#x} mcause={mcause}");
}

/// P3d: acceso a memoria no mapeada con mtvec, sin hooks.
#[test]
fn p3d_acceso_invalido_sin_hook() {
    let (res, pc, mcause, mtval) = correr("p3d", || {
        let prog = [
            lui(5, 0x400),
            addi(5, 5, 0x200),
            csrrw_x0(0x305, 5),
            lui(6, 0x99999),
            // lw t2, 0(t1) = (0<<20)|(6<<15)|(2<<12)|(7<<7)|0x03
            (6 << 15) | (2 << 12) | (7 << 7) | 0x03,
        ];
        let mut uc = motor(&prog);
        let handler = [csrrs_rd(0x342, 7), NOP];
        let hbytes: Vec<u8> = handler.iter().flat_map(|w| w.to_le_bytes()).collect();
        uc.mem_write(BASE + 0x200, &hbytes).unwrap();
        let res = uc.emu_start(BASE, BASE + 0x208, 0, 20);
        (
            res,
            uc.reg_read(RegisterRISCV::PC).unwrap_or(0),
            uc.reg_read(MCAUSE).unwrap_or(u64::MAX),
            uc.reg_read(MTVAL).unwrap_or(u64::MAX),
        )
    });
    eprintln!(
        "[P3d acceso inválido sin hook] res={res:?} pc={pc:#x} mcause={mcause} mtval={mtval:#x}"
    );
}

/// P3e: ebreak con mtvec, sin hooks (RP-1 dice que sin mtvec es
/// INSN_INVALID — ¿mtvec lo cambia?).
#[test]
fn p3e_ebreak_con_mtvec() {
    let (res, pc) = correr("p3e", || {
        let prog = [
            lui(5, 0x400),
            addi(5, 5, 0x200),
            csrrw_x0(0x305, 5),
            0x0010_0073, // ebreak
        ];
        let mut uc = motor(&prog);
        uc.mem_write(BASE + 0x200, &NOP.to_le_bytes()).unwrap();
        let res = uc.emu_start(BASE, BASE + 0x204, 0, 20);
        (res, uc.reg_read(RegisterRISCV::PC).unwrap_or(0))
    });
    eprintln!("[P3e ebreak con mtvec] res={res:?} pc={pc:#x}");
    // Documental: si res == Err(EXCEPTION/INSN_INVALID), RP-1 sigue intacto
    // incluso con mtvec; si vectoriza, hay que protegerlo en la delegación.
}

/// P4: hook INTR con uc_error: ¿qué entrega exactamente el res cuando hay
/// hook que NO frena? (complemento de P3b: res esperado Ok si el guest
/// siguió hasta el «hasta» del emu_start.)
#[test]
fn p4_resumen() {
    // Solo deja constancia en el log del orden de ejecución del spike.
    eprintln!("[P4] ver tabla consolidada en BITACORA RP8-F1");
}

/// P2b: verificación de los IDs host: el guest escribe mepc/mtvec con csrw
/// y el HOST los lee por id crudo — deben coincidir.
#[test]
fn p2b_ids_host_coinciden() {
    let (mepc_host, mtvec_host) = correr("p2b", || {
        let prog = [
            lui(5, 0x12345),
            csrrw_x0(0x341, 5), // mepc  = 0x12345000
            lui(6, 0x67800),
            csrrw_x0(0x305, 6), // mtvec = 0x67800000
            NOP,
        ];
        let mut uc = motor(&prog);
        let _ = uc.emu_start(BASE, BASE + 20, 0, 10);
        (
            uc.reg_read(MEPC).unwrap_or(u64::MAX),
            uc.reg_read(MTVEC).unwrap_or(u64::MAX),
        )
    });
    eprintln!("[P2b] mepc(host id 128)={mepc_host:#x} mtvec(host id 121)={mtvec_host:#x}");
    assert_eq!(mepc_host, 0x1234_5000, "id 128 ES mepc");
    assert_eq!(mtvec_host, 0x6780_0000, "id 121 ES mtvec");
}

/// P3b2: reinyección con mepc CONOCIDO (sin confiar en el pc leído dentro
/// del hook crudo): el handler corre, lee mcause, hace mret y el programa
/// CONTINÚA después del ecall. Este es el mecanismo candidato de F3.
#[test]
fn p3b2_reinyeccion_con_mepc_conocido() {
    let (res, t1, t2) = correr("p3b2", || {
        let prog = [
            lui(5, 0x400),
            addi(5, 5, 0x200),
            csrrw_x0(0x305, 5),
            ECALL,         // BASE+0xC
            addi(6, 0, 7), // BASE+0x10 ← mret debe aterrizar acá
            NOP,
        ];
        let mut uc = motor(&prog);
        let handler = [csrrs_rd(0x342, 7), MRET];
        let hbytes: Vec<u8> = handler.iter().flat_map(|w| w.to_le_bytes()).collect();
        uc.mem_write(BASE + 0x200, &hbytes).unwrap();
        uc.add_intr_hook(move |uc, intno| {
            // mepc = dirección DESPUÉS del ecall, conocida por el host
            // (en producción handle_trap la deriva del pc confiable).
            let _ = uc.reg_write(MEPC, BASE + 0x10);
            let _ = uc.reg_write(MCAUSE, u64::from(intno));
            let mtvec = uc.reg_read(MTVEC).unwrap_or(0);
            let _ = uc.reg_write(RegisterRISCV::PC, mtvec);
        })
        .expect("hook");
        let res = uc.emu_start(BASE, BASE + 0x18, 0, 50);
        (
            res,
            uc.reg_read(T1).unwrap_or(u64::MAX),
            uc.reg_read(T2).unwrap_or(u64::MAX),
        )
    });
    eprintln!("[P3b2] res={res:?} t1={t1} t2(mcause)={t2}");
    assert!(res.is_ok(), "el flujo completo debe terminar: {res:?}");
    assert_eq!(
        t2, 8,
        "el handler leyó mcause (ECALL desde M en Unicorn = 8)"
    );
    assert_eq!(t1, 7, "mret volvió DESPUÉS del ecall y el programa siguió");
}

/// P3f: ¿la reinyección funciona desde el hook MEM_INVALID? (callback bool:
/// true = "arreglado, reintentá"). Escribimos CSRs + pc←mtvec y devolvemos
/// true: ¿la ejecución sigue en el handler o reintenta el acceso?
#[test]
fn p3f_reinyeccion_desde_mem_invalid() {
    let (res, t1, t2, mtval_h) = correr("p3f", || {
        let prog = [
            lui(5, 0x400),
            addi(5, 5, 0x200),
            csrrw_x0(0x305, 5),
            lui(6, 0x99999),
            (6 << 15) | (2 << 12) | (7 << 7) | 0x03, // lw t2, 0(t1) ← inválido (BASE+0x10)
            addi(6, 0, 7),                           // BASE+0x14 ← mret debería aterrizar acá
            NOP,
        ];
        let mut uc = motor(&prog);
        let handler = [csrrs_rd(0x343, 7), MRET]; // csrr t2, mtval ; mret
        let hbytes: Vec<u8> = handler.iter().flat_map(|w| w.to_le_bytes()).collect();
        uc.mem_write(BASE + 0x200, &hbytes).unwrap();
        uc.add_mem_hook(
            HookType::MEM_INVALID,
            1,
            0,
            move |uc, _tipo, dir, _sz, _v| {
                let _ = uc.reg_write(MEPC, BASE + 0x14); // post-lw, conocido
                let _ = uc.reg_write(MCAUSE, 5u64); // load access fault
                let _ = uc.reg_write(MTVAL, dir);
                let mtvec = uc.reg_read(MTVEC).unwrap_or(0);
                let _ = uc.reg_write(RegisterRISCV::PC, mtvec);
                true // "manejado"
            },
        )
        .expect("mem hook");
        let res = uc.emu_start(BASE, BASE + 0x1C, 0, 80);
        (
            res,
            uc.reg_read(T1).unwrap_or(u64::MAX),
            uc.reg_read(T2).unwrap_or(u64::MAX),
            uc.reg_read(MTVAL).unwrap_or(u64::MAX),
        )
    });
    eprintln!("[P3f] res={res:?} t1={t1:#x} t2(mtval leído)={t2:#x} mtval={mtval_h:#x}");
    // Documental: si t2 == 0x99999000 y t1 == 7, la delegación de accesos
    // inválidos es viable con el mismo mecanismo; si no, queda como límite.
}
