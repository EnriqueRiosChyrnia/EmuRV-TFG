//! RP29 Fase 1 — spike de viabilidad del «ELF reubicable» (RP-29, Cap. VI
//! §6.2.3). Patrón de los spikes del proyecto (spike_csr/spike_mmio): PoC con
//! evidencia ANTES de comprometer arquitectura. La toolchain GNU
//! (riscv64-elf-as/ld/objcopy) es el ORÁCULO; si no está, el test se saltea
//! con aviso (la CI macOS la tiene — ver rp12_gnu_oracle.rs).
//!
//! Pregunta del spike: ¿cuál es la vía más limpia para exportar un ELF cuyo
//! CÓDIGO no dependa de cargarse en una dirección fija?
//!
//!   Vía 1 — el ensamblador emite las referencias a símbolos como
//!     `auipc rd, %pcrel_hi(sym)` + `addi rd, rd, %pcrel_lo` (PC-relativo
//!     estándar RISC-V), codificando `sym - pc` en vez de la dirección
//!     absoluta. Autocontenida: no necesita un linker externo en runtime.
//!   Vía 2 — emitir relocations en el ELF (R_RISCV_PCREL_HI20/LO12) y que un
//!     linker las resuelva (más cercano a un .o que a un ejecutable).
//!
//! Este spike DEMUESTRA con el oráculo que:
//!
//!   A) El par `auipc/%pcrel` es genuinamente independiente de la posición:
//!      enlazado en dos bases distintas (manteniendo la distancia .text↔.data)
//!      produce el MISMO .text byte a byte; el par absoluto `lui/%hi` NO.
//!   B) Nuestras primitivas YA existentes (codificación de `auipc` +
//!      helpers `hi20`/`lo12`) reproducen el par pcrel resuelto por GNU byte a
//!      byte: la única diferencia con el modo absoluto es alimentar
//!      `sym - pc` en vez de `sym`. → Vía 1 es barata y autocontenida.
//!
//! Conclusión registrada en BITACORA [Seguro]: se elige la Vía 1.

use emurv_lib::core::assembler::assemble;
use std::path::PathBuf;
use std::process::Command;

/// `(as, ld, objcopy)` si la toolchain GNU RISC-V está disponible.
fn toolchain() -> Option<(String, String, String)> {
    let (as_, ld, objcopy) = ("riscv64-elf-as", "riscv64-elf-ld", "riscv64-elf-objcopy");
    Command::new(as_).arg("--version").output().ok()?;
    Command::new(ld).arg("--version").output().ok()?;
    Command::new(objcopy).arg("--version").output().ok()?;
    Some((as_.into(), ld.into(), objcopy.into()))
}

