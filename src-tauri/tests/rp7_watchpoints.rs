//! RP7 Fase 1 — watchpoints con hooks instalados SOLO si hay alguno.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason, WatchKind, Watchpoint};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::{DATA_BASE, KEYBOARD_BASE};
use std::sync::mpsc::channel;

fn machine_for(src: &str) -> Machine<'static> {
    let program = assemble(src).expect("el fuente de prueba debe ensamblar");
    let (out_tx, _out_rx) = channel::<ConsoleEvent>();
    let (_in_tx, in_rx) = channel::<InputMsg>();
    Machine::new(program, out_tx, in_rx).expect("máquina")
}

fn w(addr: u32, len: u8, kind: WatchKind) -> Watchpoint {
    Watchpoint { addr, len, kind }
}

/// Watchpoint de ESCRITURA: dispara con dirección, valor y pc del acceso.
#[test]
fn watch_de_escritura_dispara_con_valor_y_pc() {
    let src = "
.data
v: .word 0
.text
  li t0, 42
  la t1, v
  sw t0, 0(t1)
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    m.set_watchpoints(vec![w(DATA_BASE, 4, WatchKind::Write)])
        .expect("instalar");
    let razon = m.run_slice(0, 100_000);
    match razon {
        StopReason::Watchpoint {
            addr,
            access,
            value,
            pc,
        } => {
            assert_eq!(addr, DATA_BASE, "dirección del acceso");
            assert_eq!(access, WatchKind::Write);
            assert_eq!(value, 42, "el valor ESCRITO");
            // El sw es la 4.ª palabra emitida (li=1, la=2).
            assert_eq!(pc, 0x0040_000C, "pc de la instrucción que escribió");
        }
        otra => panic!("debía frenar por watchpoint, no {otra:?}"),
    }
}

/// Watchpoint de LECTURA: ídem, con el valor leído desde la memoria.
#[test]
fn watch_de_lectura_dispara() {
    let src = "
.data
v: .word 7
.text
  la t1, v
  lw t0, 0(t1)
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    m.set_watchpoints(vec![w(DATA_BASE, 4, WatchKind::Read)])
        .expect("instalar");
    match m.run_slice(0, 100_000) {
        StopReason::Watchpoint { access, value, .. } => {
            assert_eq!(access, WatchKind::Read);
            assert_eq!(value, 7, "el valor LEÍDO");
        }
        otra => panic!("debía frenar por lectura, no {otra:?}"),
    }
}

/// Both frena en ambas direcciones; solapamiento parcial: un sw de 4 bytes
/// que pisa un watch de 1 byte interior dispara igual.
#[test]
fn both_y_solapamiento_parcial() {
    let src = "
.data
v: .word 0
.text
  li t0, 0x11223344
  la t1, v
  sw t0, 0(t1)
  lw t2, 0(t1)
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    // Watch de UN byte en v+2: el sw de 4 bytes lo pisa.
    m.set_watchpoints(vec![w(DATA_BASE + 2, 1, WatchKind::Both)])
        .expect("instalar");
    let r1 = m.run_slice(0, 100_000);
    assert!(
        matches!(
            r1,
            StopReason::Watchpoint {
                access: WatchKind::Write,
                ..
            }
        ),
        "primero la escritura: {r1:?}"
    );
    let r2 = m.run_slice_continuation(0, 100_000);
    assert!(
        matches!(
            r2,
            StopReason::Watchpoint {
                access: WatchKind::Read,
                ..
            }
        ),
        "después la lectura: {r2:?}"
    );
}

/// Las lecturas del HOST (visor de memoria, gdb «m») NO disparan
/// watchpoints: mem_read de la API no pasa por hooks (garantía TEC2).
#[test]
fn lectura_de_host_no_dispara() {
    let src = "
.data
v: .word 9
.text
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    m.set_watchpoints(vec![w(DATA_BASE, 4, WatchKind::Both)])
        .expect("instalar");
    let bytes = m.read_mem(DATA_BASE, 4).expect("leer como host");
    assert_eq!(u32::from_le_bytes(bytes.try_into().unwrap()), 9);
    // Correr a fin: ninguna parada por watchpoint (el guest no toca v).
    let razon = m.run_slice(0, 100_000);
    assert!(
        matches!(razon, StopReason::Exited { code: 0 }),
        "la lectura del host no debió dejar disparo pendiente: {razon:?}"
    );
}

/// Página del teclado MMIO: vetada con error didáctico. Direcciones fuera
/// de toda región: error claro. Tamaños inválidos: error.
#[test]
fn validaciones_de_region_y_tamano() {
    let mut m = machine_for(".text\n  li a7, 10\n  ecall\n");
    let e = m
        .set_watchpoints(vec![w(KEYBOARD_BASE, 4, WatchKind::Read)])
        .expect_err("teclado vetado");
    assert!(e.contains("teclado"), "{e}");
    let e = m
        .set_watchpoints(vec![w(0x2000_0000, 4, WatchKind::Write)])
        .expect_err("zona inválida");
    assert!(e.contains("no está en una zona"), "{e}");
    let e = m
        .set_watchpoints(vec![w(DATA_BASE, 3, WatchKind::Write)])
        .expect_err("len inválido");
    assert!(e.contains("1, 2 o 4"), "{e}");
}

/// El patrón instalar-solo-si-hace-falta es LEY: sin watchpoints no hay
/// hooks; al vaciar la lista, los hooks se van.
#[test]
fn hooks_solo_cuando_hay_watchpoints() {
    let mut m = machine_for(".text\n  li a7, 10\n  ecall\n");
    assert!(!m.watch_hooks_instalados(), "sin watchpoints, sin hooks");
    m.set_watchpoints(vec![w(DATA_BASE, 4, WatchKind::Write)])
        .expect("instalar");
    assert!(m.watch_hooks_instalados());
    m.set_watchpoints(Vec::new()).expect("vaciar");
    assert!(!m.watch_hooks_instalados(), "al vaciar se retiran");
    // Y la máquina sigue corriendo bien después del ciclo instalar/retirar.
    assert!(matches!(
        m.run_slice(0, 100_000),
        StopReason::Exited { code: 0 }
    ));
}
