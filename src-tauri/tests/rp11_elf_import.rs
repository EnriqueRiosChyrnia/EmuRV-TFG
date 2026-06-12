//! RP11 Fase 2 — import de ELF32 RISC-V con validaciones didácticas.
//!
//! Fixture primaria: round-trip con nuestro propio export (lleva .symtab
//! desde RP-6). Fixture real: ELF de riscv64-elf-gcc commiteado en
//! tests/fixtures/rp11/ (con fuente y Makefile de regeneración).

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::elf_export::export_elf;
use emurv_lib::core::elf_import::{es_elf, import_elf};
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::TEXT_BASE;
use std::sync::mpsc::{channel, Receiver};

fn machine_de(p: emurv_lib::core::assembler::Program) -> (Machine<'static>, Receiver<ConsoleEvent>) {
    let (out_tx, out_rx) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    (Machine::new(p, out_tx, in_rx).expect("máquina"), out_rx)
}

fn correr(m: &mut Machine<'static>) -> StopReason {
    let mut r = m.run_slice(0, 1_000_000);
    for _ in 0..50 {
        if !matches!(r, StopReason::Paused) {
            break;
        }
        r = m.run_slice_continuation(0, 1_000_000);
    }
    r
}

fn salida(rx: &Receiver<ConsoleEvent>) -> String {
    let mut out = String::new();
    while let Ok(ev) = rx.try_recv() {
        if let ConsoleEvent::Output(s) = ev {
            out.push_str(&s);
        }
    }
    out
}

const FUENTE: &str = "
.data
v: .word 7
saludo: .asciiz \"hola elf\\n\"
.text
main:
  la a0, saludo
  li a7, 4
  ecall
  la t0, v
  lw a0, 0(t0)
  li a7, 1
  ecall
  li a0, 10
  li a7, 11
  ecall
  li a7, 10
  ecall
";

/// EL criterio del ROADMAP: asm → ELF → import → correr → MISMA salida.
#[test]
fn round_trip_misma_salida_y_simbolos() {
    let original = assemble(FUENTE).expect("ensambla");
    let bytes = export_elf(&original).expect("exporta");
    assert!(es_elf(&bytes));

    let importado = import_elf(&bytes).expect("importa");
    assert!(importado.from_binary, "marcado como binario");
    assert_eq!(importado.entry(), TEXT_BASE);
    assert!(importado.source_map.addr_to_line.is_empty(), "sin fuente");
    assert_eq!(
        importado.symbols.get("main"),
        original.symbols.get("main"),
        "símbolo de .text preservado"
    );
    assert_eq!(
        importado.symbols.get("v"),
        original.symbols.get("v"),
        "símbolo de .data preservado"
    );
    assert_eq!(&importado.text, &original.text, "bytes de .text idénticos");

    let (mut m1, rx1) = machine_de(original);
    let (mut m2, rx2) = machine_de(importado);
    assert!(matches!(correr(&mut m1), StopReason::Exited { code: 0 }));
    assert!(matches!(correr(&mut m2), StopReason::Exited { code: 0 }));
    let (s1, s2) = (salida(&rx1), salida(&rx2));
    assert_eq!(s1, s2, "misma salida fuente vs binario");
    assert_eq!(s1, "hola elf\n7\n");
}

/// Cada validación didáctica, una por una, parcheando un ELF válido.
#[test]
fn validaciones_didacticas() {
    let valido = export_elf(&assemble(FUENTE).unwrap()).unwrap();

    // No-ELF.
    let e = import_elf(b"hola mundo").unwrap_err();
    assert!(e.contains("no es un ELF"), "{e}");

    // Clase 64.
    let mut b = valido.clone();
    b[4] = 2;
    let e = import_elf(&b).unwrap_err();
    assert!(e.contains("64 bits"), "{e}");
    assert!(e.contains("-march=rv32im"), "flags en el mensaje: {e}");

    // Big-endian.
    let mut b = valido.clone();
    b[5] = 2;
    let e = import_elf(&b).unwrap_err();
    assert!(e.contains("big-endian"), "{e}");

    // Otra máquina (62 = x86-64).
    let mut b = valido.clone();
    b[18] = 62;
    b[19] = 0;
    let e = import_elf(&b).unwrap_err();
    assert!(e.contains("no es de RISC-V"), "{e}");
    assert!(e.contains("243"), "{e}");

    // ET_DYN (PIE).
    let mut b = valido.clone();
    b[16] = 3;
    let e = import_elf(&b).unwrap_err();
    assert!(e.contains("PIE"), "{e}");
    assert!(e.contains("-no-pie"), "{e}");

    // ET_REL (objeto sin linkear).
    let mut b = valido.clone();
    b[16] = 1;
    let e = import_elf(&b).unwrap_err();
    assert!(e.contains("relocalizable"), "{e}");
}

/// Sección fuera del mapa RARS → error con los flags de linker EXACTOS.
#[test]
fn seccion_fuera_del_mapa_con_flags_en_el_error() {
    let mut b = export_elf(&assemble(FUENTE).unwrap()).unwrap();
    // Parchear sh_addr de la sección cuyo addr == TEXT_BASE (ELF32:
    // e_shoff en 32, sh_entsize 40, sh_addr en el offset 12 del header).
    let shoff = u32::from_le_bytes([b[32], b[33], b[34], b[35]]) as usize;
    let shnum = u16::from_le_bytes([b[48], b[49]]) as usize;
    let mut parcheado = false;
    for i in 0..shnum {
        let off = shoff + i * 40;
        let addr = u32::from_le_bytes([b[off + 12], b[off + 13], b[off + 14], b[off + 15]]);
        if addr == TEXT_BASE {
            b[off + 12..off + 16].copy_from_slice(&0x1000u32.to_le_bytes());
            parcheado = true;
            break;
        }
    }
    assert!(parcheado, "no encontré la sección .text para parchear");
    let e = import_elf(&b).unwrap_err();
    assert!(e.contains("fuera del mapa"), "{e}");
    assert!(
        e.contains("-march=rv32im -mabi=ilp32 -Ttext=0x00400000 -Tdata=0x10010000 -nostdlib"),
        "los flags COMPLETOS van en el error: {e}"
    );
}

/// e_entry fuera del código cargado → error didáctico.
#[test]
fn entry_fuera_de_text_rechazado() {
    let mut b = export_elf(&assemble(FUENTE).unwrap()).unwrap();
    b[24..28].copy_from_slice(&0x0050_0000u32.to_le_bytes());
    let e = import_elf(&b).unwrap_err();
    assert!(e.contains("punto de entrada"), "{e}");
}

/// La fixture REAL de gcc (tests/fixtures/rp11): entry ≠ TEXT_BASE pero
/// dentro de .text (caso «entry variable»), corre y da su salida exacta.
#[test]
fn fixture_gnu_importa_y_corre() {
    let bytes = include_bytes!("fixtures/rp11/fixture.elf");
    let p = import_elf(bytes).expect("importa el ELF de gcc");
    assert!(p.from_binary);
    assert_ne!(p.entry(), TEXT_BASE, "gcc eligió otro _start");
    assert!(
        p.entry() > TEXT_BASE && p.entry() < TEXT_BASE + p.text.len() as u32,
        "pero cae dentro del código cargado"
    );
    assert!(p.symbols.contains_key("_start"), "símbolo global de gcc");

    let (mut m, rx) = machine_de(p);
    let r = correr(&mut m);
    assert!(matches!(r, StopReason::Exited { code: 0 }), "{r:?}");
    assert_eq!(salida(&rx), "42\n", "la salida exacta de la fixture");
}
