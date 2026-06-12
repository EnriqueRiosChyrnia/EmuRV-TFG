//! RP10 Fase 1 — Exit2 (93) con código real y rechazo didáctico del 17.
//!
//! El núcleo guarda el i32 REAL de a0 (el acotamiento a rango de shell es
//! del CLI). El 17 no existe en RARS (era Exit2 de MARS/MIPS): sin mtvec
//! da el mensaje didáctico que apunta al 93; con mtvec, RP-8 manda y el
//! handler del guest lo recibe como servicio desconocido (mcause=11).

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

/// EL criterio del ROADMAP: 93 termina con el código de a0.
#[test]
fn exit2_propaga_el_codigo_de_a0() {
    let mut m = machine_for(".text\n  li a0, 7\n  li a7, 93\n  ecall\n");
    assert_eq!(correr(&mut m), StopReason::Exited { code: 7 });
}

/// Código 0 explícito y el tope útil del CLI (125) llegan tal cual.
#[test]
fn exit2_cero_y_125() {
    let mut m = machine_for(".text\n  li a0, 0\n  li a7, 93\n  ecall\n");
    assert_eq!(correr(&mut m), StopReason::Exited { code: 0 });
    let mut m = machine_for(".text\n  li a0, 125\n  li a7, 93\n  ecall\n");
    assert_eq!(correr(&mut m), StopReason::Exited { code: 125 });
}

/// Negativo: el núcleo conserva el i32 REAL (-3), sin enmascarar — la
/// vista «& 0xFF estilo Unix» es decisión del CLI, no del núcleo.
#[test]
fn exit2_negativo_conserva_el_i32() {
    let mut m = machine_for(".text\n  li a0, -3\n  li a7, 93\n  ecall\n");
    assert_eq!(correr(&mut m), StopReason::Exited { code: -3 });
}

/// Regresión: la syscall 10 sigue terminando SIEMPRE con código 0 (RARS:
/// no recibe argumento), aunque a0 tenga basura.
#[test]
fn syscall_10_sigue_dando_cero() {
    let mut m = machine_for(".text\n  li a0, 99\n  li a7, 10\n  ecall\n");
    assert_eq!(correr(&mut m), StopReason::Exited { code: 0 });
}

/// 17 sin mtvec: mensaje didáctico que nombra a MARS y apunta al 93.
#[test]
fn syscall_17_sin_mtvec_mensaje_didactico() {
    let mut m = machine_for(".text\n  li a7, 17\n  ecall\n");
    match correr(&mut m) {
        StopReason::RuntimeError { message } => {
            assert!(message.contains("MARS"), "{message}");
            assert!(message.contains("93"), "{message}");
            assert!(message.contains("17"), "{message}");
        }
        otra => panic!("debía dar el error didáctico del 17: {otra:?}"),
    }
}

/// 17 con mtvec: RP-8 manda — el handler del guest lo recibe ANTES que el
/// mensaje didáctico (servicio desconocido, mcause=11) y mret vuelve.
#[test]
fn syscall_17_con_mtvec_se_delega_al_handler() {
    let src = "
.text
main:
  la t0, handler
  csrw mtvec, t0
  li a7, 17
  ecall
  li t5, 42           # debe ejecutarse tras el mret
  li a7, 10
  ecall
handler:
  csrr t1, mcause     # 11 = ecall desconocido (espec)
  csrr t3, mepc
  addi t3, t3, 4
  csrw mepc, t3
  mret
";
    let mut m = machine_for(src);
    let r = correr(&mut m);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
    let regs = m.registers().expect("regs");
    assert_eq!(regs[6], 11, "mcause = 11: el 17 es desconocido para RP-8");
    assert_eq!(regs[30], 42, "el programa siguió tras el mret");
}
