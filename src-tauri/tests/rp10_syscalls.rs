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

// ---------------------------------------------------------------------------
// RP10 Fase 2 — ReadString (8) y Sbrk (9).
// ---------------------------------------------------------------------------

use emurv_lib::core::{DATA_BASE, HEAP_BASE};
use std::sync::mpsc::Sender;

/// Máquina + emisor de entrada (la 8 bloquea esperando una línea: el test
/// la encola ANTES de correr — canal con buffer).
fn machine_con_entrada(src: &str) -> (Machine<'static>, Sender<InputMsg>) {
    let program = assemble(src).expect("ensambla");
    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (in_tx, in_rx) = channel::<InputMsg>();
    (
        Machine::new(program, out_tx, in_rx).expect("máquina"),
        in_tx,
    )
}

/// Línea corta: queda completa + NUL; a0 no se toca.
#[test]
fn readstring_linea_corta() {
    let src = "
.data
buf: .word 0, 0, 0, 0
.text
  la a0, buf
  li a1, 16
  li a7, 8
  ecall
  li a7, 10
  ecall
";
    let (mut m, tx) = machine_con_entrada(src);
    tx.send(InputMsg::Line("hola".into())).expect("encolar");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let bytes = m.read_mem(DATA_BASE, 6).expect("leer buffer");
    assert_eq!(&bytes, b"hola\0\0", "cadena + NUL, el resto intacto");
}

/// Línea larga: trunca a a1−1 bytes + NUL, SIN error (semántica RARS).
#[test]
fn readstring_trunca_con_nul() {
    let src = "
.data
buf: .word 0, 0
.text
  la a0, buf
  li a1, 4
  li a7, 8
  ecall
  li a7, 10
  ecall
";
    let (mut m, tx) = machine_con_entrada(src);
    tx.send(InputMsg::Line("holamundo".into())).expect("encolar");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let bytes = m.read_mem(DATA_BASE, 5).expect("leer buffer");
    assert_eq!(&bytes, b"hol\0\0", "3 bytes + NUL; el 5.º quedó intacto");
}

/// a1 = 0: no escribe NADA (ni pide entrada — el test no encola línea).
#[test]
fn readstring_a1_cero_no_escribe() {
    let src = "
.data
buf: .word 0
.text
  la a0, buf
  li a1, 0
  li a7, 8
  ecall
  li a7, 10
  ecall
";
    let (mut m, _tx) = machine_con_entrada(src);
    m.write_mem(DATA_BASE, &[0xAA, 0xBB, 0xCC, 0xDD]).expect("centinela");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let bytes = m.read_mem(DATA_BASE, 4).expect("leer buffer");
    assert_eq!(bytes, vec![0xAA, 0xBB, 0xCC, 0xDD], "centinela intacto");
}

/// Buffer en .text: rechazo didáctico (mismas zonas que write_mem RP-7).
#[test]
fn readstring_buffer_en_text_rechazo() {
    let src = ".text\nmain:\n  la a0, main\n  li a1, 8\n  li a7, 8\n  ecall\n";
    let (mut m, _tx) = machine_con_entrada(src);
    match correr(&mut m) {
        StopReason::RuntimeError { message } => {
            assert!(message.contains("syscall 8"), "{message}");
            assert!(message.contains("inmutable"), "{message}");
        }
        otra => panic!("debía rechazar el buffer en .text: {otra:?}"),
    }
}

/// Reservas sucesivas: direcciones desde HEAP_BASE, próxima alineada a 4;
/// sbrk(0) devuelve el puntero actual sin avanzar.
#[test]
fn sbrk_reservas_sucesivas_alineadas() {
    let src = "
.text
  li a0, 5
  li a7, 9
  ecall
  mv s0, a0      # HEAP_BASE
  li a0, 3
  li a7, 9
  ecall
  mv s1, a0      # HEAP_BASE + 8 (5 → 8 por alineación)
  li a0, 0
  li a7, 9
  ecall
  mv s2, a0      # HEAP_BASE + 12 (3 → 4); el 0 no avanza
  li a7, 10
  ecall
";
    let mut m = machine_con_entrada(src).0;
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let regs = m.registers().expect("regs");
    // RP32: registers() es u64; HEAP_BASE es u32 (el mapa cabe en 32 bits).
    assert_eq!(regs[8], u64::from(HEAP_BASE), "primera reserva = HEAP_BASE");
    assert_eq!(regs[9], u64::from(HEAP_BASE) + 8, "5 bytes redondean a 8");
    assert_eq!(regs[18], u64::from(HEAP_BASE) + 12, "3 bytes redondean a 4");
}

