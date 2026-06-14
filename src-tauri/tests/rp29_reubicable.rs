//! RP29 Fase 2 — export ELF reubicable, verificado contra GNU.
//!
//! El modo reubicable codifica las referencias a símbolos de las
//! pseudo-instrucciones (`la`/`lla`, cargas/almacenamientos con etiqueta)
//! como `auipc rd, %pcrel_hi(sym)` + `op rd, %pcrel_lo(...)` (PC-relativo)
//! en vez del par absoluto `lui rd, %hi` + `op rd, %lo`. El export lo marca
//! ET_DYN. El modo normal (absoluto, ET_EXEC) NO cambia.
//!
//! Verificación (la toolchain GNU es el ORÁCULO; si falta, los tests que la
//! usan se saltean — ver rp12_gnu_oracle.rs):
//!   - El `.text` reubicable es BYTE A BYTE el par auipc/%pcrel que GNU as/ld
//!     resuelve para el mapa de EmuRV (.text 0x00400000, .data 0x10010000).
//!   - Ese mismo objeto, enlazado en OTRA base (desplazando .text y .data
//!     juntos), da un `.text` IDÉNTICO → reubicable: misma conducta en otra
//!     dirección.
//!   - El export normal sigue siendo el par absoluto `lui/%hi` (regresión) y
//!     ET_EXEC.

use emurv_lib::core::assembler::{assemble, assemble_reubicable};
use emurv_lib::core::elf_export::export_elf;
use emurv_lib::core::{DATA_BASE, TEXT_BASE};
use std::path::PathBuf;
use std::process::Command;

/// Programa de prueba para EmuRV: una `la`, una carga y un almacenamiento,
/// todas a un dato de .data. Cubre las tres formas que expanden a un par
/// auipc/lui + segunda instrucción.
const PROG: &str = "\
.text
la a0, msj
lw a1, msj
sw a1, msj, t0
.data
msj: .word 0xcafe
";

/// El equivalente GNU con el par PC-relativo EXPLÍCITO (la idiomática
/// auipc/%pcrel_hi + op/%pcrel_lo, donde %pcrel_lo referencia la etiqueta del
/// auipc). Misma secuencia y mismo dato que PROG, para comparar `.text`.
const GNU_PCREL: &str = "\
.text
.globl _start
_start:
1:  auipc a0, %pcrel_hi(msj)
    addi  a0, a0, %pcrel_lo(1b)
2:  auipc a1, %pcrel_hi(msj)
    lw    a1, %pcrel_lo(2b)(a1)
3:  auipc t0, %pcrel_hi(msj)
    sw    a1, %pcrel_lo(3b)(t0)
.data
msj: .word 0xcafe
";

/// El equivalente GNU ABSOLUTO (par lui/%hi + op/%lo): oráculo del modo normal.
const GNU_ABS: &str = "\
.text
.globl _start
_start:
    lui   a0, %hi(msj)
    addi  a0, a0, %lo(msj)
    lui   a1, %hi(msj)
    lw    a1, %lo(msj)(a1)
    lui   t0, %hi(msj)
    sw    a1, %lo(msj)(t0)
.data
msj: .word 0xcafe
";

fn toolchain() -> Option<(String, String, String)> {
    let (as_, ld, objcopy) = ("riscv64-elf-as", "riscv64-elf-ld", "riscv64-elf-objcopy");
    Command::new(as_).arg("--version").output().ok()?;
    Command::new(ld).arg("--version").output().ok()?;
    Command::new(objcopy).arg("--version").output().ok()?;
    Some((as_.into(), ld.into(), objcopy.into()))
}

fn dir(label: &str) -> PathBuf {
    let d = std::env::temp_dir().join("rp29_f2").join(label);
    std::fs::create_dir_all(&d).expect("crear dir temporal");
    d
}

