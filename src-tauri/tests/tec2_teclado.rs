//! TEC2 — Dispositivo de teclado MMIO estilo RARS/MARS.
//!
//! Mecanismo (decidido en tests/spike_mmio.rs): hook MEM_READ sobre la
//! página 0xFFFF_0000 que escribe el valor fresco en memoria ANTES de que
//! la lectura del guest complete; la página tiene respaldo real, así las
//! lecturas de host (visor de memoria) no consumen teclas. Semántica:
//!   * RECEIVER_CONTROL (0xFFFF_0000): bit 0 = hay tecla pendiente.
//!   * RECEIVER_DATA    (0xFFFF_0004): leerlo consume la tecla más vieja.
//!
//! Acá se valida la integración real (Machine + assemble + KeyboardShared),
//! no el unicorn crudo del spike.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::keyboard::KeyboardShared;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::{RECEIVER_CONTROL, RECEIVER_DATA};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Máquina lista + extremos de los canales de consola (patrón de
/// tests/tec1_detener.rs). Los extremos viven lo que dure el test.
fn machine_for(src: &str) -> (Machine<'static>, Receiver<ConsoleEvent>, Sender<InputMsg>) {
    let program = assemble(src).expect("el fuente de prueba debe ensamblar");
    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    let m = Machine::new(program, out_tx, in_rx).expect("máquina");
    (m, out_rx, in_tx)
}

/// Empuja `teclas` a la cola compartida de la máquina (lo que hará la UI
/// vía el comando `push_key`).
fn inyectar(m: &Machine<'_>, teclas: &[u8]) {
    let shared = m.keyboard_shared();
    let mut k = shared.lock().expect("lock del teclado");
    for &t in teclas {
        k.push(t);
    }
}

/// Programa determinista del flujo CONTROL→DATA→CONTROL con `lw`
/// (mismo patrón que el spike, ahora vía ensamblador real).
const FLUJO_LW: &str = "li t1, 0xFFFF0000
lw t0, 0(t1)
lw t2, 4(t1)
lw t3, 0(t1)
li a7, 10
ecall";

/// (a) Tecla inyectada → el guest lee CONTROL=1, DATA=tecla, CONTROL=0.
#[test]
fn tecla_inyectada_flujo_lw() {
    let (mut m, _out, _inp) = machine_for(FLUJO_LW);
    inyectar(&m, b"h");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    let r = m.registers().expect("registros");
    assert_eq!(r[5], 1, "1.ª lectura de CONTROL debía dar 1 (dato listo)");
    assert_eq!(r[7], u32::from(b'h'), "lectura de DATA debía dar 'h'");
    assert_eq!(r[28], 0, "2.ª lectura de CONTROL debía dar 0 (consumido)");
}

/// (b) Mismo flujo pero leyendo DATA con `lb`: el hook ve size=1 y la
/// semántica de consumo se mantiene (spike B3).
#[test]
fn tecla_inyectada_flujo_lb() {
    let src = "li t1, 0xFFFF0000
lw t0, 0(t1)
lb t2, 4(t1)
lw t3, 0(t1)
li a7, 10
ecall";
    let (mut m, _out, _inp) = machine_for(src);
    inyectar(&m, b"k");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    let r = m.registers().expect("registros");
    assert_eq!(r[5], 1, "CONTROL debía dar 1 con una tecla en cola");
    assert_eq!(r[7], u32::from(b'k'), "lb sobre DATA debía dar 'k'");
    assert_eq!(r[28], 0, "CONTROL debía dar 0 tras consumir con lb");
}

/// (c) Cola vacía: CONTROL=0 y DATA=0 (sin error ni bloqueo).
#[test]
fn cola_vacia_da_cero() {
    let (mut m, _out, _inp) = machine_for(FLUJO_LW);
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    let r = m.registers().expect("registros");
    assert_eq!(r[5], 0, "CONTROL debía dar 0 sin teclas");
    assert_eq!(r[7], 0, "DATA debía dar 0 sin teclas");
    assert_eq!(r[28], 0, "CONTROL debía seguir en 0");
}

/// (d) Overflow: 65 teclas en una cola de 64 → se descarta la MÁS VIEJA,
/// quedan exactamente 64 y no hay pánico.
#[test]
fn overflow_descarta_la_mas_vieja() {
    let mut k = KeyboardShared::default();
    for code in 1..=65u8 {
        k.push(code);
    }
    assert!(k.hay_dato(), "con 64 teclas debía haber dato");
    // La tecla 1 (la más vieja) se perdió: lo primero que sale es la 2.
    for esperado in 2..=65u8 {
        assert_eq!(
            k.pop(),
            Some(esperado),
            "la cola debía conservar las 64 más nuevas en orden"
        );
    }
    assert_eq!(k.pop(), None, "no debía quedar más que las 64 conservadas");
    assert!(!k.hay_dato());
}

/// (e) reset() limpia la cola Y los registros MMIO: sin teclas fantasma
/// entre reinicios.
#[test]
fn reset_limpia_cola_y_registros_mmio() {
    // Dos teclas, el programa consume una: tras correr, en memoria quedan
    // CONTROL=1 (queda 'b' pendiente) y DATA='a'.
    let (mut m, _out, _inp) = machine_for(FLUJO_LW);
    inyectar(&m, b"ab");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    let ctrl = m.read_mem(RECEIVER_CONTROL, 4).expect("leer CONTROL");
    assert_eq!(ctrl, 1u32.to_le_bytes(), "precondición: CONTROL quedó en 1");

    m.reset().expect("reset");

    // Los registros MMIO vuelven a 0 (lectura de host: no dispara el hook,
    // ve los bytes reales de la página).
    let ctrl = m.read_mem(RECEIVER_CONTROL, 4).expect("leer CONTROL");
    let data = m.read_mem(RECEIVER_DATA, 4).expect("leer DATA");
    assert_eq!(ctrl, 0u32.to_le_bytes(), "CONTROL debía quedar en 0");
    assert_eq!(data, 0u32.to_le_bytes(), "DATA debía quedar en 0");
    // Y la cola quedó vacía: la 'b' pendiente no sobrevive al reinicio.
    assert!(
        !m.keyboard_shared().lock().expect("lock").hay_dato(),
        "la cola debía vaciarse en el reset"
    );
    // Corrida nueva sin teclas: el guest también ve todo en 0.
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    let r = m.registers().expect("registros");
    assert_eq!(r[5], 0, "tras reset no debía haber tecla fantasma");
    assert_eq!(r[7], 0, "DATA debía dar 0 tras reset");
}

/// (f) Eco end-to-end: el programa pollea CONTROL, lee DATA, imprime con
/// syscall 11 y termina al recibir Esc (27). Con "ab\x1b" inyectado antes
/// de correr, la consola debe mostrar "ab" y salir con código 0.
#[test]
fn eco_end_to_end_hasta_esc() {
    let src = "li t1, 0xFFFF0000
poll:
lw t0, 0(t1)
beqz t0, poll
lw a0, 4(t1)
li t2, 27
beq a0, t2, fin
li a7, 11
ecall
j poll
fin:
li a7, 10
ecall";
    let (mut m, out, _inp) = machine_for(src);
    inyectar(&m, b"ab\x1b");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });

    let mut salida = String::new();
    while let Ok(ev) = out.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            salida.push_str(&s);
        }
    }
    assert_eq!(
        salida, "ab",
        "el eco debía imprimir las teclas previas a Esc"
    );
}
