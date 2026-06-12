//! Mediciones de los RNF de rendimiento (RNF-02 y RNF-03).
//!
//! Marcados #[ignore]: los números solo valen en release. Ejecutar con:
//! `cargo test --release --test rnf_rendimiento -- --ignored --nocapture`

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use std::sync::mpsc::channel;
use std::time::Instant;

/// RNF-02: ensamblar un programa de 500 líneas en menos de 200 ms.
#[test]
#[ignore]
fn rnf02_ensamblar_500_lineas() {
    // Programa sintético de 500 líneas: mezcla representativa de datos,
    // etiquetas, saltos y pseudo-instrucciones (expansiones incluidas).
    let mut src = String::from(".data\n");
    for i in 0..50 {
        src.push_str(&format!("dato{i}: .word {i}, {}\n", i * 7));
    }
    src.push_str(".text\nmain:\n");
    for i in 0..149 {
        src.push_str(&format!(
            "et{i}: addi t0, t0, 1\n    la t1, dato{}\n    bne t0, t1, et{i}\n",
            i % 50
        ));
    }
    src.push_str("    li a7, 10\n    ecall\n");
    let total_lineas = src.lines().count();
    assert!(
        total_lineas >= 500,
        "el programa sintético debe tener ≥500 líneas"
    );

    let inicio = Instant::now();
    let programa = assemble(&src).expect("debe ensamblar");
    let ms = inicio.elapsed().as_secs_f64() * 1000.0;
    println!(
        "RNF-02: {total_lineas} líneas → {} bytes de .text en {ms:.2} ms",
        programa.text.len()
    );
    assert!(ms < 200.0, "RNF-02 exige <200 ms; midió {ms:.2} ms");
}

/// RNF-02 con el preprocesador en el mix (RP5): mismas 500 líneas pero con
/// un `.include` (los datos viven en otro archivo) y una macro invocada en
/// el bucle. El gate sigue siendo <200 ms.
#[test]
#[ignore]
fn rnf02_ensamblar_500_lineas_con_include_y_macro() {
    use emurv_lib::core::assembler::assemble_with_includes;
    let dir = std::env::temp_dir().join("rnf02_rp5");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("carpeta temporal");
    let mut datos = String::from(".data\n");
    for i in 0..50 {
        datos.push_str(&format!("dato{i}: .word {i}, {}\n", i * 7));
    }
    std::fs::write(dir.join("datos.s"), datos).expect("escribir include");

    let mut src = String::from(
        ".include \"datos.s\"\n.macro paso (%rd, %et)\n  addi %rd, %rd, 1\n  la t1, %et\n.end_macro\n.text\nmain:\n",
    );
    for i in 0..230 {
        src.push_str(&format!(
            "et{i}: paso t0, dato{}\n    bne t0, t1, et{i}\n",
            i % 50
        ));
    }
    src.push_str("    li a7, 10\n    ecall\n");
    let total_lineas = src.lines().count() + 51; // + las del include
    assert!(total_lineas >= 500, "≥500 líneas contando el include");

    let inicio = Instant::now();
    let programa = assemble_with_includes(&src, Some(&dir), "main.s").expect("debe ensamblar");
    let ms = inicio.elapsed().as_secs_f64() * 1000.0;
    println!(
        "RNF-02 (include+macro): {total_lineas} líneas → {} bytes de .text en {ms:.2} ms",
        programa.text.len()
    );
    assert!(ms < 200.0, "RNF-02 exige <200 ms; midió {ms:.2} ms");
}

