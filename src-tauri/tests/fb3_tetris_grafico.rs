//! Test FB3: Tetris gráfico (port real del Tetris por turnos de v0.1).
//! Reusa el oráculo del test de 06-tetris: misma secuencia determinista de
//! piezas (LCG con semilla 0x1234) y verificación del estado del tablero,
//! ahora leyéndolo de memoria (símbolo `tablero`) en lugar del ASCII, más
//! la verificación de los píxeles dibujados en el framebuffer.

use emurv_lib::core::{
    assembler::assemble,
    machine::{Machine, StopReason},
    syscalls::{ConsoleEvent, InputMsg},
};
use std::sync::mpsc::channel;

const J: u8 = 5 + 2; // tipo J = 5 → color CGA 7 (gris claro)
const O: u8 = 1 + 2; // tipo O = 1 → color CGA 3 (cian)

fn run_tetris(inputs: &[&str]) -> (String, Vec<u8>, Vec<u8>, StopReason) {
    let src =
        std::fs::read_to_string("../ejemplos/08-tetris-grafico.asm").expect("leer ejemplo 08");
    let program = assemble(&src).unwrap_or_else(|errs| {
        panic!(
            "el ejemplo debe ensamblar sin errores; hubo:\n{}",
            errs.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let tablero_addr = *program.symbols.get("tablero").expect("símbolo tablero");

    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    for line in inputs {
        in_tx.send(InputMsg::Line((*line).to_string())).unwrap();
    }
    let mut m = Machine::new(program, out_tx, in_rx).expect("máquina");
    let reason = m.run(0);

    let mut output = String::new();
    while let Ok(ev) = out_rx.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            output.push_str(&s);
        }
    }
    let board = m.read_mem(tablero_addr, 200).expect("leer tablero");
    let fb = m.read_framebuffer_rgba().expect("leer framebuffer");
    (output, board, fb, reason)
}

/// RGBA del centro de la celda (fila, col) del tablero dibujado:
/// celda de 12x12 px con origen del tablero en x=8, y=0.
fn pixel_celda(fb: &[u8], fila: usize, col: usize) -> [u8; 3] {
    let y = fila * 12 + 5;
    let x = 8 + col * 12 + 5;
    let off = (y * 320 + x) * 4;
    [fb[off], fb[off + 1], fb[off + 2]]
}

#[test]
fn fb3_tetris_logica_identica_al_port_por_turnos() {
    // Jugada del oráculo de v0.1: J rot 0 en col 0; O rot 0 en col 4; -1.
    let (out, board, _fb, reason) = run_tetris(&["0", "0", "0", "4", "-1"]);
    assert_eq!(reason, StopReason::Exited { code: 0 });

    // Orden de aparición de las piezas (verifica el port del LCG).
    let pj = out.find("Pieza: J").expect("J primero");
    let po = out.find("Pieza: O").expect("O segundo");
    let pi = out.find("Pieza: I").expect("I tercero");
    assert!(pj < po && po < pi, "orden de piezas LCG");

    // Estado final del tablero EN MEMORIA (mismo oráculo que el ASCII
    // |##..##....| / |.#..##....| / |.#........|, con colores por pieza).
    let celda = |fila: usize, col: usize| board[fila * 10 + col];
    assert_eq!(celda(19, 0), J, "fila 19 col 0: bloque J");
    assert_eq!(celda(19, 1), J, "fila 19 col 1: bloque J");
    assert_eq!(celda(19, 4), O, "fila 19 col 4: bloque O");
    assert_eq!(celda(19, 5), O, "fila 19 col 5: bloque O");
    assert_eq!(celda(18, 1), J, "fila 18 col 1: bloque J");
    assert_eq!(celda(18, 4), O, "fila 18 col 4: bloque O");
    assert_eq!(celda(18, 5), O, "fila 18 col 5: bloque O");
    assert_eq!(celda(17, 1), J, "fila 17 col 1: bloque J");
    // celdas que deben seguir vacías
    assert_eq!(celda(19, 2), 0);
    assert_eq!(celda(18, 0), 0);
    assert_eq!(celda(0, 0), 0);

    // Sin líneas completas no hay puntos (tabla 100/300/500/800 intacta).
    assert!(out.contains("SCORE: 0"), "salida: {out}");
    assert!(out.contains("Fin del juego"), "salida: {out}");
}

#[test]
fn fb3_tetris_dibuja_el_tablero_en_el_framebuffer() {
    let (_out, _board, fb, _reason) = run_tetris(&["0", "0", "0", "4", "-1"]);

    // Bloques J (color CGA 7 = 0xAA gris claro) apilados en col 0-1.
    assert_eq!(
        pixel_celda(&fb, 19, 0),
        [0xAA, 0xAA, 0xAA],
        "celda J (19,0)"
    );
    assert_eq!(
        pixel_celda(&fb, 17, 1),
        [0xAA, 0xAA, 0xAA],
        "celda J (17,1)"
    );
    // Bloques O (color CGA 3 = cian 0x00,0xAA,0xAA) en col 4-5.
    assert_eq!(
        pixel_celda(&fb, 19, 4),
        [0x00, 0xAA, 0xAA],
        "celda O (19,4)"
    );
    assert_eq!(
        pixel_celda(&fb, 18, 5),
        [0x00, 0xAA, 0xAA],
        "celda O (18,5)"
    );
    // Celda vacía = negro.
    assert_eq!(pixel_celda(&fb, 0, 5), [0x00, 0x00, 0x00], "celda vacía");
    // Borde lateral blanco (x=5, y=100).
    let off = (100 * 320 + 5) * 4;
    assert_eq!(&fb[off..off + 3], &[0xFF, 0xFF, 0xFF], "borde blanco");
}
