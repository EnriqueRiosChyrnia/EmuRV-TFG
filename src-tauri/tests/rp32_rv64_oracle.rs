//! RP32 Fase 3 — EL criterio de la compuerta del ensamblador RV64: el
//! repertorio de 64 bits ensamblado por NOSOTROS (`assemble_with_xlen`,
//! `Xlen::Rv64`) y por GNU `as -march=rv64im -mabi=lp64` da las MISMAS
//! palabras, una por una. Mismo patrón que tests/rp12_gnu_oracle.rs (la
//! toolchain `riscv64-elf-*` quedó instalada en RP-11; si no está, el test
//! se saltea con aviso).
//!
//! Además, sin toolchain: la conmutación de repertorio (RV64 ilegal en RV32 y
//! viceversa del shamt) y que RV32 ensambla IDÉNTICO con y sin el parámetro.

use emurv_lib::core::assembler::{assemble, assemble_with_xlen};
use emurv_lib::core::Xlen;
use std::process::Command;

/// Una instrucción RV64 real por línea (sin pseudos de varias palabras): el
/// oráculo compara encoding puro. Incluye las W (OP-32/OP-IMM-32), ld/lwu/sd,
/// los shift con shamt de 6 bits (solo válidos en RV64) y, al final, un par
/// RV32I compartido para confirmar que NO cambian en modo 64.
const REPERTORIO_RV64: &str = "\
ld t0, 0(t1)
ld a0, 16(sp)
ld s0, -8(s1)
lwu t2, 8(t0)
lwu a1, 2047(a2)
sd t0, 24(t1)
sd a1, 0(sp)
sd s2, -16(gp)
addiw t0, t1, 5
addiw a0, a1, -1
addiw s0, s0, 2047
slliw t0, t1, 3
slliw a0, a1, 0
srliw t2, t0, 7
sraiw a0, a1, 31
addw t0, t1, t2
subw a0, a1, a2
sllw t0, t1, t2
srlw a0, a1, a2
sraw t2, t0, t1
mulw t0, t1, t2
divw a0, a1, a2
divuw t2, t0, t1
remw t0, t1, t2
remuw a0, a1, a2
slli t0, t1, 40
slli a0, a1, 63
srli t2, t0, 50
srai a0, a1, 33
add t0, t1, t2
addi a0, a1, 7
";

fn toolchain() -> Option<(String, String)> {
    let as_ = "riscv64-elf-as";
    let objcopy = "riscv64-elf-objcopy";
    Command::new(as_).arg("--version").output().ok()?;
    Command::new(objcopy).arg("--version").output().ok()?;
    Some((as_.into(), objcopy.into()))
}

