//! RP12 Fase 2 — EL criterio de la compuerta: el repertorio F/D completo
//! ensamblado por NOSOTROS y por GNU as (-march=rv32imfd) da las MISMAS
//! palabras, una por una. La toolchain quedó instalada en RP-11 (brew
//! riscv64-elf-gcc); si no está, el test se saltea con aviso (la CI macOS
//! la tiene; la matriz experimental no — documentado en el plan).

use emurv_lib::core::assembler::assemble;
use std::process::Command;

/// El repertorio completo, UNA instrucción real por línea (sin pseudos de
/// varias palabras ni etiquetas: el oráculo compara encoding puro; los
/// pseudos de un word —fmv/fabs/fneg— SÍ entran porque GNU también los
/// acepta y deben dar el mismo sgnj).
const REPERTORIO: &str = "\
flw ft0, 0(t0)
flw fa0, -4(sp)
fld ft1, 8(t1)
fsw ft0, 12(t0)
fsd ft1, 16(t2)
fadd.s ft0, ft1, ft2
fsub.s fs0, fa0, fa1
fmul.s ft3, ft4, ft5
fdiv.s ft6, ft7, fs1
fsqrt.s ft0, ft1
fmin.s ft0, ft1, ft2
fmax.s ft0, ft1, ft2
fsgnj.s ft0, ft1, ft2
fsgnjn.s ft0, ft1, ft2
fsgnjx.s ft0, ft1, ft2
fmadd.s ft0, ft1, ft2, ft3
fmsub.s ft0, ft1, ft2, ft3
fnmadd.s ft0, ft1, ft2, ft3
fnmsub.s ft0, ft1, ft2, ft3
fcvt.w.s t0, ft0
fcvt.wu.s t1, ft1
fcvt.s.w ft0, t0
fcvt.s.wu ft1, t1
fmv.x.w t0, ft0
fmv.w.x ft0, t0
feq.s t0, ft0, ft1
flt.s t1, ft1, ft2
fle.s t2, ft2, ft3
fclass.s t0, ft0
fadd.d ft0, ft1, ft2
fsub.d fs2, fa2, fa3
fmul.d ft3, ft4, ft5
fdiv.d ft6, ft7, fs3
fsqrt.d ft0, ft1
fmin.d ft0, ft1, ft2
fmax.d ft0, ft1, ft2
fsgnj.d ft0, ft1, ft2
fsgnjn.d ft0, ft1, ft2
fsgnjx.d ft0, ft1, ft2
fmadd.d ft0, ft1, ft2, ft3
fmsub.d ft0, ft1, ft2, ft3
fnmadd.d ft0, ft1, ft2, ft3
fnmsub.d ft0, ft1, ft2, ft3
fcvt.s.d ft0, ft1
fcvt.d.s ft1, ft0
fcvt.w.d t0, ft0
fcvt.wu.d t1, ft1
fcvt.d.w ft0, t0
fcvt.d.wu ft1, t1
feq.d t0, ft0, ft1
flt.d t1, ft1, ft2
fle.d t2, ft2, ft3
fclass.d t0, ft0
fmv.s ft0, ft1
fabs.s ft0, ft1
fneg.s ft0, ft1
fmv.d ft0, ft1
fabs.d ft0, ft1
fneg.d ft0, ft1
fadd.s ft11, fs11, fa7
";

fn toolchain() -> Option<(String, String)> {
    let as_ = "riscv64-elf-as";
    let objcopy = "riscv64-elf-objcopy";
    Command::new(as_).arg("--version").output().ok()?;
    Command::new(objcopy).arg("--version").output().ok()?;
    Some((as_.into(), objcopy.into()))
}

#[test]
fn repertorio_fd_palabra_a_palabra_contra_gnu() {
    let fuente = REPERTORIO.to_string();

    let nuestro = assemble(&format!(".text\n{fuente}\n"))
        .unwrap_or_else(|e| panic!("nuestro ensamblador rechazó el repertorio: {e:?}"))
        .text;

    let Some((gnu_as, objcopy)) = toolchain() else {
        eprintln!(
            "SALTEADO: riscv64-elf-as/objcopy no están en este equipo \
             (brew install riscv64-elf-gcc); el oráculo corre donde estén"
        );
        return;
    };

    let dir = std::env::temp_dir().join("rp12_oraculo");
    let _ = std::fs::create_dir_all(&dir);
    let s_path = dir.join("rep.s");
    let o_path = dir.join("rep.o");
    let bin_path = dir.join("rep.bin");
    std::fs::write(&s_path, format!(".text\n{fuente}\n")).expect("escribir fuente");

    let out = Command::new(&gnu_as)
        .args([
            "-march=rv32imfd",
            "-mabi=ilp32d",
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
