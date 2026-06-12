//! RP8 Fase 3 — delegación didáctica de traps al handler del guest.
//!
//! Política (BITACORA RP8-F1): mtvec==0 ⇒ TODO como siempre; ecall
//! conocido JAMÁS se delega; ebreak JAMÁS se delega; doble falta ⇒
//! RuntimeError didáctico (protocolo mstatus.MIE, autolimpiante por mret).

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason, WatchKind, Watchpoint};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::DATA_BASE;
use std::sync::mpsc::{channel, Receiver};

fn machine_for(src: &str) -> (Machine<'static>, Receiver<ConsoleEvent>) {
    let program = assemble(src).expect("ensambla");
    let (out_tx, out_rx) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    (
        Machine::new(program, out_tx, in_rx).expect("máquina"),
        out_rx,
    )
}

/// Corre con el bucle de tramos de producción hasta una parada real.
fn correr(m: &mut Machine<'static>) -> StopReason {
    let mut r = m.run_slice(0, 1_000_000);
    for _ in 0..50 {
        if !matches!(r, StopReason::Paused) {
            break;
        }
        r = m.run_slice_continuation(0, 1_000_000);
    }
    r
}

fn salida(rx: &Receiver<ConsoleEvent>) -> String {
    let mut out = String::new();
    while let Ok(ev) = rx.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            out.push_str(&s);
        }
    }
    out
}

/// EL criterio del ROADMAP: ecall desconocida con mtvec → handler corre
/// (lee mcause=11 de la ESPEC), ajusta mepc+4 como un kernel real, mret
/// vuelve y el programa termina OK.
#[test]
fn ecall_desconocida_delegada_y_mret_vuelve() {
    let src = "
.text
main:
  la t0, handler
  csrw mtvec, t0
  li a7, 777          # servicio inexistente
  ecall
  li t5, 42           # debe ejecutarse tras el mret
  li a7, 10
  ecall

handler:
  csrr t1, mcause     # 11 = ecall desde M-mode (espec)
  csrr t2, mepc       # dirección del ecall
  csrr t3, mepc
  addi t3, t3, 4      # ajustar el retorno DESPUÉS del ecall
  csrw mepc, t3
  mret
";
    let (mut m, _rx) = machine_for(src);
    let r = correr(&mut m);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
    let regs = m.registers().expect("regs");
    assert_eq!(regs[6], 11, "mcause de la ESPEC (no el intno de Unicorn)");
    assert_eq!(regs[30], 42, "t5: el programa siguió tras el mret");
    // mepc apuntaba AL ecall (la=2 words, csrw=1, li=1 → ecall en +0x10).
    assert_eq!(regs[7], 0x0040_0010, "mepc = dirección del ecall");
}

/// ecall CONOCIDA con mtvec: syscall del host, JAMÁS delegada.
#[test]
fn ecall_conocida_no_se_delega() {
    let src = "
.text
  la t0, handler
  csrw mtvec, t0
  li a0, 7
  li a7, 1            # imprimir entero: servicio CONOCIDO
  ecall
  li a7, 10
  ecall
handler:
  li t6, 99           # si esto corre, se delegó mal
  mret
";
    let (mut m, rx) = machine_for(src);
    let r = correr(&mut m);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
    assert_eq!(salida(&rx), "7", "la syscall host imprimió normalmente");
    assert_eq!(m.registers().expect("regs")[31], 0, "el handler NO corrió");
}

/// Instrucción ilegal: con mtvec → handler lee mcause=2 y mtval=la palabra;
/// SIN mtvec → el error didáctico actual (regresión fijada).
#[test]
fn ilegal_delegada_y_regresion_sin_mtvec() {
    // Via determinista para ILLEGAL_INSN: caer mas alla del final de .text
    // (pagina R+X rellena de ceros; 0x00000000 es ilegal en RV32). El
    // handler va PRIMERO y main salta por encima, asi la caida tras main
    // entra a los ceros (y no al handler).
    let src4 = "
.text
  j main
handler:
  csrr t1, mcause
  csrr t2, mtval
  li a7, 10
  ecall
main:
  la t0, handler
  csrw mtvec, t0
  nop                 # cae hacia los ceros del final de .text
";
    let (mut m, _rx) = machine_for(src4);
    let r = correr(&mut m);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
    let regs = m.registers().expect("regs");
    assert_eq!(regs[6], 2, "mcause = 2 (instrucción ilegal)");
    assert_eq!(regs[7], 0, "mtval = la palabra culpable (0x00000000)");

    // REGRESIÓN: sin mtvec, el mensaje didáctico de siempre.
    let (mut m, _rx) = machine_for(".text\n  nop\n");
    let r = correr(&mut m);
    match r {
        StopReason::RuntimeError { message } => {
            assert!(message.contains("siguió de largo"), "{message}");
        }
        otra => panic!("sin mtvec debía dar el error didáctico: {otra:?}"),
    }
}