/// Agotamiento: error didáctico con lo pedido y lo libre (apartamiento de
/// RARS registrado: nada de -1 silencioso).
#[test]
fn sbrk_agotamiento_error_didactico() {
    let src = ".text\n  li a0, 0x400000\n  li a7, 9\n  ecall\n";
    let mut m = machine_con_entrada(src).0;
    match correr(&mut m) {
        StopReason::RuntimeError { message } => {
            assert!(message.contains("se agotó"), "{message}");
            assert!(message.contains("4194304"), "lo pedido: {message}");
        }
        otra => panic!("debía agotar el heap: {otra:?}"),
    }
}

/// reset() devuelve el puntero a HEAP_BASE: la segunda corrida recibe las
/// mismas direcciones que la primera.
#[test]
fn sbrk_reset_limpia_el_puntero() {
    let src = ".text\n  li a0, 64\n  li a7, 9\n  ecall\n  mv s0, a0\n  li a7, 10\n  ecall\n";
    let mut m = machine_con_entrada(src).0;
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    assert_eq!(m.registers().expect("regs")[8], u64::from(HEAP_BASE));
    m.reset().expect("reset");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    assert_eq!(
        m.registers().expect("regs")[8],
        u64::from(HEAP_BASE),
        "tras reset, sbrk vuelve a repartir desde HEAP_BASE"
    );
}

/// .data que invade el heap: la CARGA falla con el error didáctico (no se
/// llega ni a correr). 0x30000 = 196608 bytes es justo el límite.
#[test]
fn data_que_pisa_el_heap_error_de_carga() {
    let src = ".data\n.space 200000\n.text\n  li a7, 10\n  ecall\n";
    let program = assemble(src).expect("el ensamblado en sí es válido");
    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    let e = match Machine::new(program, out_tx, in_rx) {
        Err(e) => e,
        Ok(_) => panic!("debía rechazar la carga"),
    };
    assert!(e.contains("pisan el heap"), "{e}");
    // Y el límite exacto pasa: 196608 bytes llegan JUSTO a 0x10040000.
    let src_ok = ".data\n.space 196608\n.text\n  li a7, 10\n  ecall\n";
    let program = assemble(src_ok).expect("ensambla");
    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    Machine::new(program, out_tx, in_rx).expect("196608 bytes caben exactos");
}

// ---------------------------------------------------------------------------
// RP10 Fase 3 — syscall 60: limpiar consola (extensión EmuRV).
// ---------------------------------------------------------------------------

use std::sync::mpsc::Receiver;

/// El 60 emite ConsoleEvent::Clear en el lugar correcto de la secuencia
/// (entre las dos salidas) y no frena la ejecución.
#[test]
fn syscall_60_emite_clear_en_secuencia() {
    let src = "
.text
  li a0, 65        # 'A'
  li a7, 11
  ecall
  li a7, 60        # limpiar
  ecall
  li a0, 66        # 'B'
  li a7, 11
  ecall
  li a7, 10
  ecall
";
    let program = assemble(src).expect("ensambla");
    let (out_tx, out_rx) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let eventos: Vec<ConsoleEvent> = drenar(&out_rx);
    assert_eq!(
        eventos,
        vec![
            ConsoleEvent::Output("A".into()),
            ConsoleEvent::Clear,
            ConsoleEvent::Output("B".into()),
        ],
        "Clear viaja en orden, entre las dos salidas"
    );
}

fn drenar(rx: &Receiver<ConsoleEvent>) -> Vec<ConsoleEvent> {
    let mut v = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        v.push(ev);
    }
    v
}
