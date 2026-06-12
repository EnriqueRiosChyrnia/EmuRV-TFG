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

#[test]
fn leccion_04_condicionales() {
    let (code, out) = correr_leccion("../lecciones/04-condicionales/leccion.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(
        out,
        "5 y 7 son distintos\nel máximo entre 12 y 33 es 33\n\
         blt (con signo): -1 es menor que 5\n\
         bltu (sin signo): -1 NO es menor que 5 (son 4294967295)\n"
    );
}

#[test]
fn leccion_05_bucles() {
    let (code, out) = correr_leccion("../lecciones/05-bucles/leccion.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(
        out,
        "suma 1..10 = 55\n3 x 1 = 3\n3 x 2 = 6\n3 x 3 = 9\n3 x 4 = 12\n3 x 5 = 15\n"
    );
}

#[test]
fn leccion_06_funciones() {
    let (code, out) = correr_leccion("../lecciones/06-funciones/leccion.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(out, "doble de 7 = 14\ncuádruple de 7 = 28\n");
}

/// Interactiva: 25 está fuera de rango (revalida), 7 es válido, «Ana» se
/// lee con el servicio 8 a un buffer del 9, y el 93 sale con código 0
/// (RP10 amplió la lección). Demuestra 30/32 sin imprimir la hora cruda.
#[test]
fn leccion_07_syscalls() {
    let (code, out) = correr_leccion("../lecciones/07-syscalls/leccion.s", "25\n7\nAna\n");
    assert_eq!(code, Some(0));
    assert_eq!(
        out,
        "Ingresá un número del 1 al 10: Fuera de rango, probá de nuevo.\n\
         Ingresá un número del 1 al 10: El doble es 14\n\
         el reloj avanzó: dormimos 100 ms y se notó\n\
         ¿Cómo te llamás? ¡Hola, Ana!\n"
    );
}

/// Multi-archivo: corre el main.s y los .include se resuelven al lado.
#[test]
fn leccion_08_multiarchivo() {
    let (code, out) = correr_leccion("../lecciones/08-multiarchivo/main.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(
        out,
        "precio de una entrada: 1500\nentradas compradas: 3\n\
         total a pagar: 4500\ngracias por su compra\n"
    );
}

/// MMIO headless: sin teclas el poll agota sus 30 vueltas (~1,5 s) y
/// termina SOLO con salida fija — el requisito duro del plan RP-9.
#[test]
fn leccion_09_mmio_termina_sola_sin_teclas() {
    let (code, out) = correr_leccion("../lecciones/09-mmio/leccion.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(
        out,
        "raya roja pintada: mirá «Pantalla (320×240)»\n\
         encuesta terminada, teclas atendidas: 0\n"
    );
}

/// Traps: el handler corre solo (instala mtvec, atiende el 777, mret
/// vuelve) — toda la lección 10 es headless por diseño.
#[test]
fn leccion_10_traps() {
    let (code, out) = correr_leccion("../lecciones/10-traps/leccion.s", "");
    assert_eq!(code, Some(0));
    assert_eq!(
        out,
        "manejador instalado: ahora tengo mini-kernel\n\
         \u{20}\u{20}[manejador] mcause = 11\n\
         \u{20}\u{20}[manejador] servicio 777: atendido\n\
         de vuelta en main: el programa siguió como si nada\n"
    );
}
