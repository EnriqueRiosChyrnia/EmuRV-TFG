//! RP9 — las lecciones corren por CLI con salida EXACTA: la CI las vigila
//! para siempre (si un cambio futuro rompe una lección, esto lo caza).

use std::process::Command;

fn correr_leccion(ruta: &str, stdin: &str) -> (Option<i32>, String) {
    use std::io::Write;
    use std::time::{Duration, Instant};
    let mut hijo = Command::new(env!("CARGO_BIN_EXE_emurv-cli"))
        .args(["run", ruta])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("lanzar emurv-cli");
    hijo.stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .expect("stdin");
    let limite = Duration::from_secs(if cfg!(debug_assertions) { 60 } else { 20 });
    let inicio = Instant::now();
    loop {
        match hijo.try_wait().expect("try_wait") {
            Some(_) => break,
            None if inicio.elapsed() > limite => {
                let _ = hijo.kill();
                panic!("{ruta}: no terminó en {limite:?}");
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let out = hijo.wait_with_output().expect("output");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn leccion_01_registros() {
    let (code, out) = correr_leccion("../lecciones/01-registros/leccion.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(out, "40\nregistros listos\n");
}

#[test]
fn leccion_02_aritmetica() {
    let (code, out) = correr_leccion("../lecciones/02-aritmetica/leccion.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(out, "3 resto 2\n-2147483648\n48 252 204\n-4\n");
}

#[test]
fn leccion_03_memoria() {
    let (code, out) = correr_leccion("../lecciones/03-memoria/leccion.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(out, "68 17\n42\nhola\n");
}
