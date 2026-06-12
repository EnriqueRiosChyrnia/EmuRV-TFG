/// Test FB1: núcleo del framebuffer
/// Verifica conversión índice→RGBA, comando read_framebuffer, rendimiento.
use emurv_lib::core::{
    assembler::assemble, machine::Machine, syscalls::ConsoleEvent, FRAMEBUFFER_BASE, PALETTE_BASE,
};
use std::sync::mpsc::channel;
use std::time::Instant;

fn run_program_and_read_fb(src: &str) -> (Vec<u8>, String) {
    let program = assemble(src).expect("programa debe ensamblar");
    let (out_tx, out_rx) = channel();
    let (_, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    let _ = m.run(0);
    let mut output = String::new();
    while let Ok(ev) = out_rx.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            output.push_str(&s);
        }
    }
    let fb = m.read_framebuffer_rgba().expect("leer FB");
    (fb, output)
}

#[test]
fn fb1_conversion_pixeles_conocidos() {
    let src = format!(
        r#"
.text
  # Escribir paleta: entrada 42 = color rojo puro (FF, 00, 00)
  li t0, {palette_base}
  li t1, {palette_offset}
  add t0, t0, t1
  li t2, 0xFF
  sb t2, 0(t0)    # R = 0xFF
  sb zero, 1(t0)  # G = 0x00
  sb zero, 2(t0)  # B = 0x00

  # Escribir framebuffer: píxel 0 = índice 42
  li t0, {fb_base}
  li t1, 42
  sb t1, 0(t0)

  # Retornar
  ecall
"#,
        fb_base = FRAMEBUFFER_BASE,
        palette_base = PALETTE_BASE,
        palette_offset = 42 * 3,
    );

    let (fb_rgba, _) = run_program_and_read_fb(&src);

    // Verificar píxel 0 (primeros 4 bytes: RGBA)
    assert_eq!(fb_rgba[0], 0xFF, "R debe ser 0xFF");
    assert_eq!(fb_rgba[1], 0x00, "G debe ser 0x00");
    assert_eq!(fb_rgba[2], 0x00, "B debe ser 0x00");
    assert_eq!(fb_rgba[3], 0xFF, "A debe ser 0xFF");
}

#[test]
fn fb1_tamaño_buffer_correcto() {
    let src = ".text\n  ecall";
    let (fb, _) = run_program_and_read_fb(src);
    assert_eq!(
        fb.len(),
        320 * 240 * 4,
        "buffer debe ser 307200 bytes (320×240×4)"
    );
}

/// Conversión del framebuffer a RGBA en menos de 3 ms de promedio.
///
/// Marcado #[ignore]: el umbral solo vale en release. Ejecutar con:
/// `cargo test --release --test fb1_framebuffer_core -- --ignored --nocapture`
#[test]
#[ignore]
fn fb1_performance_conversion() {
    let src = ".text\n  ecall";
    let mut duraciones = Vec::new();

    // Ejecutar 10 veces para medir promedio
    for _ in 0..10 {
        let program = assemble(src).expect("programa");
        let (out_tx, _out_rx) = channel();
        let (_, in_rx) = channel();
        let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
        let _ = m.run(0);

        let start = Instant::now();
        let _ = m.read_framebuffer_rgba().expect("leer FB");
        let elapsed = start.elapsed();
        duraciones.push(elapsed.as_millis());
    }

    let promedio = duraciones.iter().sum::<u128>() / duraciones.len() as u128;
    eprintln!("FB1 conversión: {} ms promedio", promedio);
    assert!(
        promedio <= 3,
        "conversión debe ser <= 3 ms en release (obtuvo {} ms)",
        promedio
    );
}