#[test]
fn repertorio_rv64im_palabra_a_palabra_contra_gnu() {
    let fuente = REPERTORIO_RV64.to_string();

    let nuestro = assemble_with_xlen(&format!(".text\n{fuente}\n"), Xlen::Rv64)
        .unwrap_or_else(|e| panic!("nuestro ensamblador rechazó el repertorio RV64: {e:?}"))
        .text;

    let Some((gnu_as, objcopy)) = toolchain() else {
        eprintln!(
            "SALTEADO: riscv64-elf-as/objcopy no están en este equipo \
             (brew install riscv64-elf-gcc); el oráculo corre donde estén"
        );
        return;
    };

    let dir = std::env::temp_dir().join("rp32_oraculo");
    let _ = std::fs::create_dir_all(&dir);
    let s_path = dir.join("rep.s");
    let o_path = dir.join("rep.o");
    let bin_path = dir.join("rep.bin");
    std::fs::write(&s_path, format!(".text\n{fuente}\n")).expect("escribir fuente");

    let out = Command::new(&gnu_as)
        .args([
            "-march=rv64im",
            "-mabi=lp64",
            "-o",
            o_path.to_str().unwrap(),
            s_path.to_str().unwrap(),
        ])
        .output()
        .expect("lanzar as");
    assert!(
        out.status.success(),
        "GNU as rechazó el repertorio: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new(&objcopy)
        .args([
            "-O",
            "binary",
            "-j",
            ".text",
            o_path.to_str().unwrap(),
            bin_path.to_str().unwrap(),
        ])
        .output()
        .expect("lanzar objcopy");
    assert!(out.status.success(), "objcopy falló");

    let gnu = std::fs::read(&bin_path).expect("leer binario GNU");
    assert_eq!(
        nuestro.len(),
        gnu.len(),
        "cantidades distintas de bytes: nuestro={} gnu={}",
        nuestro.len(),
        gnu.len()
    );
    let lineas: Vec<&str> = fuente.lines().collect();
    for (i, (n, g)) in nuestro.chunks(4).zip(gnu.chunks(4)).enumerate() {
        let nw = u32::from_le_bytes(n.try_into().unwrap());
        let gw = u32::from_le_bytes(g.try_into().unwrap());
        assert_eq!(
            nw, gw,
            "palabra {i} difiere: nuestra {nw:#010x} vs GNU {gw:#010x} — «{}»",
            lineas.get(i).unwrap_or(&"?")
        );
    }
}

// ---- Conmutación del repertorio (sin toolchain) ----

/// Una instrucción RV64 en un programa RV32 es un error didáctico (no
/// «instrucción desconocida»): el mnemónico existe, pero no en ese ancho.
#[test]
fn rv64_only_rechazada_en_rv32() {
    for m in ["addiw t0, t1, 1", "addw t0, t1, t2", "ld t0, 0(t1)", "sd t0, 0(t1)"] {
        let err = assemble_with_xlen(&format!(".text\n{m}\n"), Xlen::Rv32)
            .expect_err(&format!("«{m}» debía rechazarse en RV32"));
        let msg = &err[0].msg;
        assert!(
            msg.contains("RV64") && !msg.contains("desconocida"),
            "mensaje didáctico esperado para «{m}», hubo: {msg}"
        );
    }
}

/// Las mismas instrucciones SÍ ensamblan en RV64 (4 bytes cada una).
#[test]
fn rv64_only_aceptada_en_rv64() {
    for m in ["addiw t0, t1, 1", "addw t0, t1, t2", "ld t0, 0(t1)", "sd t0, 0(t1)"] {
        let p = assemble_with_xlen(&format!(".text\n{m}\n"), Xlen::Rv64)
            .unwrap_or_else(|e| panic!("«{m}» debía ensamblar en RV64: {e:?}"));
        assert_eq!(p.text.len(), 4, "«{m}» es una palabra de 4 bytes");
        assert_eq!(p.xlen, Xlen::Rv64);
    }
}

/// El shamt de 6 bits (32..63) solo es válido en RV64: en RV32 da error de
/// rango; en RV64 ensambla.
#[test]
fn shamt_de_6_bits_segun_ancho() {
    let err = assemble_with_xlen(".text\nslli t0, t1, 40\n", Xlen::Rv32)
        .expect_err("slli shamt=40 debía rechazarse en RV32");
    assert!(err[0].msg.contains("0 y 31"), "rango RV32: {}", err[0].msg);

    let p = assemble_with_xlen(".text\nslli t0, t1, 40\n", Xlen::Rv64)
        .expect("slli shamt=40 es válido en RV64");
    assert_eq!(p.text.len(), 4);
}

/// REGRESIÓN: un programa RV32 ensambla EXACTAMENTE igual con `assemble`
/// (camino histórico) y con `assemble_with_xlen(.., Rv32)` (camino nuevo).
#[test]
fn rv32_identico_con_y_sin_parametro() {
    let src = ".data\nv: .word 7\n.text\nli t0, 5\nla t1, v\nlw t2, 0(t1)\nadd a0, t0, t2\nslli a0, a0, 3\nli a7, 10\necall\n";
    let viejo = assemble(src).expect("camino histórico");
    let nuevo = assemble_with_xlen(src, Xlen::Rv32).expect("camino nuevo");
    assert_eq!(viejo.text, nuevo.text, "el .text RV32 no debe cambiar");
    assert_eq!(viejo.data, nuevo.data, "el .data RV32 no debe cambiar");
    assert_eq!(nuevo.xlen, Xlen::Rv32);
}
