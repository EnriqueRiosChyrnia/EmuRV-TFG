/// Test FB2: panel gráfico en la UI
/// Verifica que el ejemplo 07-franjas ejecuta sin errores y renderiza correctamente.

use emurv_lib::core::{
    assembler::assemble,
    machine::{Machine, StopReason},
    syscalls::ConsoleEvent,
};
use std::sync::mpsc::channel;

#[test]
fn fb2_ejemplo_franjas_ejecuta() {
    let src = std::fs::read_to_string("../ejemplos/07-franjas.asm")
        .expect("leer ejemplo 07-franjas.asm");

    let program = assemble(&src).unwrap_or_else(|errs| {
        panic!(
            "ejemplo 07-franjas debe ensamblar sin errores; hubo:\n{}",
            errs.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    let (out_tx, out_rx) = channel();
    let (_, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");

    let reason = m.run(0);

    // Verificar que el programa terminó correctamente
    assert!(
        matches!(reason, StopReason::Exited { code: 0 }),
        "programa debe terminar con código 0, obtuvo {:?}",
        reason
    );

    // Verificar que hay salida (opcional, pero el programa no imprime)
    let mut _output = String::new();
    while let Ok(ev) = out_rx.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            _output.push_str(&s);
        }
    }

    // Verificar que el framebuffer fue escrito
    let fb = m.read_framebuffer_rgba().expect("leer framebuffer");
    assert_eq!(fb.len(), 320 * 240 * 4, "framebuffer debe tener tamaño correcto");

    // Verificar que hay píxeles no negros (franjas pintadas)
    let non_black = fb.chunks(4).filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0).count();
    assert!(
        non_black >= 900,
        "debe haber píxeles pintados (coloreados), obtuvo {} píxeles",
        non_black
    );
}

#[test]
fn fb2_franjas_colores_correctos() {
    // Verificar que la primera franja (azul, índice 1) tiene píxeles azules
    let src = std::fs::read_to_string("../ejemplos/07-franjas.asm").expect("leer ejemplo");
    let program = assemble(&src).expect("ensamblar");

    let (out_tx, _) = channel();
    let (_, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    let _ = m.run(0);

    let fb = m.read_framebuffer_rgba().expect("leer FB");

    // Primeros píxeles (franja 0, azul oscuro según paleta CGA: índice 1)
    // Índice 1 = (0, 0, 170) en paleta VGA13h
    let pixel_0 = &fb[0..4];
    eprintln!("Píxel 0 (franja azul): R={} G={} B={}", pixel_0[0], pixel_0[1], pixel_0[2]);

    // Debe ser aproximadamente azul
    assert!(pixel_0[2] > 150, "canal B debe ser alto en píxel azul");
    assert!(pixel_0[0] < 100, "canal R debe ser bajo en píxel azul");
}
