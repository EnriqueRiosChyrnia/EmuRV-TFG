//! TEC1 — «Detener» responde SIEMPRE, incluso en bucles apretados sin
//! syscalls. La pieza nueva es `Machine::run_slice(timeout_ms, max_instr)`:
//! el count corta lo que el timeout de Unicorn no corta (bug conocido,
//! verificado en tests/spike_count.rs). commands.rs ejecuta en tramos de
//! RUN_SLICE_COUNT instrucciones y chequea la señal de stop entre tramos.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::TEXT_BASE;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// Mismo orden de magnitud que RUN_SLICE_COUNT en commands.rs (la const es
/// privada del binario; acá lo que se valida es el mecanismo).
const SLICE: u64 = 1_000_000;

/// Máquina lista + extremos de los canales de consola (patrón de los tests
/// de machine.rs). Los extremos se devuelven para que vivan lo que dure el
/// test (si se soltaran, las syscalls de consola fallarían).
fn machine_for(src: &str) -> (Machine<'static>, Receiver<ConsoleEvent>, Sender<InputMsg>) {
    let program = assemble(src).expect("el fuente de prueba debe ensamblar");
    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    let m = Machine::new(program, out_tx, in_rx).expect("máquina");
    (m, out_rx, in_tx)
}

/// Corre `f` en un hilo aparte y espera el resultado a lo sumo `limite`:
/// si `run_slice` colgara (la regresión que este archivo vigila), el test
/// FALLA con mensaje claro en vez de colgar la suite (patrón de
/// tests/spike_count.rs; el hilo zombi muere con el proceso). La máquina se
/// construye adentro del hilo porque Unicorn no es Send.
fn con_guardia<T, F>(nombre: &'static str, limite: Duration, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        // Si el receptor ya abandonó (timeout), el send falla y da igual.
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limite) {
        Ok(v) => v,
        Err(_) => panic!(
            "{nombre}: run_slice no retornó en {limite:?}; \
             «Detener» volvería a colgarse en bucles apretados"
        ),
    }
}

/// (a) Un bucle infinito apretado sin syscalls devuelve el control al
/// agotar el count del tramo. Con `run(timeout)` esto NO retornaba jamás:
/// es exactamente el caso por el que «Detener» no respondía.
#[test]
fn detener_interrumpe_bucle_apretado() {
    // En release el tramo tarda ~2 ms; en debug el core C de Unicorn se
    // compila sin optimizar y se le da margen (mismo criterio que el spike).
    let limite = Duration::from_secs(if cfg!(debug_assertions) { 10 } else { 1 });
    let (reason, pc, ms) = con_guardia("bucle apretado", limite, || {
        let (mut m, _out, _inp) = machine_for("inicio: j inicio");
        let t0 = Instant::now();
        let reason = m.run_slice(100, SLICE);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        (reason, m.pc().expect("leer pc"), ms)
    });
    eprintln!("bucle apretado: run_slice(100, {SLICE}) → {reason:?} en {ms:.1} ms");
    // Count agotado sin evento = Paused (tramo terminado, NO pausa de
    // usuario: commands.rs decide si fue Detener mirando su stop_flag).
    assert_eq!(
        reason,
        StopReason::Paused,
        "el tramo debe terminar en Paused"
    );
    // `j inicio` salta siempre a sí mismo: el pc queda clavado en la base.
    assert_eq!(pc, TEXT_BASE, "el pc debe seguir en el bucle");
}

/// (b) El slicing no pierde ni duplica instrucciones: un contador ejecutado
/// en N tramos termina con el valor exacto, y de verdad necesita varios
/// tramos (si entrara en uno solo, el test no probaría nada).
#[test]
fn slicing_no_pierde_instrucciones() {
    // li t1 (lui+addi: 2) + 2×300 000 del bucle + li a7 + ecall = 600 004
    // instrucciones; con tramos de 100 000 son ≥6 tramos Paused + el final.
    let src = "li t1, 300000\nbucle:\naddi t0, t0, 1\nbne t0, t1, bucle\nli a7, 10\necall";
    let (reason, t0, tramos) =
        con_guardia("contador en tramos", Duration::from_secs(30), move || {
            let (mut m, _out, _inp) = machine_for(src);
            let mut tramos = 0u32;
            let reason = loop {
                tramos += 1;
                assert!(tramos <= 1_000, "demasiados tramos: el contador no avanza");
                match m.run_slice(100, 100_000) {
                    StopReason::Paused => {}
                    other => break other,
                }
            };
            (reason, m.registers().expect("registros")[5], tramos)
        });
    eprintln!("contador en tramos: {tramos} tramos → {reason:?}, t0={t0}");
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert_eq!(
        t0, 300_000,
        "el contador debe ser exacto a través de los tramos"
    );
    assert!(
        tramos > 1,
        "con count=100k el programa DEBE repartirse en varios tramos"
    );
}

/// (c) Regresión de semántica: `run()` sin límite de instrucciones sigue
/// funcionando igual que antes de TEC1 (delega en run_slice con
/// max_instr=0): un programa corto corre a Exited(0) en UNA llamada.
#[test]
fn run_sin_limite_sigue_funcionando() {
    let (mut m, _out, _inp) = machine_for("li t0, 7\nli a7, 10\necall");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    assert_eq!(m.registers().expect("registros")[5], 7);
}

/// (d) Regresión: breakpoint en el borde exacto del count. Si un tramo
/// agota su count JUSTO en la dirección de un breakpoint, el corte por
/// count gana la carrera de hooks (el de breakpoints ni corre) y el tramo
/// devuelve Paused con el pc sobre el bp TODAVÍA NO reportado. El tramo
/// siguiente de la MISMA orden Run no debe suprimir ese breakpoint:
/// suprimirlo es semántica de «reanudar tras frenar EN el bp», y acá
/// nadie frenó ahí. Antes del fix, la continuación lo pasaba de largo y
/// el programa corría hasta el final sin reportarlo.
#[test]
fn breakpoint_en_borde_de_count_se_reporta_en_la_continuacion() {
    let limite = Duration::from_secs(if cfg!(debug_assertions) { 10 } else { 2 });
    let (tramo, pc, cont, t0) = con_guardia("bp en borde de count", limite, || {
        // Programa de 4 instrucciones + salida; breakpoint en la 4ª
        // (TEXT_BASE + 12). El tramo de 3 instrucciones agota el count
        // exactamente en esa dirección.
        let (mut m, _out, _inp) = machine_for("nop\nnop\nnop\nli t0, 7\nli a7, 10\necall");
        let bp = TEXT_BASE + 12;
        m.set_breakpoints([bp].into()).expect("breakpoints");
        let tramo = m.run_slice(0, 3);
        let pc = m.pc().expect("leer pc");
        // Continuación de la MISMA orden Run (no es reanudación del
        // usuario): el breakpoint del pc de arranque debe valer.
        let cont = m.run_slice_continuation(0, SLICE);
        (tramo, pc, cont, m.registers().expect("registros")[5])
    });
    assert_eq!(tramo, StopReason::Paused, "count agotado = tramo terminado");
    assert_eq!(pc, TEXT_BASE + 12, "el pc debe quedar sobre el breakpoint");
    assert_eq!(
        cont,
        StopReason::Breakpoint {
            addr: TEXT_BASE + 12
        },
        "la continuación debe reportar el breakpoint, no pasarlo de largo"
    );
    assert_eq!(t0, 0, "la instrucción del breakpoint NO debe ejecutarse");
}
