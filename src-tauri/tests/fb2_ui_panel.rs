/// Test FB2: panel gráfico en la UI
/// Verifica que el ejemplo 07-franjas ejecuta sin errores y renderiza
/// correctamente, y que la publicación de frames hacia la UI (estado
/// compartido + flag sucio, RF1/RF2/RF4) se comporta como el plan exige.
use emurv_lib::core::{
    assembler::assemble,
    machine::{Machine, StopReason},
    syscalls::ConsoleEvent,
    video::VideoShared,
};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

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

#[test]
fn fb2_publicacion_con_flag_sucio() {
    let src = std::fs::read_to_string("../ejemplos/07-franjas.asm").expect("leer");
    let program = assemble(&src).expect("ensamblar");

    let shared: Arc<Mutex<VideoShared>> = Arc::default();
    let (out_tx, _) = channel();
    let (_, in_rx) = channel();
    let mut m =
        Machine::new_with_video(program, out_tx, in_rx, Arc::clone(&shared)).expect("máquina");

    // Antes de correr: la VRAM está vacía, publicar no hace nada (RF4).
    m.publish_frame();
    assert!(!shared.lock().unwrap().in_use, "sin uso de VRAM no se publica");
    assert!(shared.lock().unwrap().rgba.is_none());

    // Tras correr el programa gráfico: hay frame y uso detectado.
    let _ = m.run(0);
    m.publish_frame();
    {
        let mut v = shared.lock().unwrap();
        assert!(v.in_use, "el programa escribió VRAM");
        let frame = v.rgba.take().expect("debe haber frame publicado");
        assert_eq!(frame.len(), 320 * 240 * 4);
    }

    // Sin cambios en la VRAM: no se vuelve a publicar (flag sucio, RF2).
    m.publish_frame();
    assert!(
        shared.lock().unwrap().rgba.is_none(),
        "sin cambios no debe publicarse otro frame"
    );
}

#[test]
fn fb2_publica_antes_de_bloquear_por_entrada() {
    // Programa que dibuja un píxel y DESPUÉS pide entrada: el frame debe
    // estar publicado mientras el hilo está bloqueado esperando la línea
    // (es el caso del Tetris por turnos: ver tablero antes de teclear).
    let src = r#"
.text
  li t0, 0x9000_0000
  li t1, 14          # amarillo
  sb t1, 0(t0)
  li a7, 5           # leer entero (bloquea)
  ecall
  li a7, 10
  ecall
"#;
    let program = assemble(src).expect("ensamblar");
    let shared: Arc<Mutex<VideoShared>> = Arc::default();
    let (out_tx, _out_rx) = channel();
    let (in_tx, in_rx) = channel();
    // La línea ya está encolada: el run no se bloquea de verdad, pero el
    // orden interno es el mismo (publicar → pedir entrada → leer).
    in_tx.send(emurv_lib::core::syscalls::InputMsg::Line("7".into())).unwrap();
    let mut m =
        Machine::new_with_video(program, out_tx, in_rx, Arc::clone(&shared)).expect("máquina");
    let _ = m.run(0);

    let v = shared.lock().unwrap();
    assert!(v.in_use, "el hook de entrada debe haber publicado el frame");
    let frame = v.rgba.as_ref().expect("frame publicado antes de bloquear");
    // Píxel 0 = índice 14 = amarillo CGA (0xFF, 0xFF, 0x55).
    assert_eq!(&frame[0..3], &[0xFF, 0xFF, 0x55]);
}

#[test]
fn fb2_reset_limpia_la_pantalla() {
    // Tras Reiniciar, el panel debe mostrar la VRAM limpia (frame negro),
    // no el último frame de la corrida anterior.
    let src = std::fs::read_to_string("../ejemplos/07-franjas.asm").expect("leer");
    let program = assemble(&src).expect("ensamblar");
    let shared: Arc<Mutex<VideoShared>> = Arc::default();
    let (out_tx, _) = channel();
    let (_, in_rx) = channel();
    let mut m =
        Machine::new_with_video(program, out_tx, in_rx, Arc::clone(&shared)).expect("máquina");

    let _ = m.run(0);
    m.publish_frame();
    let frame = shared.lock().unwrap().rgba.take().expect("frame con franjas");
    assert_ne!(&frame[0..3], &[0, 0, 0], "el píxel 0 estaba pintado (azul)");

    m.reset().expect("reset");
    let v = shared.lock().unwrap();
    let negro = v.rgba.as_ref().expect("reset debe publicar frame limpio");
    assert_eq!(negro.len(), 320 * 240 * 4);
    assert_eq!(&negro[0..4], &[0, 0, 0, 255], "píxel 0 negro tras reset");
}
