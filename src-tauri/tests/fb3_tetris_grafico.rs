/// Test FB3: Tetris gráfico
/// Verifica que el ejemplo 08-tetris-grafico ejecuta y dibuja rectángulos.

use emurv_lib::core::{
    assembler::assemble,
    machine::{Machine, StopReason},
    syscalls::ConsoleEvent,
};
use std::sync::mpsc::channel;

#[test]
fn fb3_tetris_grafico_ejecuta() {
    let src = std::fs::read_to_string("../ejemplos/08-tetris-grafico.asm")
        .expect("leer ejemplo 08");

    let program = assemble(&src).expect("ensamblar ejemplo");
    let (out_tx, out_rx) = channel();
    let (_, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");

    let reason = m.run(0);

    assert!(
        matches!(reason, StopReason::Exited { code: 0 }),
        "programa debe terminar con código 0, obtuvo {:?}",
        reason
    );

    let mut _output = String::new();
    while let Ok(ev) = out_rx.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            _output.push_str(&s);
        }
    }

    // Verificar framebuffer
    let fb = m.read_framebuffer_rgba().expect("leer FB");
    assert_eq!(fb.len(), 320 * 240 * 4, "tamaño correcto");

    // Verificar que hay píxeles pintados (rojos y verdes)
    let non_black = fb.chunks(4).filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0).count();
    assert!(
        non_black > 1500,
        "debe haber píxeles pintados, obtuvo {}",
        non_black
    );
}

#[test]
fn fb3_tetris_rectangulos_detectados() {
    let src = std::fs::read_to_string("../ejemplos/08-tetris-grafico.asm").expect("leer");
    let program = assemble(&src).expect("ensamblar");

    let (out_tx, _) = channel();
    let (_, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    let _ = m.run(0);

    let fb = m.read_framebuffer_rgba().expect("leer FB");

    // Verificar presencia de rojo (255, 0, 0) en la zona esperada
    // Rectángulo rojo: fila 100-120, columnas 50-100
    let pixel_red = 100 * 320 * 4 + 50 * 4;
    let is_red = fb[pixel_red] > 100 && fb[pixel_red + 1] < 50 && fb[pixel_red + 2] < 50;
    assert!(is_red, "debe haber píxeles rojos en el rectángulo");

    // Verificar presencia de verde (0, 255, 0) en la otra zona
    // Rectángulo verde: fila 130-150, columnas 150-200
    let pixel_green = 130 * 320 * 4 + 150 * 4;
    let is_green = fb[pixel_green] < 50 && fb[pixel_green + 1] > 100 && fb[pixel_green + 2] < 50;
    assert!(is_green, "debe haber píxeles verdes en el segundo rectángulo");
}
