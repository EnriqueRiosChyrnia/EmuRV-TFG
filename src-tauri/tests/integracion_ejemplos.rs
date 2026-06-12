//! Tests de integración (RNF-06): uno por cada ejemplo de RF-09.
//! Cada test ensambla el ejemplo real de `ejemplos/`, lo ejecuta en la
//! máquina y verifica salida de consola y estado final de registros.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::SP_INIT;
use std::sync::mpsc::channel;

/// Ensambla y ejecuta un ejemplo completo, alimentando `inputs` a las
/// syscalls de lectura. Devuelve (salida de consola, registros, motivo).
fn run_program(src: &str, inputs: &[&str]) -> (String, [u32; 32], StopReason) {
    let program = assemble(src).unwrap_or_else(|errs| {
        panic!(
            "el ejemplo debe ensamblar sin errores; hubo:\n{}",
            errs.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    for line in inputs {
        in_tx.send(InputMsg::Line((*line).to_string())).unwrap();
    }
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    let reason = m.run(0);
    let mut output = String::new();
    while let Ok(ev) = out_rx.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            output.push_str(&s);
        }
    }
    (output, m.registers().expect("registros legibles"), reason)
}

#[test]
fn ejemplo_01_hola_mundo() {
    let (out, _regs, reason) = run_program(include_str!("../../ejemplos/01-hola-mundo.asm"), &[]);
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert_eq!(out, "¡Hola, mundo!\n");
}

#[test]
fn ejemplo_02_suma_interactiva() {
    let (out, regs, reason) = run_program(
        include_str!("../../ejemplos/02-suma-interactiva.asm"),
        &["8", "34"],
    );
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert!(out.contains("La suma es: 42\n"), "salida: {out}");
    assert_eq!(regs[8], 8); // s0: primer número
    assert_eq!(regs[9], 34); // s1: segundo número
}

#[test]
fn ejemplo_02_reintenta_entrada_invalida() {
    // La consola reintenta si el usuario no tipea un entero (RF-07).
    let (out, regs, reason) = run_program(
        include_str!("../../ejemplos/02-suma-interactiva.asm"),
        &["ocho", "8", "34"],
    );
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert!(out.contains("no es un entero válido"), "salida: {out}");
    assert!(out.contains("La suma es: 42\n"), "salida: {out}");
    assert_eq!(regs[8], 8); // la entrada inválida no se consumió como número
}

#[test]
fn ejemplo_03_contador() {
    let (out, regs, reason) = run_program(include_str!("../../ejemplos/03-contador.asm"), &[]);
    assert_eq!(reason, StopReason::Exited { code: 0 });
    let esperado: String = (1..=10).map(|n| format!("{n}\n")).collect();
    assert_eq!(out, esperado);
    assert_eq!(regs[5], 11); // t0 terminó pasado el límite
    assert_eq!(regs[6], 10); // t1: límite intacto
}

#[test]
fn ejemplo_04_fibonacci() {
    let (out, regs, reason) = run_program(include_str!("../../ejemplos/04-fibonacci.asm"), &[]);
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert_eq!(out, "0 1 1 2 3 5 8 13 21 34 \n");
    assert_eq!(regs[8], 55); // s0: F(10), el siguiente a imprimir
    assert_eq!(regs[9], 89); // s1: F(11)
    assert_eq!(regs[18], 0); // s2: contador agotado
}

#[test]
fn ejemplo_05_pila_funciones() {
    let (out, regs, reason) =
        run_program(include_str!("../../ejemplos/05-pila-funciones.asm"), &[]);
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert_eq!(out, "5! = 120\n"); // 120 impreso desde a0
    assert_eq!(regs[2], SP_INIT); // sp balanceado: la pila quedó limpia
}
