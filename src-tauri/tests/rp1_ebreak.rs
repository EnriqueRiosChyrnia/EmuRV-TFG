//! RP1 — Bug latente de `ebreak` (BITACORA ~L282, sesión 2026-06-11).
//!
//! Semántica deseada (RARS/gdb): `ebreak` PAUSA, la GUI muestra el estado,
//! y al reanudar la ejecución continúa en la instrucción INMEDIATAMENTE
//! SIGUIENTE al `ebreak`, sin saltear ninguna ni re-ejecutarlo.
//!
//! Fase 1: este test documenta el bug (test sonda). Mientras el bug exista,
//! FALLA mostrando los valores reales de pc/registros observados — esa
//! evidencia va a BITACORA. La suite restante debe seguir verde.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::TEXT_BASE;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

fn machine_for(src: &str) -> (Machine<'static>, Receiver<ConsoleEvent>, Sender<InputMsg>) {
    let program = assemble(src).expect("el fuente de prueba debe ensamblar");
    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    let m = Machine::new(program, out_tx, in_rx).expect("máquina");
    (m, out_rx, in_tx)
}

/// Guardián anticuelgue (patrón tec1_detener.rs): si la reanudación colgara,
/// el test falla con mensaje claro en vez de colgar la suite.
fn con_guardia<T, F>(nombre: &'static str, limite: Duration, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limite) {
        Ok(v) => v,
        Err(_) => panic!("{nombre}: la máquina no retornó en {limite:?}"),
    }
}

/// Programa del plan (PLAN-RP1 Fase 1). Direcciones (todas las li expanden
/// a 1 instrucción por ser inmediatos chicos):
///   0x0040_0000  li t0, 1
///   0x0040_0004  ebreak          ← dirección del ebreak
///   0x0040_0008  li t1, 2        ← acá debe quedar el pc tras la pausa
///   0x0040_000C  li t2, 3
///   0x0040_0010  li a7, 10
///   0x0040_0014  ecall
const SRC: &str = "
.text
  li t0, 1
  ebreak
  li t1, 2
  li t2, 3
  li a7, 10
  ecall
";

const DIR_EBREAK: u32 = TEXT_BASE + 4;

struct Observado {
    razon_pausa: StopReason,
    pc_en_pausa: u32,
    t0_en_pausa: u32,
    t1_en_pausa: u32,
    razon_final: StopReason,
    t1_final: u32,
    t2_final: u32,
}

#[test]
fn ebreak_pausa_y_reanuda_en_la_instruccion_siguiente() {
    let limite = Duration::from_secs(if cfg!(debug_assertions) { 10 } else { 2 });
    let obs = con_guardia("rp1 sonda", limite, || {
        let (mut m, _out, _in) = machine_for(SRC);

        // Corrida libre acotada: debe frenar en el ebreak, no terminar.
        let razon_pausa = m.run_slice(0, 1_000);
        let pc_en_pausa = m.pc().expect("pc legible en la pausa");
        let regs = m.registers().expect("registros legibles en la pausa");
        let (t0_en_pausa, t1_en_pausa) = (regs[5], regs[6]);

        // Reanudar por el camino real de commands.rs (continuación de Run).
        let razon_final = m.run_slice_continuation(0, 1_000);
        let regs = m.registers().expect("registros legibles al final");
        Observado {
            razon_pausa,
            pc_en_pausa,
            t0_en_pausa,
            t1_en_pausa,
            razon_final,
            t1_final: regs[6],
            t2_final: regs[7],
        }
    });

    // La pausa por ebreak no es fin de programa.
    assert!(
        !matches!(obs.razon_pausa, StopReason::Exited { .. }),
        "el ebreak no debe terminar el programa; razón observada: {:?}",
        obs.razon_pausa
    );
    // Lo previo al ebreak se ejecutó; lo posterior todavía no.
    assert_eq!(obs.t0_en_pausa, 1, "t0 debía valer 1 en la pausa (lo previo corrió)");
    assert_eq!(
        obs.t1_en_pausa, 0,
        "t1 debía valer 0 en la pausa (lo posterior NO debe haberse ejecutado)"
    );
    // Convención RARS/gdb: el pc queda en la instrucción siguiente al ebreak.
    assert_eq!(
        obs.pc_en_pausa,
        DIR_EBREAK + 4,
        "pc tras la pausa debía ser 0x{:08X} (la instrucción siguiente al ebreak); \
         observado 0x{:08X}",
        DIR_EBREAK + 4,
        obs.pc_en_pausa
    );
    // La reanudación ejecuta TODO lo que falta, sin saltear instrucciones.
    assert!(
        matches!(obs.razon_final, StopReason::Exited { code: 0 }),
        "la reanudación debía terminar con Exited{{0}}; observado: {:?}",
        obs.razon_final
    );
    assert_eq!(obs.t1_final, 2, "t1 debía valer 2 al final (instrucción post-ebreak)");
    assert_eq!(obs.t2_final, 3, "t2 debía valer 3 al final");
}
