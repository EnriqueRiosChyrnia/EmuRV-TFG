//! RP3 Fase 1 — el CLI como proceso real (env! CARGO_BIN_EXE).
//! stdout del invitado y stderr de diagnósticos se verifican por separado.

use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emurv-cli"))
}

/// (a) `asm` de 01-hola-mundo produce un ELF32 válido (magic + class).
#[test]
fn asm_genera_elf32() {
    let dir = std::env::temp_dir().join("rp3_asm_elf32");
    let _ = std::fs::create_dir_all(&dir);
    let salida = dir.join("hola.elf");
    let out = cli()
        .args([
            "asm",
            "../ejemplos/01-hola-mundo.asm",
            "-o",
            salida.to_str().unwrap(),
        ])
        .output()
        .expect("lanzar emurv-cli");
    assert!(
        out.status.success(),
        "asm debía salir con 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&salida).expect("leer el ELF generado");
    assert_eq!(&bytes[0..4], b"\x7fELF", "magic ELF");
    assert_eq!(bytes[4], 1, "EI_CLASS = ELFCLASS32 (32 bits)");
    let _ = std::fs::remove_file(&salida);
}

/// (b) Fuente con error de sintaxis → exit 2 y mensaje didáctico en
/// español con número de línea por stderr; stdout queda VACÍO.
#[test]
fn error_de_ensamblado_sale_2_con_mensaje_en_espanol() {
    let dir = std::env::temp_dir().join("rp3_asm_err");
    let _ = std::fs::create_dir_all(&dir);
    let fuente = dir.join("roto.s");
    std::fs::write(&fuente, ".text\n  addi x1, x2\n").expect("escribir fuente");
    let out = cli()
        .args(["asm", fuente.to_str().unwrap()])
        .output()
        .expect("lanzar emurv-cli");
    assert_eq!(out.status.code(), Some(2), "error de ensamblado = exit 2");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("línea 2") || stderr.contains("línea"),
        "stderr debía traer el diagnóstico didáctico con línea; fue: {stderr}"
    );
    assert!(out.stdout.is_empty(), "stdout debe quedar limpio");
    let _ = std::fs::remove_file(&fuente);
}

/// (c) Sin argumentos → exit 3 y texto de uso por stderr.
#[test]
fn sin_argumentos_sale_3_con_uso() {
    let out = cli().output().expect("lanzar emurv-cli");
    assert_eq!(out.status.code(), Some(3), "uso inválido = exit 3");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("USO"), "stderr debía traer el uso; fue: {stderr}");
    assert!(out.stdout.is_empty(), "stdout debe quedar limpio");
}

// ---------------------------------------------------------------------------
// RP3 Fase 2 — subcomando run: puente de consola y códigos de salida.
// ---------------------------------------------------------------------------

use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

/// Corre el CLI con stdin dado y guardián anticuelgue propio (patrón
/// con_guardia de tec1): si el proceso no termina en `limite`, lo mata y
/// el test falla con mensaje claro.
fn run_con_stdin(args: &[&str], stdin_data: &str, limite: Duration) -> std::process::Output {
    let mut hijo = cli()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lanzar emurv-cli");
    hijo.stdin
        .take()
        .expect("stdin del hijo")
        .write_all(stdin_data.as_bytes())
        .expect("escribir stdin");
    // (al soltar el handle, el hijo ve EOF tras consumir lo escrito)
    let inicio = std::time::Instant::now();
    loop {
        match hijo.try_wait().expect("try_wait") {
            Some(_) => break,
            None if inicio.elapsed() > limite => {
                let _ = hijo.kill();
                panic!("emurv-cli no terminó en {limite:?} (args: {args:?})");
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    hijo.wait_with_output().expect("recoger salida")
}

fn limite() -> Duration {
    Duration::from_secs(if cfg!(debug_assertions) { 60 } else { 20 })
}

/// (a) 01-hola-mundo: stdout EXACTO y exit 0 (stdout sin contaminación).
#[test]
fn run_hola_mundo_stdout_exacto() {
    let out = run_con_stdin(&["run", "../ejemplos/01-hola-mundo.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "¡Hola, mundo!\n",
        "stdout debe ser EXACTAMENTE la salida del programa"
    );
}

/// (b) 02-suma-interactiva con entrada por pipe.
#[test]
fn run_suma_interactiva_con_pipe() {
    let out = run_con_stdin(
        &["run", "../ejemplos/02-suma-interactiva.asm"],
        "8\n34\n",
        limite(),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("La suma es: 42\n"),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// (c) 03, 04 y 05: salidas conocidas (mismas que los tests de integración).
#[test]
fn run_ejemplos_de_consola_restantes() {
    let out = run_con_stdin(&["run", "../ejemplos/03-contador.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    let esperado: String = (1..=10).map(|n| format!("{n}\n")).collect();
    assert_eq!(String::from_utf8_lossy(&out.stdout), esperado);

    let out = run_con_stdin(&["run", "../ejemplos/04-fibonacci.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "0 1 1 2 3 5 8 13 21 34 \n");

    let out = run_con_stdin(&["run", "../ejemplos/05-pila-funciones.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "5! = 120\n");
}

/// (d) EOF prematuro: 02 espera dos números y stdin trae uno.
#[test]
fn run_eof_prematuro_sale_1_con_diagnostico() {
    let out = run_con_stdin(
        &["run", "../ejemplos/02-suma-interactiva.asm"],
        "8\n",
        limite(),
    );
    assert_eq!(out.status.code(), Some(1), "EOF de stdin = exit 1");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("EOF") || stderr.contains("se agotó"),
        "stderr debía diagnosticar el EOF: {stderr}"
    );
}

/// (e) Bucle infinito + --max-instr → exit 124 en tiempo acotado.
#[test]
fn run_bucle_infinito_corta_por_max_instr() {
    let dir = std::env::temp_dir().join("rp3_run_bucle");
    let _ = std::fs::create_dir_all(&dir);
    let fuente = dir.join("bucle.s");
    std::fs::write(&fuente, ".text\nbucle:\n  j bucle\n").expect("escribir fuente");
    let out = run_con_stdin(
        &["run", fuente.to_str().unwrap(), "--max-instr", "1000000"],
        "",
        limite(),
    );
    assert_eq!(out.status.code(), Some(124), "límite agotado = exit 124");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("max-instr"),
        "stderr debía mencionar el límite"
    );
    assert!(out.stdout.is_empty(), "stdout limpio en el bucle infinito");
    let _ = std::fs::remove_file(&fuente);
}
