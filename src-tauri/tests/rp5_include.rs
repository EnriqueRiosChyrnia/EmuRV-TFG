//! RP5 Fase 2 — `.include` con tabla de procedencia.
//!
//! El INVARIANTE bajo prueba: el source_map sigue hablando SOLO de líneas
//! del archivo principal (regla de atribución main_line). Los errores, en
//! cambio, reportan archivo:línea REALES.

use emurv_lib::core::assembler::{assemble, assemble_with_includes};
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

/// Carpeta temporal única por test, con los archivos dados ya escritos.
fn carpeta_con(nombre_test: &str, archivos: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rp5_inc_{nombre_test}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("crear carpeta temporal");
    for (nombre, contenido) in archivos {
        let ruta = dir.join(nombre);
        if let Some(padre) = ruta.parent() {
            std::fs::create_dir_all(padre).expect("crear subcarpeta");
        }
        std::fs::write(&ruta, contenido).expect("escribir archivo de prueba");
    }
    dir
}

#[test]
fn include_simple_simbolos_en_ambas_direcciones() {
    // El principal usa una etiqueta del include y el include una del
    // principal (las etiquetas se resuelven en pasada 2, sin orden).
    let dir = carpeta_con(
        "simple",
        &[("utiles.s", "doble:\n  add a0, a0, a0\n  j despues\n")],
    );
    let main = "
.text
  li a0, 21
  j doble
despues:
  li a7, 10
  ecall
.include \"utiles.s\"
";
    let p = assemble_with_includes(main, Some(&dir), "main.s").expect("ensambla");
    assert!(p.symbols.contains_key("doble"));
    assert!(p.symbols.contains_key("despues"));
    assert_eq!(p.files, vec!["main.s".to_string(), "utiles.s".to_string()]);
    // origins registra procedencia real: alguna palabra viene del archivo 1.
    assert!(p.origins.iter().any(|(_, f, _)| *f == 1));
}

#[test]
fn include_anidado_y_en_data() {
    let dir = carpeta_con(
        "anidado",
        &[
            ("a.s", ".include \"sub/b.s\"\nconst_a: .word 1\n"),
            ("sub/b.s", "const_b: .word 2\n"),
        ],
    );
    let main = "
.data
.include \"a.s\"
.text
  lw t0, const_a
  lw t1, const_b
  li a7, 10
  ecall
";
    let p = assemble_with_includes(main, Some(&dir), "main.s").expect("ensambla");
    assert!(p.symbols.contains_key("const_a"));
    assert!(p.symbols.contains_key("const_b"));
    assert_eq!(p.files.len(), 3);
}

#[test]
fn ciclo_de_includes_reporta_la_cadena() {
    let dir = carpeta_con(
        "ciclo",
        &[("a.s", ".include \"b.s\"\n"), ("b.s", ".include \"a.s\"\n")],
    );
    let errs = assemble_with_includes(".include \"a.s\"\n", Some(&dir), "main.s")
        .expect_err("debe fallar");
    let msg = &errs[0].msg;
    assert!(msg.contains("cíclica"), "{msg}");
    assert!(msg.contains("a.s → b.s → a.s"), "la cadena completa: {msg}");
}

#[test]
fn include_inexistente_y_sin_base_dir() {
    let dir = carpeta_con("faltante", &[]);
    let errs = assemble_with_includes(".include \"nada.s\"\nnop", Some(&dir), "main.s")
        .expect_err("debe fallar");
    assert!(errs[0].msg.contains("no se pudo leer"), "{}", errs[0].msg);
    assert!(errs[0].msg.contains("nada.s"), "{}", errs[0].msg);

    // Sin base_dir (buffer sin guardar / wrapper retrocompatible).
    let errs = assemble(".include \"x.s\"\nnop").expect_err("debe fallar");
    assert!(errs[0].msg.contains("guardado en disco"), "{}", errs[0].msg);
}

#[test]
fn error_dentro_del_include_reporta_archivo_y_linea_reales() {
    let dir = carpeta_con("error_real", &[("roto.s", "nop\n  addi x1, x2\n")]);
    let main = ".text\nnop\n.include \"roto.s\"\n";
    let errs = assemble_with_includes(main, Some(&dir), "main.s").expect_err("debe fallar");
    let e = &errs[0];
    assert_eq!(e.file.as_deref(), Some("roto.s"), "archivo real: {e:?}");
    assert_eq!(e.line, 2, "línea REAL dentro del include");
    assert!(e.to_string().starts_with("roto.s: "), "{e}");
}

/// EL INVARIANTE (PLAN-RP5): un breakpoint sobre la línea del `.include`
/// detiene en la PRIMERA instrucción ejecutable traída por ese include —
/// consecuencia directa de la regla main_line. Test a nivel Machine
/// (patrón machine_for de tec1).
#[test]
fn breakpoint_en_la_linea_del_include_frena_en_su_primera_instruccion() {
    let dir = carpeta_con("bp_invariante", &[("inc.s", "  li t1, 7\n  li t2, 8\n")]);
    // Línea 1: vacía (el fuente arranca con \n) — .include queda en línea 4.
    let main = "
.text
  li t0, 1
.include \"inc.s\"
  li a7, 10
  ecall
";
    let p = assemble_with_includes(main, Some(&dir), "main.s").expect("ensambla");
    let linea_include = 4u32;
    let addr_bp = p
        .source_map
        .addr_for_line(linea_include)
        .expect("la línea del .include debe tener dirección (primera instr del include)");

    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let (out_tx, _out_rx) = channel::<ConsoleEvent>();
        let (_in_tx, in_rx) = channel::<InputMsg>();
        let mut m = Machine::new(p, out_tx, in_rx).expect("máquina");
        let mut bps = HashSet::new();
        bps.insert(addr_bp);
        m.set_breakpoints(bps).expect("instalar bp");
        let razon = m.run_slice(0, 1_000);
        let regs = m.registers().expect("registros");
        let _ = tx.send((razon, regs[5], regs[6]));
    });
    let (razon, t0, t1) = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("la máquina no retornó");
    assert_eq!(
        razon,
        StopReason::Breakpoint { addr: addr_bp },
        "debe frenar exactamente en la primera instrucción del include"
    );
    assert_eq!(t0, 1, "lo previo al include se ejecutó");
    assert_eq!(
        t1, 0,
        "la primera instrucción del include NO se ejecutó aún"
    );
}