fn ok(out: std::process::Output, que: &str) {
    assert!(
        out.status.success(),
        "{que} falló: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Ensambla `src` con GNU (sin relax) y lo enlaza con las bases dadas;
/// devuelve los bytes de `.text`.
fn gnu_text(label: &str, src: &str, text_base: u32, data_base: u32) -> Vec<u8> {
    let (as_, ld, objcopy) = toolchain().expect("toolchain verificada por el llamador");
    let d = dir(label);
    let (s, o, elf, bin) = (
        d.join("in.s"),
        d.join("in.o"),
        d.join("out.elf"),
        d.join("out.bin"),
    );
    std::fs::write(&s, src).expect("escribir fuente");
    ok(
        Command::new(&as_)
            .args([
                "-march=rv32i",
                "-mabi=ilp32",
                "-mno-relax",
                "-o",
                o.to_str().unwrap(),
                s.to_str().unwrap(),
            ])
            .output()
            .expect("lanzar as"),
        "GNU as",
    );
    ok(
        Command::new(&ld)
            .args([
                "-m",
                "elf32lriscv",
                "--no-relax",
                &format!("--section-start=.text={text_base:#x}"),
                &format!("--section-start=.data={data_base:#x}"),
                "-o",
                elf.to_str().unwrap(),
                o.to_str().unwrap(),
            ])
            .output()
            .expect("lanzar ld"),
        "GNU ld",
    );
    ok(
        Command::new(&objcopy)
            .args([
                "-O",
                "binary",
                "-j",
                ".text",
                elf.to_str().unwrap(),
                bin.to_str().unwrap(),
            ])
            .output()
            .expect("lanzar objcopy"),
        "GNU objcopy",
    );
    std::fs::read(&bin).expect("leer .text")
}

fn opcode(word: &[u8]) -> u32 {
    u32::from_le_bytes(word.try_into().unwrap()) & 0x7f
}

const OP_LUI: u32 = 0x37;
const OP_AUIPC: u32 = 0x17;

/// Sin toolchain: el modo reubicable abre cada expansión con `auipc` (0x17);
/// el normal, con `lui` (0x37). Es la diferencia esencial entre ambos modos.
#[test]
fn reubicable_usa_auipc_y_el_normal_lui() {
    let abs = assemble(PROG).expect("ensambla normal").text;
    let reu = assemble_reubicable(PROG).expect("ensambla reubicable").text;
    assert_eq!(abs.len(), reu.len(), "misma longitud de .text");
    assert_eq!(reu.len(), 24, "la(2)+lw(2)+sw(2) = 6 palabras");

    // Las tres expansiones empiezan en 0, 8, 16 (cada una 2 palabras).
    for off in [0usize, 8, 16] {
        assert_eq!(opcode(&abs[off..off + 4]), OP_LUI, "normal: lui en {off}");
        assert_eq!(
            opcode(&reu[off..off + 4]),
            OP_AUIPC,
            "reubicable: auipc en {off}"
        );
    }
}

/// Sin toolchain: el ELF reubicable es ET_DYN; el normal, ET_EXEC.
#[test]
fn elf_reubicable_es_et_dyn_y_normal_et_exec() {
    let et = |src_prog: &str, reub: bool| -> u16 {
        let p = if reub {
            assemble_reubicable(src_prog)
        } else {
            assemble(src_prog)
        }
        .expect("ensambla");
        let elf = export_elf(&p).expect("exporta");
        u16::from_le_bytes([elf[16], elf[17]])
    };
    assert_eq!(et(PROG, false), 2, "ET_EXEC (normal)");
    assert_eq!(et(PROG, true), 3, "ET_DYN (reubicable)");
}

/// Oráculo: nuestro `.text` reubicable == el par auipc/%pcrel de GNU en el
/// mapa de EmuRV (.text 0x00400000, .data 0x10010000), byte a byte.
#[test]
fn reubicable_igual_al_pcrel_de_gnu() {
    if toolchain().is_none() {
        eprintln!("SALTEADO: toolchain GNU ausente (brew install riscv64-elf-gcc)");
        return;
    }
    let nuestro = assemble_reubicable(PROG).expect("ensambla reubicable").text;
    let gnu = gnu_text("pcrel", GNU_PCREL, TEXT_BASE, DATA_BASE);
    assert_eq!(
        nuestro, gnu,
        "el .text reubicable debe ser el par pcrel de GNU: nuestro={nuestro:02x?} gnu={gnu:02x?}"
    );
}

/// Reubicable de verdad: el MISMO objeto pcrel, enlazado en otra base
/// (desplazando .text y .data por la misma delta), da un `.text` IDÉNTICO.
/// Combinado con el test anterior, nuestro binario tiene la misma conducta
/// cargado en 0x00400000 y en una dirección distinta.
#[test]
fn pcrel_no_cambia_al_reubicar() {
    if toolchain().is_none() {
        eprintln!("SALTEADO: toolchain GNU ausente");
        return;
    }
    let base = gnu_text("pi_base", GNU_PCREL, TEXT_BASE, DATA_BASE);
    // Delta = +0x20000000 a ambos segmentos (conserva la distancia .text↔.data).
    let delta = 0x2000_0000u32;
    let movido = gnu_text(
        "pi_movido",
        GNU_PCREL,
        TEXT_BASE + delta,
        DATA_BASE + delta,
    );
    assert_eq!(
        base, movido,
        "el .text pcrel no debe cambiar al reubicar la imagen como unidad"
    );
    // Y es el mismo que producimos nosotros.
    let nuestro = assemble_reubicable(PROG).expect("ensambla reubicable").text;
    assert_eq!(nuestro, base, "nuestro .text reubicable coincide en ambas bases");
}

/// Regresión: el export normal sigue siendo el par absoluto `lui/%hi` de GNU,
/// byte a byte. El modo reubicable no tocó el camino absoluto.
#[test]
fn export_normal_sigue_absoluto() {
    if toolchain().is_none() {
        eprintln!("SALTEADO: toolchain GNU ausente");
        return;
    }
    let nuestro = assemble(PROG).expect("ensambla normal").text;
    let gnu = gnu_text("abs", GNU_ABS, TEXT_BASE, DATA_BASE);
    assert_eq!(
        nuestro, gnu,
        "el .text normal debe seguir siendo el par lui/%hi absoluto de GNU"
    );
}