/// Directorio de trabajo del spike (único por etiqueta para no pisar otros
/// tests al correr en paralelo).
fn dir(label: &str) -> PathBuf {
    let d = std::env::temp_dir().join("rp29_reubicable").join(label);
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

/// Ensambla `src` con GNU (sin relax, para obtener el par auipc+addi canónico
/// y no la fusión gp-relativa del linker) y lo ENLAZA con la base de .text y
/// .data dadas mediante `--section-start`. Devuelve los bytes de `.text`.
fn gnu_text(label: &str, src: &str, text_base: u32, data_base: u32) -> Vec<u8> {
    let (as_, ld, objcopy) = toolchain().expect("toolchain verificada por el llamador");
    let d = dir(label);
    let s = d.join("in.s");
    let o = d.join("in.o");
    let elf = d.join("out.elf");
    let bin = d.join("out.bin");
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

/// Fuente PC-relativa: `la a0, msj` expandido a auipc/%pcrel + addi/%pcrel.
/// `_start` coincide con el auipc (entry, evita el warning de ld).
const SRC_PCREL: &str = "\
.text
.globl _start
_start:
1:  auipc a0, %pcrel_hi(msj)
    addi  a0, a0, %pcrel_lo(1b)
.data
msj: .word 0
";

/// Fuente ABSOLUTA: el par lui/%hi + addi/%lo de hoy (modo no reubicable).
const SRC_ABS: &str = "\
.text
.globl _start
_start:
    lui   a0, %hi(msj)
    addi  a0, a0, %lo(msj)
.data
msj: .word 0
";

/// A) El par auipc/%pcrel es reubicable; el par lui/%hi absoluto NO.
///
/// Se enlaza CADA fuente en dos bases distintas conservando la distancia
/// .text↔.data (0x1000). Position-independent ⇔ el .text no cambia.
#[test]
fn pcrel_es_reubicable_y_el_absoluto_no() {
    if toolchain().is_none() {
        eprintln!(
            "SALTEADO: riscv64-elf-as/ld/objcopy no están en este equipo \
             (brew install riscv64-elf-gcc); el oráculo corre donde estén"
        );
        return;
    }

    // Base baja y base alta, mismo hueco .text↔.data.
    let pcrel_a = gnu_text("pcrel_a", SRC_PCREL, 0x0001_0000, 0x0001_1000);
    let pcrel_b = gnu_text("pcrel_b", SRC_PCREL, 0x8000_0000, 0x8000_1000);
    assert_eq!(
        pcrel_a, pcrel_b,
        "el .text PC-relativo debe ser IDÉNTICO en ambas bases (reubicable): \
         a={pcrel_a:02x?} b={pcrel_b:02x?}"
    );

    let abs_a = gnu_text("abs_a", SRC_ABS, 0x0001_0000, 0x0001_1000);
    let abs_b = gnu_text("abs_b", SRC_ABS, 0x8000_0000, 0x8000_1000);
    assert_ne!(
        abs_a, abs_b,
        "el .text absoluto (lui/%hi) DEBE cambiar con la base (no reubicable): \
         a={abs_a:02x?} b={abs_b:02x?}"
    );
}

/// Compone el par pcrel con las primitivas del proyecto: ésta es exactamente
/// la matemática del modo absoluto (helpers hi20/lo12), pero alimentada con
/// `sym - pc` en vez de `sym`. Replicada acá para documentar lo que el spike
/// valida (los helpers son privados del ensamblador).
fn par_pcrel_nuestro(offset: u32) -> Vec<u8> {
    let hi = offset.wrapping_add(0x800) >> 12; // %pcrel_hi: 20 bits altos con carry
    let lo = ((offset as i32) << 20) >> 20; // %pcrel_lo: 12 bits bajos con signo
    let src = format!("auipc a0, {hi}\naddi a0, a0, {lo}\n");
    assemble(&src)
        .unwrap_or_else(|e| panic!("nuestro ensamblador rechazó «{src}»: {e:?}"))
        .text
}

/// B) Nuestras primitivas reproducen el par pcrel resuelto por GNU, byte a
/// byte. Layout controlado: auipc en `text_base`, dato en `data_base`, así
/// `offset = data_base - text_base` es exactamente el hueco elegido (con
/// acarreo y %lo negativo para ejercitar la compensación de signo).
#[test]
fn nuestro_auipc_addi_reproduce_el_pcrel_de_gnu() {
    if toolchain().is_none() {
        eprintln!("SALTEADO: toolchain GNU ausente (ver el otro test del spike)");
        return;
    }

    // Huecos múltiplos de 4 (ld respeta la alineación de .data, así que un
    // hueco no alineado movería el dato y rompería la igualdad de offset):
    //   0x1000 %lo=0 · 0x2234 %lo=+0x234 con acarreo · 0x1800 %lo=-2048 ·
    //   0x2ffc %lo=-4 con acarreo. Cubre signo y carry de la compensación.
    for gap in [0x1000u32, 0x2234, 0x1800, 0x2ffc] {
        let text_base = 0x0001_0000u32;
        let data_base = text_base + gap;
        let gnu = gnu_text(&format!("gap_{gap:x}"), SRC_PCREL, text_base, data_base);
        let nuestro = par_pcrel_nuestro(gap);
        assert_eq!(
            nuestro, gnu,
            "par pcrel difiere para offset {gap:#x}: nuestro={nuestro:02x?} gnu={gnu:02x?}"
        );
    }
}
