//! RP13 Fase 3 — extensión C: encodings == GNU palabra a palabra,
//! ejecución mixta 4/2, breakpoints sobre C, y CERO regresión.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::disasm::{disasm_bytes, listado};
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::TEXT_BASE;
use std::process::Command;
use std::sync::mpsc::channel;

fn machine_for(src: &str) -> Machine<'static> {
    let program = assemble(src).expect("ensambla");
    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    Machine::new(program, out_tx, in_rx).expect("máquina")
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

/// El set didáctico de C, con un destino de salto real al final.
const REPERTORIO: &str = "\
c.nop
c.addi a0, 1
c.addi sp, -16
c.li a2, 5
c.li a0, -1
c.mv a1, a0
c.jr ra
c.j etiqueta
c.lw a0, 4(a1)
c.sw a0, 8(a1)
c.beqz a0, etiqueta
c.bnez a0, etiqueta
etiqueta:
c.nop
";

#[test]
fn repertorio_c_palabra_a_palabra_contra_gnu() {
    let nuestro = assemble(&format!(".text\n{REPERTORIO}\n"))
        .unwrap_or_else(|e| panic!("nuestro ensamblador rechazó C: {e:?}"))
        .text;

    let (as_, objcopy) = ("riscv64-elf-as", "riscv64-elf-objcopy");
    if Command::new(as_).arg("--version").output().is_err() {
        eprintln!("SALTEADO: toolchain GNU no disponible");
        return;
    }
    let dir = std::env::temp_dir().join("rp13_oraculo_c");
    let _ = std::fs::create_dir_all(&dir);
    let s = dir.join("c.s");
    let o = dir.join("c.o");
    let bin = dir.join("c.bin");
    std::fs::write(&s, format!(".text\n{REPERTORIO}\n")).unwrap();
    let out = Command::new(as_)
        .args(["-march=rv32imac", "-o", o.to_str().unwrap(), s.to_str().unwrap()])
        .output()
        .expect("as");
    assert!(out.status.success(), "GNU as: {}", String::from_utf8_lossy(&out.stderr));
    Command::new(objcopy)
        .args(["-O", "binary", "-j", ".text", o.to_str().unwrap(), bin.to_str().unwrap()])
        .output()
        .expect("objcopy");
    let gnu = std::fs::read(&bin).unwrap();
    assert_eq!(nuestro, gnu, "los bytes de C deben coincidir con GNU exacto");
}

/// Ejecución MIXTA 4/2 bytes: addi (4) + c.addi (2) + c.addi (2) + addi (4)
/// → a0 = 8, y el programa termina bien (el motor no descarrila).
#[test]
fn ejecucion_mixta_4_2() {
    let src = "
.text
  addi a0, zero, 5
  c.addi a0, 1
  c.addi a0, 1
  addi a0, a0, 1
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    assert_eq!(m.registers().expect("regs")[10], 8, "5+1+1+1");
}

/// Las direcciones del source_map son correctas con tamaños mixtos: la 4.ª
/// instrucción (addi tras dos C) está en TEXT_BASE + 4 + 2 + 2 = +8.
#[test]
fn source_map_direcciones_con_tamano_mixto() {
    let p = assemble(
        ".text\n  addi a0, zero, 5\n  c.addi a0, 1\n  c.addi a0, 1\nmeta:\n  addi a0, a0, 1\n  li a7, 10\n  ecall\n",
    )
    .expect("ensambla");
    assert_eq!(p.symbols["meta"], TEXT_BASE + 8, "4+2+2 = 8");
}

/// Un breakpoint sobre una instrucción COMPRIMIDA detiene EXACTO en su
/// dirección (la prueba del invariante de RP-7 con tamaño variable).
#[test]
fn breakpoint_sobre_comprimida() {
    let src = "
.text
  addi a0, zero, 5
  c.addi a0, 1
comp:
  c.addi a0, 1
  addi a0, a0, 1
  li a7, 10
  ecall
";
    let p = assemble(src).expect("ensambla");
    let dir = p.symbols["comp"]; // TEXT_BASE + 4 + 2 = +6
    assert_eq!(dir, TEXT_BASE + 6, "la C está en +6");
    let mut m = machine_for(src);
    m.set_breakpoints([dir]).expect("bp");
    assert_eq!(
        m.run_slice(0, 100_000),
        StopReason::Breakpoint { addr: dir },
        "frena exacto en la dirección de la C"
    );
    assert_eq!(m.registers().expect("regs")[10], 6, "5+1 ya corrió; +1 todavía no");
}

/// disasm_bytes avanza por longitud variable: lista C de 2 bytes y normales
/// de 4 con sus direcciones correlativas.
#[test]
fn disasm_bytes_longitud_variable() {
    let p = assemble(
        ".text\n  addi a0, zero, 5\n  c.addi a0, 1\n  c.li a2, 5\n  addi a0, a0, 1\n",
    )
    .expect("ensambla");
    let lineas = disasm_bytes(&p.text, TEXT_BASE);
    assert_eq!(lineas.len(), 4, "4 instrucciones");
    assert_eq!(lineas[0].addr, TEXT_BASE);
    assert_eq!(lineas[1].addr, TEXT_BASE + 4, "addi de 4");
    assert_eq!(lineas[2].addr, TEXT_BASE + 6, "c.addi de 2");
    assert_eq!(lineas[3].addr, TEXT_BASE + 8, "c.li de 2");
    assert!(lineas[1].texto.starts_with("c."), "decodifica C: {}", lineas[1].texto);
    let _ = listado(&lineas);
}

/// Errores didácticos de C: registro fuera de x8–x15 en c.lw, x0 en c.addi.
#[test]
fn errores_didacticos_c() {
    let e = assemble(".text\nc.lw t0, 4(a1)\n").unwrap_err();
    assert!(e[0].msg.contains("x8") && e[0].msg.contains("x15"), "{}", e[0].msg);
    let e = assemble(".text\nc.addi zero, 1\n").unwrap_err();
    assert!(e[0].msg.contains("x0") || e[0].msg.contains("zero"), "{}", e[0].msg);
    let e = assemble(".text\nc.lw a0, 3(a1)\n").unwrap_err();
    assert!(e[0].msg.contains("múltiplo de 4"), "{}", e[0].msg);
}

/// Importar un ELF (RP-11) con C: round-trip asm→ELF→import→correr con
/// instrucciones comprimidas dentro — el modo binario las soporta.
#[test]
fn elf_round_trip_con_c() {
    use emurv_lib::core::elf_export::export_elf;
    use emurv_lib::core::elf_import::import_elf;
    let src = "
.text
  addi a0, zero, 5
  c.addi a0, 1
  c.addi a0, 1
  li a7, 10
  ecall
";
    let original = assemble(src).expect("ensambla");
    let bytes = export_elf(&original).expect("exporta");
    let importado = import_elf(&bytes).expect("importa");
    assert_eq!(importado.text, original.text, "bytes (con C) idénticos");

    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    let mut m = Machine::new(importado, out_tx, in_rx).expect("máquina");
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    assert_eq!(m.registers().expect("regs")[10], 7, "5+1+1 con C importadas");
}
