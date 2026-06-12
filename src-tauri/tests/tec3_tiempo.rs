//! TEC3 — Syscalls de tiempo compatibles RARS: 30 (tiempo del sistema en
//! ms, bits bajos en a0 y altos en a1) y 32 (dormir a0 ms, interrumpible).
//!
//! La syscall 30 usa un reloj ANCLADO: época Unix capturada una vez +
//! transcurrido monotónico (`Instant`), así el valor nunca decrece dentro
//! de una sesión aunque el reloj de pared salte (NTP). La syscall 32 duerme
//! en trozos de ≤10 ms mirando `stop_request` entre trozos, y publica el
//! frame de video ANTES de dormir (mismo patrón que la syscall 5): una
//! animación que duerme entre cuadros se ve cuadro a cuadro.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::video::VideoShared;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Máquina lista + extremos de los canales de consola (patrón de
/// tests/tec1_detener.rs). Los extremos viven lo que dure el test.
fn machine_for(src: &str) -> (Machine<'static>, Receiver<ConsoleEvent>, Sender<InputMsg>) {
    let program = assemble(src).expect("el fuente de prueba debe ensamblar");
    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    let m = Machine::new(program, out_tx, in_rx).expect("máquina");
    (m, out_rx, in_tx)
}

/// Guardián anticuelgue (patrón de tests/tec1_detener.rs): corre `f` en un
/// hilo aparte y espera el resultado a lo sumo `limite`; si la ejecución se
/// colgara (p. ej. un sleep que ignora el stop), el test FALLA con mensaje
/// claro en vez de colgar la suite. La máquina se construye adentro del
/// hilo porque Unicorn no es Send.
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
        Err(_) => panic!("{nombre}: la ejecución no retornó en {limite:?}"),
    }
}

/// Reconstruye el valor de 64 bits que la syscall 30 reparte en dos
/// registros de 32 (bajos en a0, altos en a1, convención RARS en RV32).
fn par_a_u64(altos: u32, bajos: u32) -> u64 {
    (u64::from(altos) << 32) | u64::from(bajos)
}

/// (a) La syscall 30 devuelve milisegundos de época plausibles (posteriores
/// a 2023) y NO decrecientes entre dos llamadas seguidas. Como cada valor
/// es de 64 bits, el programa expone los 4 medios-valores en s0..s3 y la
/// comparación del par a1:a0 se hace en el host.
#[test]
fn syscall_30_da_ms_plausibles_y_no_decrecientes() {
    let src = "li a7, 30
ecall
mv s0, a0
mv s1, a1
li a7, 30
ecall
mv s2, a0
mv s3, a1
li a7, 10
ecall";
    let (reason, regs) = con_guardia("syscall 30", Duration::from_secs(10), move || {
        let (mut m, _out, _inp) = machine_for(src);
        let reason = m.run(0);
        (reason, m.registers().expect("registros"))
    });
    assert_eq!(reason, StopReason::Exited { code: 0 });
    // s0 = x8, s1 = x9, s2 = x18, s3 = x19.
    let t1 = par_a_u64(regs[9], regs[8]);
    let t2 = par_a_u64(regs[19], regs[18]);
    assert!(
        t1 > 1_700_000_000_000,
        "el tiempo debe ser posterior a 2023 (≈1.7e12 ms), dio {t1}"
    );
    // Cota superior laxa (~año 2286) para cazar basura en los bits altos.
    assert!(t1 < 10_000_000_000_000, "tiempo implausible: {t1}");
    assert!(
        t2 >= t1,
        "dos llamadas seguidas no pueden decrecer: t1={t1}, t2={t2}"
    );
}

