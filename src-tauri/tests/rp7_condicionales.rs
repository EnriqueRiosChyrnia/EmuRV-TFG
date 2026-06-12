//! RP7 Fase 2 — breakpoints condicionales (REG OP VALOR, signed).

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::cond::parse_cond;
use emurv_lib::core::machine::{BreakpointSpec, Machine, StopReason, WatchKind, Watchpoint};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::DATA_BASE;
use std::sync::mpsc::channel;

fn machine_for(src: &str) -> Machine<'static> {
    let program = assemble(src).expect("ensambla");
    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    Machine::new(program, out_tx, in_rx).expect("máquina")
}

fn bp(addr: u32, cond: &str) -> BreakpointSpec {
    BreakpointSpec {
        addr,
        cond: Some(parse_cond(cond).expect("condición válida")),
    }
}

/// EL caso didáctico estrella: parar «cuando t0 == 5» dentro de un bucle.
/// Las primeras 4 pasadas NO cuentan como parada; la 5.ª clava el pc con
/// t0 == 5 exacto.
#[test]
fn para_en_el_bucle_cuando_t0_es_5() {
    let src = "
.text
  li t0, 0
bucle:
  addi t0, t0, 1
  li t1, 10
  bne t0, t1, bucle
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    let dir_bucle = 0x0040_0004; // addi (primera instrucción del bucle)
    m.set_breakpoints([bp(dir_bucle, "t0 == 5")]).expect("bp");
    let razon = m.run_slice(0, 1_000_000);
    assert_eq!(razon, StopReason::Breakpoint { addr: dir_bucle });
    let regs = m.registers().expect("regs");
    assert_eq!(regs[5], 5, "frenó EXACTO cuando t0 == 5");
    // Reanudar: la supresión de RP-1 aplica (era una parada real) y el
    // programa termina sin volver a frenar (la condición vuelve a ser
    // falsa... hasta t0==5 que ya pasó).
    let razon = m.run_slice(0, 1_000_000);
    assert!(matches!(razon, StopReason::Exited { code: 0 }), "{razon:?}");
}

/// Registro contra registro, y hex con bits altos como signed (-1).
#[test]
fn registro_contra_registro_y_hex_signed() {
    let src = "
.text
  li t0, 7
  li t1, 3
  li t1, 7
meta:
  li t2, -1
fin:
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    let programa = assemble(src).expect("ensambla");
    let dir_meta = programa.symbols["meta"];
    let dir_fin = programa.symbols["fin"];
    // t0 == t1 recién cuando el segundo li t1 corrió.
    m.set_breakpoints([bp(dir_meta, "t0 == t1"), bp(dir_fin, "t2 == 0xFFFFFFFF")])
        .expect("bps");
    let r1 = m.run_slice(0, 100_000);
    assert_eq!(r1, StopReason::Breakpoint { addr: dir_meta });
    // 0xFFFFFFFF como i32 = -1: la comparación signed lo encuentra.
    let r2 = m.run_slice(0, 100_000);
    assert_eq!(r2, StopReason::Breakpoint { addr: dir_fin });
}

/// Errores de parseo didácticos: registro inexistente, operador malo,
/// basura al final, «=» suelto.
#[test]
fn errores_de_parseo() {
    let e = parse_cond("t9 == 5").unwrap_err();
    assert!(e.contains("t9") && e.contains("registro"), "{e}");
    let e = parse_cond("t0 <> 5").unwrap_err();
    assert!(e.contains("operador") || e.contains("valen"), "{e}");
    let e = parse_cond("t0 == 5 basura").unwrap_err();
    assert!(e.contains("obra texto") || e.contains("sobra"), "{e}");
    let e = parse_cond("t0 = 5").unwrap_err();
    assert!(e.contains("=="), "{e}");
    let e = parse_cond("").unwrap_err();
    assert!(e.contains("vacía"), "{e}");
    let e = parse_cond("t0 == cinco").unwrap_err();
    assert!(e.contains("cinco"), "{e}");
}

/// Interacción con last_bp (extiende RP-1 F4): una pasada con condición
/// FALSA no es parada — al frenar después por OTRA causa en ese mismo pc,
/// el breakpoint NO se suprime y debe frenar apenas la condición se cumpla.
#[test]
fn condicion_falsa_no_suprime_nada() {
    let src = "
.text
  li t0, 0
bucle:
  addi t0, t0, 1
  li t1, 3
  bne t0, t1, bucle
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    let dir_bucle = 0x0040_0004;
    m.set_breakpoints([bp(dir_bucle, "t0 == 2")]).expect("bp");
    // Tramo por count que corta ANTES de que la condición sea verdadera:
    // addi+li+bne = 3 instr/vuelta; count=4 corta en la 2.ª vuelta con
    // t0==1 y el pc en medio del bucle (sin parada por breakpoint).
    let r = m.run_slice(0, 4);
    assert_eq!(r, StopReason::Paused, "corte por count, no parada");
    // Continuación: el bp NO debe estar suprimido (la última parada no fue
    // ese bp): frena exacto cuando t0 == 2.
    let r = m.run_slice_continuation(0, 1_000_000);
    assert_eq!(r, StopReason::Breakpoint { addr: dir_bucle });
    assert_eq!(m.registers().expect("regs")[5], 2);
}

/// Condicional y watchpoint conviviendo en la misma corrida.
#[test]
fn condicional_y_watchpoint_conviven() {
    let src = "
.data
v: .word 0
.text
  li t0, 0
bucle:
  addi t0, t0, 1
  la t1, v
  sw t0, 0(t1)
  li t2, 4
  bne t0, t2, bucle
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    m.set_watchpoints(vec![Watchpoint {
        addr: DATA_BASE,
        len: 4,
        kind: WatchKind::Write,
    }])
    .expect("watch");
    m.set_breakpoints([bp(0x0040_0004, "t0 == 3")]).expect("bp");
    // La condición se evalúa ANTES de ejecutar el addi de la vuelta: con
    // t0==3 recién es verdadera al ENTRAR a la vuelta 4. Orden esperado:
    // w1, w2, w3 (escrituras), b3 (entrada vuelta 4), w4, exit. Cada
    // reanudación es una ORDEN nueva (run_slice), como hace la GUI — la
    // supresión de RP-1/RP-7 actúa en ese camino.
    let mut paradas = Vec::new();
    let mut razon = m.run_slice(0, 1_000_000);
    for _ in 0..10 {
        match &razon {
            StopReason::Watchpoint { value, .. } => paradas.push(format!("w{value}")),
            StopReason::Breakpoint { .. } => {
                let t0 = m.registers().expect("regs")[5];
                paradas.push(format!("b{t0}"));
            }
            StopReason::Exited { .. } => break,
            otra => panic!("inesperado: {otra:?}"),
        }
        razon = m.run_slice(0, 1_000_000);
    }
    assert_eq!(paradas, vec!["w1", "w2", "w3", "b3", "w4"], "{paradas:?}");
}
