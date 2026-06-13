//! RP12 Fase 3 — syscalls 2/3/6/7: formato de impresión FIJADO (el «{}»
//! de Rust — divergencia con RARS/Java registrada en BITACORA) y lectura
//! con reintento didáctico + NaN-boxing en fa0.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use std::sync::mpsc::{channel, Receiver, Sender};

fn machine_con_canales(src: &str) -> (Machine<'static>, Receiver<ConsoleEvent>, Sender<InputMsg>) {
    let program = assemble(src).expect("ensambla");
    let (out_tx, out_rx) = channel::<ConsoleEvent>();
    let (in_tx, in_rx) = channel::<InputMsg>();
    (
        Machine::new(program, out_tx, in_rx).expect("máquina"),
        out_rx,
        in_tx,
    )
}

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

/// Formato de la syscall 2 (f32) FIJADO: 1.5, -0 (cero negativo), NaN,
/// inf — el «{}» de Rust. RARS/Java imprimiría Infinity: divergencia
/// documentada; este test es el contrato de NUESTRO formato.
#[test]
fn print_float_formato_fijado() {
    let src = "
.data
v: .word 0x3FC00000   # 1.5
n: .word 0x80000000   # -0.0
q: .word 0x7FC00000   # NaN
i: .word 0x7F800000   # +inf
.text
  la t0, v
  flw fa0, 0(t0)
  li a7, 2
  ecall
  li a0, 32
  li a7, 11
  ecall
  flw fa0, 4(t0)
  li a7, 2
  ecall
  li a0, 32
  li a7, 11
  ecall
  flw fa0, 8(t0)
  li a7, 2
  ecall
  li a0, 32
  li a7, 11
  ecall
  flw fa0, 12(t0)
  li a7, 2
  ecall
  li a7, 10
  ecall
";
    let (mut m, rx, _tx) = machine_con_canales(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    assert_eq!(salida(&rx), "1.5 -0 NaN inf");
}

/// Syscall 3 (f64): el doble entero por etiqueta y un valor con decimales.
#[test]
fn print_double_formato_fijado() {
    let src = "
.data
v: .word 0x00000000, 0x3FF80000   # 1.5f64
.text
  fld fa0, v, t0
  li a7, 3
  ecall
  li a7, 10
  ecall
";
    let (mut m, rx, _tx) = machine_con_canales(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    assert_eq!(salida(&rx), "1.5");
}

/// Round-trip 6→2: lee «1.5» y lo reimprime EXACTO; la basura previa
/// dispara el reintento didáctico y fa0 queda NaN-BOXED (espec D).
#[test]
fn read_float_reintento_nan_boxing_y_round_trip() {
    let src = "
.text
  li a7, 6
  ecall          # lee a fa0 (boxed)
  li a7, 2
  ecall          # reimprime
  li a7, 10
  ecall
";
    let (mut m, rx, tx) = machine_con_canales(src);
    tx.send(InputMsg::Line("hola".into())).expect("basura");
    tx.send(InputMsg::Line("1.5".into())).expect("valor");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let out = salida(&rx);
    assert!(
        out.contains("no es un número"),
        "el reintento didáctico avisó: {out}"
    );
    assert!(out.ends_with("1.5"), "round-trip exacto: {out}");
    let f = m.registers_f().expect("f");
    assert_eq!(
        f[10],
        0xFFFF_FFFF_0000_0000 | u64::from(1.5f32.to_bits()),
        "fa0 NaN-boxed con los bits de 1.5f32"
    );
}

/// Syscall 7: double con valor exacto y coma rechazada (punto decimal).
#[test]
fn read_double_y_coma_rechazada() {
    let src = "
.text
  li a7, 7
  ecall
  li a7, 3
  ecall
  li a7, 10
  ecall
";
    let (mut m, rx, tx) = machine_con_canales(src);
    tx.send(InputMsg::Line("2,5".into())).expect("coma");
    tx.send(InputMsg::Line("2.5".into())).expect("punto");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let out = salida(&rx);
    assert!(out.contains("punto decimal"), "{out}");
    assert!(out.ends_with("2.5"), "{out}");
    let f = m.registers_f().expect("f");
    assert_eq!(f[10], 2.5f64.to_bits(), "fa0 = bits crudos del f64");
}

/// write_reg_f del panel: bits crudos visibles por el programa.
#[test]
fn write_reg_f_visible_por_el_programa() {
    let src = "
.text
  fadd.s ft2, fa0, fa1
  li a7, 10
  ecall
";
    let (mut m, _rx, _tx) = machine_con_canales(src);
    m.write_reg_f(10, 0xFFFF_FFFF_0000_0000 | u64::from(2.5f32.to_bits()))
        .expect("fa0");
    m.write_reg_f(11, 0xFFFF_FFFF_0000_0000 | u64::from(0.5f32.to_bits()))
        .expect("fa1");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let f = m.registers_f().expect("f");
    assert_eq!(f[2] as u32, 3.0f32.to_bits(), "el programa usó lo editado");
    // Índice fuera de rango: rechazo claro.
    let e = m.write_reg_f(32, 0).expect_err("f32 no existe");
    assert!(e.contains("f32"), "{e}");
}
