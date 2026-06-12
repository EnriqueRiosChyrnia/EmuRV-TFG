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
