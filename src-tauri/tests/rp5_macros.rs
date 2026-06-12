//! RP5 Fase 3 — macros estilo RARS con atribución a la invocación.

use emurv_lib::core::assembler::{assemble, assemble_with_includes};
use std::path::PathBuf;

fn carpeta_con(nombre_test: &str, archivos: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rp5_mac_{nombre_test}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("crear carpeta temporal");
    for (nombre, contenido) in archivos {
        std::fs::write(dir.join(nombre), contenido).expect("escribir archivo");
    }
    dir
}

fn words(p: &emurv_lib::core::assembler::Program) -> Vec<u32> {
    p.text
        .chunks(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

#[test]
fn macro_simple_y_con_parametros() {
    // La expansión debe producir EXACTAMENTE el mismo binario que el
    // código escrito a mano.
    let con_macro = assemble(
        ".macro doble (%rd, %rs)\n  add %rd, %rs, %rs\n.end_macro\n.text\n  li t0, 21\n  doble t1, t0\n  li a7, 10\n  ecall\n",
    )
    .expect("ensambla");
    let a_mano =
        assemble(".text\n  li t0, 21\n  add t1, t0, t0\n  li a7, 10\n  ecall\n").expect("ensambla");
    assert_eq!(words(&con_macro), words(&a_mano));

    // Cierre alternativo .endm y macro sin parámetros.
    let p = assemble(".macro tres_nops\n  nop\n  nop\n  nop\n.endm\n.text\n  tres_nops\n  ecall\n")
        .expect("ensambla");
    assert_eq!(words(&p).len(), 4);
}

#[test]
fn macro_usa_constante_eqv() {
    let p = assemble(
        ".eqv LIMITE 10\n.macro carga (%rd)\n  li %rd, LIMITE\n.end_macro\n.text\n  carga t0\n  ecall\n",
    )
    .expect("ensambla");
    let q = assemble(".text\n  li t0, 10\n  ecall\n").expect("ensambla");
    assert_eq!(words(&p), words(&q));
}

#[test]
fn macro_definida_en_include_invocada_en_principal() {
    let dir = carpeta_con(
        "en_include",
        &[(
            "macros.s",
            ".macro salir\n  li a7, 10\n  ecall\n.end_macro\n",
        )],
    );
    let p = assemble_with_includes(
        ".include \"macros.s\"\n.text\n  nop\n  salir\n",
        Some(&dir),
        "main.s",
    )
    .expect("ensambla");
    // nop + li + ecall = 3 palabras.
    assert_eq!(words(&p).len(), 3);
    // Atribución: las dos palabras de «salir» van a la línea 4 (invocación).
    assert_eq!(p.source_map.addr_for_line(4), Some(0x0040_0004));
}

#[test]
fn macro_dentro_de_macro_y_recursion_cortada() {
    // Invocar una macro desde otra: permitido.
    let p = assemble(
        ".macro fin\n  li a7, 10\n  ecall\n.end_macro\n.macro todo\n  nop\n  fin\n.end_macro\n.text\n  todo\n",
    )
    .expect("ensambla");
    assert_eq!(words(&p).len(), 3);

    // Recursión directa: el límite la corta con error didáctico.
    let errs =
        assemble(".macro bucle\n  bucle\n.end_macro\n.text\n  bucle\n").expect_err("debe fallar");
    assert!(
        errs[0].msg.contains("demasiado profunda"),
        "{}",
        errs[0].msg
    );
}

#[test]
fn n_invocaciones_atribuyen_a_sus_n_lineas() {
    // El corazón del invariante en macros: cada invocación es un grupo de
    // direcciones propio, atribuido a SU línea de invocación.
    let p = assemble(".macro par\n  nop\n  nop\n.end_macro\n.text\n  par\n  par\n  par\n  ecall\n")
        .expect("ensambla");
    // Líneas 6, 7, 8 = invocaciones (1-based). 2 palabras cada una.
    assert_eq!(p.source_map.addr_for_line(6), Some(0x0040_0000));
    assert_eq!(p.source_map.addr_for_line(7), Some(0x0040_0008));
    assert_eq!(p.source_map.addr_for_line(8), Some(0x0040_0010));
    // Y las direcciones intermedias del grupo apuntan de vuelta a su línea.
    assert_eq!(p.source_map.line_for_addr(0x0040_000C), Some(7));
}

#[test]
fn errores_didacticos_de_macros() {
    // Cantidad de argumentos.
    let errs = assemble(".macro f (%a)\n  li t0, %a\n.end_macro\n.text\n  f 1, 2\n")
        .expect_err("debe fallar");
    assert!(
        errs[0].msg.contains("espera 1 argumento"),
        "{}",
        errs[0].msg
    );

    // Definición anidada.
    let errs = assemble(".macro a\n.macro b\n.end_macro\n.text\nnop\n").expect_err("debe fallar");
    assert!(
        errs.iter().any(|e| e.msg.contains("dentro de otra macro")),
        "{errs:?}"
    );

    // .macro sin cierre.
    let errs = assemble(".macro abierta\n  nop\n.text\nnop\n").expect_err("debe fallar");
    assert!(
        errs.iter().any(|e| e.msg.contains("sin su .end_macro")),
        "{errs:?}"
    );

    // Parámetro sin %.
    let errs = assemble(".macro f (a)\n.end_macro\n.text\nnop\n").expect_err("debe fallar");
    assert!(errs[0].msg.contains("van con %"), "{}", errs[0].msg);
}
