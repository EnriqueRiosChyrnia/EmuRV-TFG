//! RP7 Fase 3 — edición en vivo de registros y memoria (máquina pausada).

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::{DATA_BASE, KEYBOARD_BASE, SP_INIT, TEXT_BASE};
use std::sync::mpsc::channel;

fn machine_for(src: &str) -> Machine<'static> {
    let program = assemble(src).expect("ensambla");
    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    Machine::new(program, out_tx, in_rx).expect("máquina")
}

/// EL criterio del ROADMAP, literal: editar t0 con la máquina pausada y la
/// SIGUIENTE instrucción ejecutada usa el valor nuevo.
#[test]
fn editar_t0_pausado_la_siguiente_instruccion_usa_el_valor() {
    let src = "
.text
  li t0, 1
  addi t1, t0, 0
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    // Paso 1: ejecuta li t0, 1. Pausa.
    assert!(matches!(m.step(), StopReason::StepDone));
    assert_eq!(m.registers().expect("regs")[5], 1);
    // Edición en vivo: t0 = 99.
    m.write_reg(5, 99).expect("escribir t0");
    // Paso 2: addi t1, t0, 0 — debe usar el 99 recién escrito.
    assert!(matches!(m.step(), StopReason::StepDone));
    assert_eq!(m.registers().expect("regs")[6], 99, "t1 tomó el valor editado");
}

/// Editar el pc: la ejecución continúa exactamente ahí y last_bp queda
/// reseteado (un breakpoint en el destino DEBE frenar, sin supresión).
#[test]
fn editar_pc_salta_y_no_suprime_breakpoints() {
    let src = "
.text
  li t0, 1
  li t1, 2
salto:
  li t2, 3
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    let programa = assemble(src).expect("ensambla");
    let dir_salto = programa.symbols["salto"];
    m.set_breakpoints([dir_salto]).expect("bp");
    // Correr hasta el bp (frena en salto).
    assert_eq!(
        m.run_slice(0, 100_000),
        StopReason::Breakpoint { addr: dir_salto }
    );
    // Editar el pc DE VUELTA al comienzo: last_bp debe resetearse — el
    // próximo Run pasa por «salto» de nuevo y DEBE frenar (sin el reset,
    // la supresión de RP-1 lo dejaría pasar de largo).
    m.write_pc(TEXT_BASE).expect("escribir pc");
    assert_eq!(
        m.run_slice(0, 100_000),
        StopReason::Breakpoint { addr: dir_salto },
        "el bp en el destino debe frenar tras editar el pc"
    );
}

/// Rechazos didácticos: x0, .text, página MMIO, zona inválida, tamaño.
#[test]
fn rechazos_didacticos() {
    let mut m = machine_for(".text\n  li a7, 10\n  ecall\n");
    let e = m.write_reg(0, 1).expect_err("x0");
    assert!(e.contains("x0") && e.contains("ISA"), "{e}");
    let e = m.write_reg(32, 1).expect_err("x32");
    assert!(e.contains("inválido"), "{e}");
    let e = m.write_mem(TEXT_BASE, &[0; 4]).expect_err(".text");
    assert!(e.contains("inmutable"), "{e}");
    let e = m.write_mem(KEYBOARD_BASE, &[1]).expect_err("MMIO");
    assert!(e.contains("teclado"), "{e}");
    let e = m.write_mem(0x2000_0000, &[1]).expect_err("zona");
    assert!(e.contains("zona editable"), "{e}");
    let e = m.write_mem(DATA_BASE, &[]).expect_err("vacío");
    assert!(e.contains("1 a 4"), "{e}");
}

/// write_mem visible por el programa Y por read_mem; byte/half/word; pila.
#[test]
fn write_mem_en_data_y_pila() {
    let src = "
.data
v: .word 0
.text
  la t1, v
  lw t0, 0(t1)
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    // word en .data
    m.write_mem(DATA_BASE, &0xDEAD_BEEFu32.to_le_bytes())
        .expect("word");
    // half y byte encima (little-endian: byte bajo primero)
    m.write_mem(DATA_BASE, &0xBE_EFu16.to_le_bytes()).expect("half");
    m.write_mem(DATA_BASE + 3, &[0x7F]).expect("byte");
    let leido = u32::from_le_bytes(
        m.read_mem(DATA_BASE, 4).expect("host lee").try_into().unwrap(),
    );
    assert_eq!(leido, 0x7FAD_BEEF, "composición word+half+byte");
    // El PROGRAMA ve el valor editado.
    let razon = m.run_slice(0, 100_000);
    assert!(matches!(razon, StopReason::Exited { code: 0 }), "{razon:?}");
    assert_eq!(m.registers().expect("regs")[5], 0x7FAD_BEEF);
    // Pila: válida.
    m.write_mem(SP_INIT - 4, &[1, 2, 3, 4]).expect("pila");
}
