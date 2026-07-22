//! RT-3 — contrato del runner RISCOF (`src/bin/riscof_runner.rs`).
//!
//! Fixture: tests/fixtures/rt3/sig_fixture.elf (fuente y Makefile al lado)
//! escribe dos palabras conocidas en la región de signature y termina en
//! `ebreak`. El test ejecuta el binario real (CARGO_BIN_EXE_) y verifica el
//! volcado — el mismo contrato que RISCOF consume.

use std::path::Path;
use std::process::Command;

#[test]
fn runner_vuelca_signature_conocida() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rt3/sig_fixture.elf");
    let salida = std::env::temp_dir().join("rt3_sig_fixture.out");

    let st = Command::new(env!("CARGO_BIN_EXE_riscof_runner"))
        .arg(&fixture)
        .arg("--signature")
        .arg(&salida)
        .output()
        .expect("ejecutar riscof_runner");
    assert!(
        st.status.success(),
        "runner falló: {}",
        String::from_utf8_lossy(&st.stderr)
    );

    let sig = std::fs::read_to_string(&salida).expect("leer signature");
    assert_eq!(sig, "deadbeef\ncafe1234\n");
    let _ = std::fs::remove_file(&salida);
}

#[test]
fn runner_rechaza_elf_sin_signature() {
    // Un ELF de RT-2 (sin símbolos de signature) debe dar error claro.
    let elf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/riscv-tests/rv32ui/addi.elf");
    let salida = std::env::temp_dir().join("rt3_sig_inexistente.out");

    let st = Command::new(env!("CARGO_BIN_EXE_riscof_runner"))
        .arg(&elf)
        .arg("--signature")
        .arg(&salida)
        .output()
        .expect("ejecutar riscof_runner");
    assert!(!st.status.success());
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(
        err.contains("símbolos de signature"),
        "error inesperado: {err}"
    );
}
