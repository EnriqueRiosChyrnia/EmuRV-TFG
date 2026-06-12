//! TEC5 — Oráculo determinista del ejemplo `ejemplos/09-eco-teclado.asm`
//! (la primera exposición del estudiante al polling MMIO).
//!
//! Las teclas se inyectan en la cola compartida ANTES de correr (patrón de
//! tests/tec2_teclado.rs): el programa nunca encuentra la cola vacía, así
//! que su rama de espera (dormir 10 ms con la syscall 32) no se ejecuta y
//! la corrida entera termina en milisegundos. Eso depende de un detalle de
//! diseño del .asm que acá se vigila explícitamente: el bucle chequea
//! CONTROL ANTES de dormir, no al revés.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::ConsoleEvent;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

/// Resultado observable de una corrida completa del ejemplo.
struct Corrida {
    reason: StopReason,
    salida: String,
    fb: Vec<u8>,
    /// Duración de `run()` sola (sin ensamblar ni crear la máquina).
    run_ms: u128,
}

/// Guardián anticuelgue (patrón de tests/tec1_detener.rs): corre `f` en un
/// hilo aparte y espera el resultado a lo sumo `limite`; si la ejecución se
/// colgara (p. ej. un bucle de polling que nunca ve el Esc), el test FALLA
/// con mensaje claro en vez de colgar la suite. La máquina se construye
/// adentro del hilo porque Unicorn no es Send.
fn con_guardia<T, F>(nombre: &'static str, limite: Duration, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        // Si el receptor ya abandonó (timeout), el send falla y da igual.
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limite) {
        Ok(v) => v,
        Err(_) => panic!("{nombre}: la ejecución no retornó en {limite:?}"),
    }
}

/// Ensambla el ejemplo REAL del repo, inyecta `teclas` en la cola
/// compartida antes de correr (lo que hará la UI vía `push_key`) y corre
/// hasta que el programa termine solo.
fn correr_eco(teclas: Vec<u8>) -> Corrida {
    // En debug el core C de Unicorn se compila sin optimizar y se le da
    // margen (mismo criterio que tests/tec1_detener.rs).
    let limite = Duration::from_secs(if cfg!(debug_assertions) { 20 } else { 5 });
    con_guardia("eco de teclado", limite, move || {
        let src =
            std::fs::read_to_string("../ejemplos/09-eco-teclado.asm").expect("leer ejemplo 09");
        let program = assemble(&src).unwrap_or_else(|errs| {
            panic!(
                "el ejemplo debe ensamblar sin errores; hubo:\n{}",
                errs.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        let (out_tx, out_rx) = channel();
        let (_in_tx, in_rx) = channel();
        let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");

        // Inyección PREVIA a la corrida (patrón de tests/tec2_teclado.rs).
        {
            let shared = m.keyboard_shared();
            let mut k = shared.lock().expect("lock del teclado");
            for &t in &teclas {
                k.push(t);
            }
        }

        let t0 = Instant::now();
        let reason = m.run(0);
        let run_ms = t0.elapsed().as_millis();

        let mut salida = String::new();
        while let Ok(ev) = out_rx.try_recv() {
            if let ConsoleEvent::Output(s) = ev {
                salida.push_str(&s);
            }
        }
        let fb = m.read_framebuffer_rgba().expect("leer framebuffer");
        Corrida {
            reason,
            salida,
            fb,
            run_ms,
        }
    })
}

/// RGB del centro del bloque `i`: los bloques son de 8x8 píxeles en la
/// fila superior de la pantalla, avanzando de a 8 en x. El centro del
/// bloque `i` es el píxel (x = i*8 + 4, y = 4) del RGBA de 320 de ancho.
fn pixel_bloque(fb: &[u8], i: usize) -> [u8; 3] {
    let off = (4 * 320 + i * 8 + 4) * 4;
    [fb[off], fb[off + 1], fb[off + 2]]
}

/// (a) Oráculo end-to-end con "hola" + Esc inyectados antes de correr:
/// la consola hace eco de "hola", el programa termina limpio al ver el
/// Esc (27) y en el framebuffer quedan ≥4 bloques no negros (uno por
/// tecla impresa; el color (código & 7) + 1 nunca es 0 = negro).
#[test]
fn eco_imprime_y_dibuja_hasta_esc() {
    let c = correr_eco(b"hola\x1b".to_vec());
    assert_eq!(
        c.reason,
        StopReason::Exited { code: 0 },
        "el Esc debía terminar el programa limpio"
    );
    assert_eq!(
        c.salida, "hola",
        "el eco debía imprimir exactamente las teclas previas a Esc"
    );

    // Cada una de las 4 teclas impresas dejó su bloque de color.
    for i in 0..4 {
        assert_ne!(
            pixel_bloque(&c.fb, i),
            [0, 0, 0],
            "el bloque {i} (tecla '{}') no debía ser negro",
            b"hola"[i] as char
        );
    }
    let no_negros = (0..40)
        .filter(|&i| pixel_bloque(&c.fb, i) != [0, 0, 0])
        .count();
    assert!(
        no_negros >= 4,
        "el framebuffer debía tener ≥4 bloques no negros, tiene {no_negros}"
    );
}

/// (b) Con teclas ya encoladas el programa NO duerme: el bucle del .asm
/// chequea CONTROL antes de dormir, así que la rama del sleep de 10 ms
/// jamás corre si siempre hay dato. Si el diseño fuera al revés (dormir
/// y después preguntar), 40 teclas costarían ≥400 ms; acá se exige que
/// la corrida entera quede muy por debajo de eso.
#[test]
fn con_teclas_en_cola_no_duerme() {
    let mut teclas = vec![b'a'; 40];
    teclas.push(27); // Esc al final: terminar
    let c = correr_eco(teclas);
    assert_eq!(c.reason, StopReason::Exited { code: 0 });
    assert_eq!(
        c.salida,
        "a".repeat(40),
        "las 40 teclas previas al Esc debían hacer eco"
    );
    assert!(
        c.run_ms < 200,
        "con la cola siempre cargada no se duerme: un diseño que durmiera \
         10 ms por vuelta tardaría ≥400 ms y esta corrida tardó {} ms",
        c.run_ms
    );
}
