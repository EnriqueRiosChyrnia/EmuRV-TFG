//! TEC5 — Tetris en tiempo real (ejemplos/10-tetris-tiempo-real.asm):
//! teclado MMIO no bloqueante + gravedad por reloj (syscalls 30/32).
//!
//! Dos tests:
//! (a) Oráculo determinista: se "congela" la gravedad escribiendo un
//!     plazo gigante en el word `gravedad_ms` (localizado vía
//!     `program.symbols`, el mismo mecanismo que usa fb3 para `tablero`),
//!     se inyecta una secuencia conocida de teclas en la cola compartida
//!     (patrón de tec2) y se verifica el tablero final en memoria.
//! (b) Smoke de gravedad: `gravedad_ms` = 50 y cola de teclas vacía: el
//!     juego corre solo (polling + sleep de 15 ms) en tramos acotados de
//!     `run_slice`, y el word `ultimo_descenso` tiene que avanzar al
//!     menos dos veces (= dos descensos disparados por el reloj real).

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Color con el que `fijar` marca los bloques de la pieza J en el
/// tablero: tipo J = 5 → índice CGA 5 + 2 = 7 (mismo código que fb3).
const J: u8 = 5 + 2;

/// Máquina lista sobre el ejemplo 10 con el plazo de gravedad pisado en
/// memoria ANTES de correr, más las direcciones de los words que los
/// tests necesitan observar. Las direcciones salen de `program.symbols`
/// (la tabla de símbolos del ensamblador, igual que en fb3).
struct Banco {
    m: Machine<'static>,
    out: Receiver<ConsoleEvent>,
    /// Extremo de entrada de la consola: vive lo que dure el test
    /// aunque el ejemplo nunca use la syscall 5.
    _inp: Sender<InputMsg>,
    tablero: u32,
    ultimo_descenso: u32,
}