/// (b) La syscall 32 duerme de verdad: tiempo(30) → dormir 80 ms (32) →
/// tiempo(30) debe dar un delta ≥ 60 ms (margen para el scheduler) y
/// acotado por arriba (no quedó dormida de más).
#[test]
fn syscall_32_duerme_el_tiempo_pedido() {
    let src = "li a7, 30
ecall
mv s0, a0
mv s1, a1
li a0, 80
li a7, 32
ecall
li a7, 30
ecall
mv s2, a0
mv s3, a1
li a7, 10
ecall";
    let (reason, regs) = con_guardia("syscall 32 duerme", Duration::from_secs(15), move || {
        let (mut m, _out, _inp) = machine_for(src);
        let reason = m.run(0);
        (reason, m.registers().expect("registros"))
    });
    assert_eq!(reason, StopReason::Exited { code: 0 });
    let t1 = par_a_u64(regs[9], regs[8]);
    let t2 = par_a_u64(regs[19], regs[18]);
    assert!(t2 >= t1, "el reloj anclado no puede decrecer");
    let delta = t2 - t1;
    assert!(
        delta >= 60,
        "dormir 80 ms debía reflejarse en el reloj (delta = {delta} ms)"
    );
    assert!(delta < 5000, "el sleep durmió de más (delta = {delta} ms)");
}

/// (c) El sleep es interrumpible: con un programa que duerme 10 000 ms, el
/// host setea `stop_request` a los ~50 ms desde otro hilo y `run()` debe
/// retornar enseguida como pausa de usuario (Paused, NO RuntimeError),
/// igual que la rama Cancel de la syscall 5.
#[test]
fn syscall_32_interrumpible_por_stop() {
    let src = "li a0, 10000
li a7, 32
ecall
li a7, 10
ecall";
    let (reason, ms) = con_guardia("sleep interrumpible", Duration::from_secs(15), move || {
        let (mut m, _out, _inp) = machine_for(src);
        let stop = Arc::new(AtomicBool::new(false));
        m.set_stop_request(Arc::clone(&stop));
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            stop.store(true, Ordering::SeqCst);
        });
        let t0 = Instant::now();
        let reason = m.run(0);
        let ms = t0.elapsed().as_millis();
        let _ = stopper.join();
        (reason, ms)
    });
    assert_eq!(
        reason,
        StopReason::Paused,
        "stop durante el sleep = pausa de usuario, no error"
    );
    assert!(
        ms < 1000,
        "run() debía retornar enseguida tras el stop, tardó {ms} ms"
    );
    assert!(
        ms >= 30,
        "el programa debía estar realmente dormido cuando llegó el stop ({ms} ms)"
    );
}

/// (d) Animación durante el sleep: el publish PRE-sleep es el mecanismo que
/// hace visible cada cuadro. Un bucle de 3 vueltas que pinta el píxel 0 con
/// un color distinto, marca el control de video y duerme 30 ms debe dejar
/// el contador de publicaciones (seq del VideoShared) en ≥ 2.
#[test]
fn syscall_32_publica_frames_durante_la_animacion() {
    let src = "li s0, 0x90000000
li s1, 0x90030000
li t0, 1
sw t0, 0(s1)
li s2, 12
li s3, 3
bucle:
sb s2, 0(s0)
li a0, 30
li a7, 32
ecall
addi s2, s2, 1
addi s3, s3, -1
bnez s3, bucle
li a7, 10
ecall";
    let shared: Arc<Mutex<VideoShared>> = Arc::default();
    let compartido = Arc::clone(&shared);
    let reason = con_guardia("animación con sleep", Duration::from_secs(15), move || {
        let program = assemble(src).expect("el fuente de prueba debe ensamblar");
        let (out_tx, _out_rx) = channel();
        let (_in_tx, in_rx) = channel();
        let mut m = Machine::new_with_video(program, out_tx, in_rx, compartido).expect("máquina");
        m.run(0)
    });
    assert_eq!(reason, StopReason::Exited { code: 0 });
    let v = shared.lock().expect("lock de video");
    assert!(v.in_use, "el programa marcó el control de video");
    assert!(
        v.seq >= 2,
        "cada vuelta publica su cuadro antes de dormir: seq debía avanzar ≥2, dio {}",
        v.seq
    );
}
