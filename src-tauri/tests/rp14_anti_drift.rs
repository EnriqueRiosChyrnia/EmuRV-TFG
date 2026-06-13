//! RP14 Fase 1 — anti-drift de los tokens del editor.
//!
//! Diseño elegido (BITACORA RP14-F1): `assembler::known_mnemonics()` es la
//! lista CANÓNICA en Rust; este test (1) prueba que cada entrada de esa
//! lista la acepta el ensamblador de verdad (no «instrucción desconocida»)
//! y (2) lee `src/editor/isa.ts` y exige que MNEMONICS ∪ PSEUDOS == la
//! lista canónica. Así el resaltado/autocompletado del editor no puede
//! desincronizarse de lo que el assembler acepta (la deuda de RP-12/13).

use emurv_lib::core::assembler::{assemble, known_mnemonics};
use std::collections::BTreeSet;

/// El texto de isa.ts compilado dentro del test: si alguien lo edita y
/// rompe la sincronía, el test falla en la próxima corrida.
const ISA_TS: &str = include_str!("../../src/editor/isa.ts");

/// Extrae los strings entre comillas del array `nombre` de isa.ts. Los
/// arrays MNEMONICS y PSEUDOS son listas literales de strings (sin spreads
/// ni `Array.from`), así que un parseo por comillas alcanza.
fn array_de_isa(nombre: &str) -> BTreeSet<String> {
    let inicio = ISA_TS
        .find(&format!("export const {nombre}: string[] = ["))
        .unwrap_or_else(|| panic!("no encontré el array {nombre} en isa.ts"));
    let resto = &ISA_TS[inicio..];
    let fin = resto.find("];").expect("array sin cierre");
    let cuerpo = &resto[..fin];
    let mut out = BTreeSet::new();
    let mut chars = cuerpo.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            // tomar hasta la próxima comilla
            let rel = cuerpo[i + 1..].find('"').expect("string sin cierre");
            out.insert(cuerpo[i + 1..i + 1 + rel].to_string());
            // avanzar el iterador más allá del string cerrado
            for _ in 0..rel + 1 {
                chars.next();
            }
        }
    }
    out
}

/// (1) Cada mnemónico canónico es RECONOCIDO por el ensamblador: al
/// ensamblarlo solo, el error (si lo hay) puede ser de forma, pero NUNCA
/// «instrucción desconocida». Eso prueba que known_mnemonics() no tiene
/// entradas fantasma respecto de check_shape.
#[test]
fn cada_mnemonico_conocido_se_acepta() {
    for m in known_mnemonics() {
        let src = format!(".text\n{m}\n");
        if let Err(errores) = assemble(&src) {
            for e in &errores {
                assert!(
                    !e.msg.contains("instrucción desconocida"),
                    "el ensamblador NO reconoce «{m}» (known_mnemonics está desfasado): {}",
                    e.msg
                );
            }
        }
        // Ok(_) también es válido (las de 0 operandos ensamblan enteras).
    }
}

/// (2) El editor (isa.ts) cubre EXACTAMENTE la lista canónica: ni le falta
/// ninguna (la deuda F/D/A/C que RP-14 salda) ni inventa de más.
#[test]
fn isa_ts_iguala_known_mnemonics() {
    let canonica: BTreeSet<String> = known_mnemonics().into_iter().map(String::from).collect();
    let mut editor = array_de_isa("MNEMONICS");
    editor.extend(array_de_isa("PSEUDOS"));

    let faltan: Vec<&String> = canonica.difference(&editor).collect();
    let sobran: Vec<&String> = editor.difference(&canonica).collect();
    assert!(
        faltan.is_empty(),
        "isa.ts NO incluye instrucciones que el ensamblador acepta: {faltan:?}"
    );
    assert!(
        sobran.is_empty(),
        "isa.ts incluye instrucciones que el ensamblador NO acepta: {sobran:?}"
    );
}

/// Muestra concreta de la deuda saldada: F/D, A y C estaban ausentes del
/// editor viejo; ahora isa.ts las trae (test explícito por si alguien
/// recorta la lista sin querer).
#[test]
fn isa_ts_incluye_fd_a_c() {
    let mut editor = array_de_isa("MNEMONICS");
    editor.extend(array_de_isa("PSEUDOS"));
    for m in ["fadd.s", "fmul.d", "amoadd.w", "lr.w", "c.addi", "c.lw"] {
        assert!(editor.contains(m), "isa.ts debería incluir «{m}»");
    }
}