fn banco_con_gravedad(gravedad_ms: u32) -> Banco {
    let src =
        std::fs::read_to_string("../ejemplos/10-tetris-tiempo-real.asm").expect("leer ejemplo 10");
    let program = assemble(&src).unwrap_or_else(|errs| {
        panic!(
            "el ejemplo debe ensamblar sin errores; hubo:\n{}",
            errs.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let tablero = *program.symbols.get("tablero").expect("símbolo tablero");
    let gravedad = *program
        .symbols
        .get("gravedad_ms")
        .expect("símbolo gravedad_ms");
    let ultimo_descenso = *program
        .symbols
        .get("ultimo_descenso")
        .expect("símbolo ultimo_descenso");

    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    // El word ya está cargado en .data: pisarlo acá, antes de correr,
    // cambia la dificultad sin tocar el fuente del ejemplo.
    m.write_u32(u64::from(gravedad), gravedad_ms)
        .expect("escribir gravedad_ms");
    Banco {
        m,
        out: out_rx,
        _inp: in_tx,
        tablero,
        ultimo_descenso,
    }
}

/// Guardián anticuelgue (patrón de tests/tec3_tiempo.rs): corre `f` en un
/// hilo aparte y espera el resultado a lo sumo `limite`; si la ejecución
/// se colgara, el test FALLA con mensaje claro en vez de colgar la suite.
/// La máquina se construye adentro del hilo porque Unicorn no es Send.
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

/// (a) Oráculo determinista: con la gravedad congelada (plazo de
/// 0x7FFF_FFFF ms, que nunca vence dentro de un test) la pieza solo se
/// mueve por las teclas inyectadas. La primera pieza del LCG con semilla
/// 0x1234 es la J (igual que en fb3); la secuencia la pasea por las dos
/// direcciones (letras Y flechas), la rota y la baja a caída suave hasta
/// fijarla, y termina con Esc:
///
///   a, a            col 3 → 1
///   d, → (0x81)     col 1 → 3
///   ← (0x80), a     col 3 → 1
///   w               rot 0 → 1 (J rot 1 = 0x8E00: celdas relativas
///                   (0,0), (1,0), (1,1), (1,2))
///   s × 18          fila 0 → 18 (con celdas en filas 18 y 19)
///   ↓ (0x83)        abajo choca con el piso → la pieza se fija
///   Esc (27)        salir limpio
///
/// Tablero esperado: exactamente 4 celdas con el color J (7):
/// (18,1), (19,1), (19,2) y (19,3); todo el resto en 0.
#[test]
fn oraculo_teclas_con_gravedad_congelada() {
    let (motivo, board, salida) =
        con_guardia("oráculo tetris rt", Duration::from_secs(30), move || {
            let mut b = banco_con_gravedad(0x7FFF_FFFF);
            {
                let shared = b.m.keyboard_shared();
                let mut k = shared.lock().expect("lock del teclado");
                let mut teclas: Vec<u8> = vec![b'a', b'a', b'd', 0x81, 0x80, b'a', b'w'];
                teclas.extend_from_slice(&[b's'; 18]);
                teclas.push(0x83); // el descenso que la fija
                teclas.push(27); // Esc
                for t in teclas {
                    k.push(t);
                }
            }
            let motivo = b.m.run(0);
            let board = b.m.read_mem(b.tablero, 200).expect("leer tablero");
            let mut salida = String::new();
            while let Ok(ev) = b.out.try_recv() {
                if let ConsoleEvent::Output(s) = ev {
                    salida.push_str(&s);
                }
            }
            (motivo, board, salida)
        });

    assert_eq!(
        motivo,
        StopReason::Exited { code: 0 },
        "Esc debía salir limpio"
    );

    let celda = |fila: usize, col: usize| board[fila * 10 + col];
    assert_eq!(celda(18, 1), J, "fila 18 col 1: codo de la J");
    assert_eq!(celda(19, 1), J, "fila 19 col 1: base de la J");
    assert_eq!(celda(19, 2), J, "fila 19 col 2: base de la J");
    assert_eq!(celda(19, 3), J, "fila 19 col 3: base de la J");
    let ocupadas = board.iter().filter(|&&c| c != 0).count();
    assert_eq!(
        ocupadas, 4,
        "solo la J fijada debía estar en el tablero (la O siguiente quedó en vuelo)"
    );

    // El LCG no cambió: J primero y, recién tras fijarla, nace la O.
    let pj = salida.find("Pieza: J").expect("anuncio de la J");
    let po = salida
        .find("Pieza: O")
        .expect("anuncio de la O tras fijar la J");
    assert!(pj < po, "la J debía anunciarse antes que la O");
    // Sin líneas completas no hay puntos, y la salida fue por Esc.
    assert!(salida.contains("SCORE: 0"), "salida: {salida}");
    assert!(salida.contains("Fin del juego"), "salida: {salida}");
}

/// (b) Smoke de gravedad real: plazo de 50 ms y NINGUNA tecla. El juego
/// corre solo (polling con cola vacía + sleep de 15 ms) en tramos de
/// `run_slice`; entre tramo y tramo se muestrea el word `ultimo_descenso`,
/// que la gravedad reescribe con la hora de cada descenso. Verlo cambiar
/// dos veces = dos descensos disparados por el reloj. Cada descenso
/// redibuja el tablero (~125 mil instrucciones), así que un tramo de
/// 60 mil no puede contener dos descensos: ninguno pasa desapercibido.
#[test]
fn gravedad_sola_baja_la_pieza() {
    let (motivo, descensos) = con_guardia("smoke tetris rt", Duration::from_secs(30), move || {
        let mut b = banco_con_gravedad(50);

        // Red de seguridad: si la gravedad nunca disparara, el juego
        // se la pasaría durmiendo y los tramos tardarían una
        // eternidad en consumir sus instrucciones; el stop_request
        // corta cualquier run a los 6 s (el sleep es interrumpible).
        let stop = Arc::new(AtomicBool::new(false));
        b.m.set_stop_request(Arc::clone(&stop));
        let centinela = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(6));
            centinela.store(true, Ordering::SeqCst);
        });

        // Primer tramo: main ya inicializa `ultimo_descenso` con la
        // hora de arranque (ese cambio NO es un descenso y queda
        // absorbido como valor base del muestreo).
        let mut motivo = b.m.run_slice(0, 60_000);
        let mut previo =
            b.m.read_u32(u64::from(b.ultimo_descenso))
                .expect("leer ultimo_descenso");
        let mut descensos = 0u32;
        let limite = Instant::now() + Duration::from_secs(2);
        while descensos < 2 && Instant::now() < limite && motivo == StopReason::Paused {
            motivo = b.m.run_slice(0, 60_000);
            let actual =
                b.m.read_u32(u64::from(b.ultimo_descenso))
                    .expect("leer ultimo_descenso");
            if actual != previo {
                descensos += 1;
                previo = actual;
            }
        }
        // Nada queda corriendo (la máquina retornó), pero el flag
        // apagado deja todo prolijo si el centinela aún no disparó.
        stop.store(true, Ordering::SeqCst);
        (motivo, descensos)
    });

    assert_eq!(
        motivo,
        StopReason::Paused,
        "el juego debía seguir corriendo (ni error ni exit) cuando dejamos de mirar"
    );
    assert!(
        descensos >= 2,
        "con gravedad de 50 ms la pieza debía bajar sola al menos 2 veces en ~2 s (hubo {descensos})"
    );
}
