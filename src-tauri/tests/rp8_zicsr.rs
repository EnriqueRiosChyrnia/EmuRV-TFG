//! RP8 Fase 2 — round-trip Zicsr: lo que el ensamblador emite ejecuta
//! NATIVO en Unicorn (el spike F1 lo predijo; esto lo fija con Machine).

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use std::sync::mpsc::channel;

fn machine_for(src: &str) -> Machine<'static> {
    let program = assemble(src).expect("ensambla");
    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    Machine::new(program, out_tx, in_rx).expect("máquina")
}

/// csrw + csrr por el guest: el valor da la vuelta completa.
#[test]
fn round_trip_mscratch() {
    let mut m = machine_for(
        ".text
  li t0, 0x1234
  csrw mscratch, t0
  csrr t1, mscratch
  csrrwi x0, mscratch, 9
  csrr t2, mscratch
  li a7, 10
  ecall
",
    );
    let r = m.run_slice(0, 1_000);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
    let regs = m.registers().expect("regs");
    assert_eq!(regs[6], 0x1234, "csrr leyó lo que csrw escribió");
    assert_eq!(regs[7], 9, "la variante inmediata también");
}

/// mret ensamblado salta a mepc (round-trip del flujo de retorno).
#[test]
fn round_trip_mret() {
    let mut m = machine_for(
        ".text
  la t0, destino
  csrw mepc, t0
  mret
  li t1, 1     # NO debe ejecutarse
destino:
  li t2, 7
  li a7, 10
  ecall
",
    );
    let r = m.run_slice(0, 1_000);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
    let regs = m.registers().expect("regs");
    assert_eq!(regs[6], 0, "el li salteado no corrió");
    assert_eq!(regs[7], 7, "mret aterrizó en destino");
}
