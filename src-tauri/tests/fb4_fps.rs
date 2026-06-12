/// Bench FB4: frames publicados por segundo del lado Rust, bajo carga.
///
/// El programa huésped repinta el framebuffer COMPLETO (76 800 bytes, byte a
/// byte, con color que cambia por pasada), marca el registro de control y
/// cierra cada frame con syscall 5 (leer entero): ese hook publica el frame
/// antes de bloquear — es el mismo camino de producción que verifica el test
/// `fb2_publica_antes_de_bloquear_por_entrada`. Con las N líneas de entrada
/// pre-encoladas, la corrida no se bloquea de verdad y los N frames salen a
/// velocidad máxima.
///
/// Mide el techo del lado Rust: emulación del repintado + detección de
/// cambios + conversión paleta→RGBA + copia al estado compartido. NO incluye
/// el puente Tauri ni el canvas (la UI consume a ~30 Hz; ver
/// MEMORIA-GRAFICA.md).
///
/// Nota de diseño: la primera versión cedía el tramo con el timeout de
/// `run(ms)` y luego con `ebreak`; ninguna de las dos vías funciona hoy
/// (el timeout de Unicorn no interrumpe el bucle apretado del repintado y
/// el camino del ebreak tiene un bug latente — registrados en BITACORA,
/// fuera del alcance de esta sesión).
///
/// Marcado #[ignore]: cifra solo significativa en release. Ejecutar con:
/// `cargo test --release --test fb4_fps -- --ignored --nocapture`
use emurv_lib::core::{
    assembler::assemble,
    machine::{Machine, StopReason},
    syscalls::InputMsg,
    video::VideoShared,
};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const FRAMES: u32 = 1000;

#[test]
#[ignore]
fn fb4_fps_bajo_carga() {
    // Repintado completo: bucle interno de 76 800 sb (3 instrucciones por
    // byte) + cierre de frame ≈ 230 000 instrucciones por frame.
    let src = format!(
        r#"
.text
  li   s1, {FRAMES}         # frames a pintar
inicio:
  li   t0, 0x9000_0000      # puntero al framebuffer
  li   t3, 0x9001_2C00      # fin (base + 76800)
  andi t4, s0, 0xFF         # color de esta pasada
bucle:
  sb   t4, 0(t0)
  addi t0, t0, 1
  bne  t0, t3, bucle
  addi s0, s0, 1            # color siguiente
  li   t5, 0x9003_0000
  li   t6, 1
  sw   t6, 0(t5)            # marcar uso de pantalla
  li   a7, 5
  ecall                     # fin de frame: publica y consume una línea
  addi s1, s1, -1
  bne  s1, zero, inicio
  li   a7, 10
  ecall
"#
    );
    let program = assemble(&src).expect("ensamblar el programa de repintado");
    let shared: Arc<Mutex<VideoShared>> = Arc::default();
    let (out_tx, _out_rx) = channel();
    let (in_tx, in_rx) = channel();
    for _ in 0..FRAMES {
        in_tx.send(InputMsg::Line("0".into())).expect("encolar entrada");
    }
    let mut m =
        Machine::new_with_video(program, out_tx, in_rx, Arc::clone(&shared)).expect("máquina");

    let seq_inicial = shared.lock().unwrap().seq;
    let inicio = Instant::now();
    let reason = m.run(0);
    let segundos = inicio.elapsed().as_secs_f64();

    assert!(
        matches!(reason, StopReason::Exited { code: 0 }),
        "el programa debe terminar bien, obtuvo {reason:?}"
    );
    let frames = shared.lock().unwrap().seq - seq_inicial;
    let fps = frames as f64 / segundos;
    eprintln!(
        "FB4 FPS bajo carga (lado Rust): {frames} frames publicados en {segundos:.3} s → {fps:.0} FPS \
         (repintado completo de 320×240 por frame, color distinto por pasada)"
    );

    assert_eq!(
        frames,
        u64::from(FRAMES),
        "cada frame repintado debe publicarse exactamente una vez"
    );
    // El requisito efectivo es alimentar la UI a 30 Hz; el lado Rust debe
    // superarlo con margen amplio.
    assert!(
        fps >= 30.0,
        "el lado Rust debe publicar ≥30 FPS bajo carga (obtuvo {fps:.0})"
    );
}
