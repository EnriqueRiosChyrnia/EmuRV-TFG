//! Tests de integración (RNF-06): uno por cada ejemplo de RF-09.
//! Cada test ensambla el ejemplo real de `ejemplos/`, lo ejecuta en la
//! máquina y verifica salida de consola y estado final de registros.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::SP_INIT;
use std::sync::mpsc::channel;

/// Ensambla y ejecuta un ejemplo completo, alimentando `inputs` a las
/// syscalls de lectura. Devuelve (salida de consola, registros, motivo).
fn run_program(src: &str, inputs: &[&str]) -> (String, [u64; 32], StopReason) {
    let program = assemble(src).unwrap_or_else(|errs| {
        panic!(
            "el ejemplo debe ensamblar sin errores; hubo:\n{}",
            errs.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    for line in inputs {
        in_tx.send(InputMsg::Line((*line).to_string())).unwrap();
    }
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    let reason = m.run(0);
    let mut output = String::new();
    while let Ok(ev) = out_rx.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            output.push_str(&s);
        }
    }
    (output, m.registers().expect("registros legibles"), reason)
}

#[test]
fn ejemplo_01_hola_mundo() {
    let (out, _regs, reason) = run_program(include_str!("../../ejemplos/01-hola-mundo.asm"), &[]);
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert_eq!(out, "¡Hola, mundo!\n");
}

#[test]
fn ejemplo_02_suma_interactiva() {
    let (out, regs, reason) = run_program(
        include_str!("../../ejemplos/02-suma-interactiva.asm"),
        &["8", "34"],
    );
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert!(out.contains("La suma es: 42\n"), "salida: {out}");
    assert_eq!(regs[8], 8); // s0: primer número
    assert_eq!(regs[9], 34); // s1: segundo número
}

#[test]
fn ejemplo_02_reintenta_entrada_invalida() {
    // La consola reintenta si el usuario no tipea un entero (RF-07).
    let (out, regs, reason) = run_program(
        include_str!("../../ejemplos/02-suma-interactiva.asm"),
        &["ocho", "8", "34"],
    );
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert!(out.contains("no es un entero válido"), "salida: {out}");
    assert!(out.contains("La suma es: 42\n"), "salida: {out}");
    assert_eq!(regs[8], 8); // la entrada inválida no se consumió como número
}

#[test]
fn ejemplo_03_contador() {
    let (out, regs, reason) = run_program(include_str!("../../ejemplos/03-contador.asm"), &[]);
    assert_eq!(reason, StopReason::Exited { code: 0 });
    let esperado: String = (1..=10).map(|n| format!("{n}\n")).collect();
    assert_eq!(out, esperado);
    assert_eq!(regs[5], 11); // t0 terminó pasado el límite
    assert_eq!(regs[6], 10); // t1: límite intacto
}

#[test]
fn ejemplo_04_fibonacci() {
    let (out, regs, reason) = run_program(include_str!("../../ejemplos/04-fibonacci.asm"), &[]);
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert_eq!(out, "0 1 1 2 3 5 8 13 21 34 \n");
    assert_eq!(regs[8], 55); // s0: F(10), el siguiente a imprimir
    assert_eq!(regs[9], 89); // s1: F(11)
    assert_eq!(regs[18], 0); // s2: contador agotado
}

#[test]
fn ejemplo_05_pila_funciones() {
    let (out, regs, reason) =
        run_program(include_str!("../../ejemplos/05-pila-funciones.asm"), &[]);
    assert_eq!(reason, StopReason::Exited { code: 0 });
    assert_eq!(out, "5! = 120\n"); // 120 impreso desde a0
    assert_eq!(regs[2], u64::from(SP_INIT)); // sp balanceado: la pila quedó limpia
}

/// RP31 — el ejemplo de sonido dispara tres tonos (acorde Do mayor) por el
/// dispositivo MMIO; quedan encolados en `SoundShared` para que el run loop los
/// emita como `emu://sonido`. `sound` arranca apagado (RF4).
#[test]
fn ejemplo_16_sonido() {
    let program = assemble(include_str!("../../ejemplos/16-sonido.asm"))
        .expect("el ejemplo de sonido debe ensamblar");
    let (out_tx, _out_rx) = channel();
    let (_in_tx, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    let sound = m.sound_shared();
    assert!(!sound.lock().unwrap().in_use, "arranca apagado (RF4)");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    let s = sound.lock().unwrap();
    assert!(s.in_use, "el ejemplo usa el dispositivo de sonido");
    let freqs: Vec<u32> = s.cola.iter().map(|e| e.freq).collect();
    assert_eq!(freqs, vec![262, 330, 392], "tres tonos: do-mi-sol");
}

/// RP31 — el ejemplo de sprites pinta un fondo azul y un sprite amarillo en el
/// centro; tras publicar el frame, el RGBA tiene el centro amarillo (overlay
/// compuesto) y el fondo azul.
#[test]
fn ejemplo_17_sprites() {
    let program = assemble(include_str!("../../ejemplos/17-sprites.asm"))
        .expect("el ejemplo de sprites debe ensamblar");
    let (out_tx, _out_rx) = channel();
    let (_in_tx, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    m.publish_frame();
    let video = m.video_shared();
    let v = video.lock().unwrap();
    let rgba = v.rgba.as_ref().expect("frame publicado con el sprite");
    // Centro (160,120): dentro del sprite (152..168 × 112..128) → amarillo (14).
    let centro = (120 * 320 + 160) * 4;
    assert_eq!(
        &rgba[centro..centro + 4],
        &[0xFF, 0xFF, 0x55, 0xFF],
        "el centro es el sprite amarillo"
    );
    // Esquina (0,0): fondo azul (color 1 = 0x0000AA).
    assert_eq!(&rgba[0..4], &[0x00, 0x00, 0xAA, 0xFF], "el fondo es azul");
}

/// RP31 — los ejemplos de sonido y sprites usan solo el repertorio base, así
/// que ensamblan tanto en RV32 como en RV64 (RP32 ya está integrado).
#[test]
fn ejemplos_de_dispositivos_ensamblan_en_rv32_y_rv64() {
    use emurv_lib::core::assembler::assemble_with_xlen;
    use emurv_lib::core::Xlen;
    for (nombre, src) in [
        ("16-sonido", include_str!("../../ejemplos/16-sonido.asm")),
        ("17-sprites", include_str!("../../ejemplos/17-sprites.asm")),
    ] {
        for xlen in [Xlen::Rv32, Xlen::Rv64] {
            assemble_with_xlen(src, xlen).unwrap_or_else(|errs| {
                panic!(
                    "{nombre} debe ensamblar en {xlen:?}: {}",
                    errs.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            });
        }
    }
}