/// RNF-03 con WATCHPOINT activo (RP7): bucle con una carga de memoria por
/// iteración (el caso que los hooks MEM_READ/MEM_WRITE globales encarecen)
/// y un watchpoint vigilando OTRA dirección. Gate: ≥ 10 M instr/s.
#[test]
#[ignore]
fn rnf03_velocidad_con_watchpoint_activo() {
    use emurv_lib::core::machine::{WatchKind, Watchpoint};
    let src = "
.data
v: .word 1
otro: .word 0
.text
  li t1, 4000000
  la t2, v
bucle:
  lw t0, 0(t2)
  addi t1, t1, -1
  bnez t1, bucle
  li a7, 10
  ecall
";
    let program = assemble(src).expect("debe ensamblar");
    let (out_tx, _o) = channel();
    let (_, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    m.set_watchpoints(vec![Watchpoint {
        addr: emurv_lib::core::DATA_BASE + 4, // «otro»: jamás accedido
        len: 4,
        kind: WatchKind::Both,
    }])
    .expect("instalar watchpoint");
    let instrucciones = 4_000_000u64 * 3 + 4;
    let inicio = Instant::now();
    let reason = m.run(0);
    let segs = inicio.elapsed().as_secs_f64();
    assert_eq!(reason, StopReason::Exited { code: 0 });
    let mips = instrucciones as f64 / segs / 1e6;
    println!("RNF-03 (con watchpoint): {instrucciones} instr en {segs:.3} s → {mips:.1} M instr/s");
    assert!(mips >= 10.0, "RNF-03 exige ≥10 M instr/s; midió {mips:.1}");
}

/// RNF-03: ejecución continua ≥ 10 M instrucciones/segundo efectivas.
#[test]
#[ignore]
fn rnf03_velocidad_de_ejecucion() {
    // Bucle de 2 instrucciones × 20 M iteraciones = 40 M instrucciones.
    let src = "li t0, 20000000\nbucle:\naddi t0, t0, -1\nbnez t0, bucle\nli a7, 10\necall";
    let programa = assemble(src).expect("debe ensamblar");
    let (out_tx, _out_rx) = channel();
    let (_in_tx, in_rx) = channel();
    let mut m = Machine::new(programa, out_tx, in_rx).expect("máquina");

    let instrucciones: f64 = 2.0 * 20_000_000.0 + 4.0;
    let inicio = Instant::now();
    let r = m.run(0);
    let segs = inicio.elapsed().as_secs_f64();
    assert_eq!(r, StopReason::Exited { code: 0 });
    let mips = instrucciones / segs / 1.0e6;
    println!("RNF-03: {instrucciones:.0} instrucciones en {segs:.3} s → {mips:.1} M instr/s");
    assert!(mips >= 10.0, "RNF-03 exige ≥10 M instr/s; midió {mips:.1}");
}

/// RNF-03 (TEC1): la MISMA carga que `rnf03_velocidad_de_ejecucion`, pero
/// ejecutada como producción tras TEC1: tramos de RUN_SLICE_COUNT
/// instrucciones (`run_slice`) hasta terminar. Mide cuánto cuesta el
/// slicing que hace que «Detener» responda siempre.
#[test]
#[ignore]
fn rnf03_velocidad_con_slicing() {
    /// Mismo valor que RUN_SLICE_COUNT en commands.rs (~2,3 ms por tramo).
    const RUN_SLICE_COUNT: u64 = 1_000_000;
    let src = "li t0, 20000000\nbucle:\naddi t0, t0, -1\nbnez t0, bucle\nli a7, 10\necall";
    let programa = assemble(src).expect("debe ensamblar");
    let (out_tx, _out_rx) = channel();
    let (_in_tx, in_rx) = channel();
    let mut m = Machine::new(programa, out_tx, in_rx).expect("máquina");

    let instrucciones: f64 = 2.0 * 20_000_000.0 + 4.0;
    let inicio = Instant::now();
    let mut tramos = 0u64;
    let reason = loop {
        match m.run_slice(100, RUN_SLICE_COUNT) {
            StopReason::Paused => {
                tramos += 1;
                assert!(tramos < 1_000, "demasiados tramos: el programa no avanza");
            }
            other => break other,
        }
    };
    let segs = inicio.elapsed().as_secs_f64();
    assert_eq!(reason, StopReason::Exited { code: 0 });
    let mips = instrucciones / segs / 1.0e6;
    println!(
        "RNF-03 (slicing): {instrucciones:.0} instrucciones en {} tramos de \
         {RUN_SLICE_COUNT} → {segs:.3} s → {mips:.1} M instr/s",
        tramos + 1
    );
    assert!(mips >= 10.0, "RNF-03 exige ≥10 M instr/s; midió {mips:.1}");
}

/// RNF-03 (caso adverso): velocidad CON un breakpoint activo lejos del
/// bucle — mide el costo del hook de código.
#[test]
#[ignore]
fn rnf03_velocidad_con_breakpoint_activo() {
    let src = "li t0, 2000000\nbucle:\naddi t0, t0, -1\nbnez t0, bucle\nli a7, 10\necall";
    let programa = assemble(src).expect("debe ensamblar");
    let (out_tx, _out_rx) = channel();
    let (_in_tx, in_rx) = channel();
    let mut m = Machine::new(programa, out_tx, in_rx).expect("máquina");
    // breakpoint en el ecall final: el hook corre en cada instrucción
    m.set_breakpoints([0x0040_0010u32])
        .unwrap();

    let instrucciones: f64 = 2.0 * 2_000_000.0 + 3.0;
    let inicio = Instant::now();
    let r = m.run(0);
    let segs = inicio.elapsed().as_secs_f64();
    assert_eq!(r, StopReason::Breakpoint { addr: 0x0040_0010 });
    let mips = instrucciones / segs / 1.0e6;
    println!("RNF-03 (con bp): {instrucciones:.0} instr en {segs:.3} s → {mips:.1} M instr/s");
}