/// Acceso a memoria inválido con mtvec: mcause 5 (carga) y mtval = la
/// dirección culpable; el handler decide terminar.
#[test]
fn acceso_invalido_delegado() {
    let src = "
.text
  j main
handler:
  csrr t1, mcause
  csrr t2, mtval
  li a7, 10
  ecall
main:
  la t0, handler
  csrw mtvec, t0
  li t3, 0x20000000   # zona no mapeada
  lw t4, 0(t3)
";
    let (mut m, _rx) = machine_for(src);
    let r = correr(&mut m);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
    let regs = m.registers().expect("regs");
    assert_eq!(regs[6], 5, "mcause = 5 (fallo de carga)");
    assert_eq!(regs[7], 0x2000_0000, "mtval = dirección culpable");
}

/// Doble falta: el handler a su vez accede a memoria inválida →
/// RuntimeError didáctico, sin colgarse.
#[test]
fn doble_falta_corta_con_error_didactico() {
    let src = "
.text
  j main
handler:
  li t3, 0x30000000
  lw t4, 0(t3)        # segunda excepción DENTRO del handler
  mret
main:
  la t0, handler
  csrw mtvec, t0
  li t5, 0x20000000
  lw t6, 0(t5)        # primera excepción
";
    let (mut m, _rx) = machine_for(src);
    let r = correr(&mut m);
    match r {
        StopReason::RuntimeError { message } => {
            assert!(message.contains("doble falta"), "{message}");
        }
        otra => panic!("debía cortar con doble falta: {otra:?}"),
    }
}

/// ebreak con mtvec configurado: SIGUE pausando (RP-1 intacto, jamás se
/// delega).
#[test]
fn ebreak_con_mtvec_sigue_pausando() {
    let src = "
.text
  la t0, handler
  csrw mtvec, t0
  ebreak
  li a7, 10
  ecall
handler:
  li t6, 99
  mret
";
    let (mut m, _rx) = machine_for(src);
    let r = m.run_slice(0, 1_000);
    assert!(
        matches!(r, StopReason::EbreakPause { .. }),
        "ebreak pausa, no delega: {r:?}"
    );
    assert_eq!(m.registers().expect("regs")[31], 0, "el handler NO corrió");
}

/// Convivencia RP-7: breakpoint DENTRO del handler frena; watchpoint sobre
/// un dato que el handler escribe dispara.
#[test]
fn convivencia_con_breakpoints_y_watchpoints() {
    let src = "
.data
marca: .word 0
.text
  j main
handler:
  li t1, 5
  la t2, marca
  sw t1, 0(t2)
  csrr t3, mepc
  addi t3, t3, 4
  csrw mepc, t3
  mret
main:
  la t0, handler
  csrw mtvec, t0
  li a7, 777
  ecall
  li a7, 10
  ecall
";
    let programa = assemble(src).expect("ensambla");
    let dir_handler = programa.symbols["handler"];
    let (mut m, _rx) = machine_for(src);
    m.set_breakpoints([dir_handler]).expect("bp");
    m.set_watchpoints(vec![Watchpoint {
        addr: DATA_BASE,
        len: 4,
        kind: WatchKind::Write,
    }])
    .expect("watch");

    // 1.ª parada: breakpoint en la PRIMERA instrucción del handler.
    let r = correr(&mut m);
    assert_eq!(r, StopReason::Breakpoint { addr: dir_handler });
    // 2.ª: watchpoint cuando el handler escribe «marca».
    let r = correr(&mut m);
    assert!(
        matches!(
            r,
            StopReason::Watchpoint {
                access: WatchKind::Write,
                value: 5,
                ..
            }
        ),
        "{r:?}"
    );
    // y el programa termina igual.
    let r = correr(&mut m);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
}
