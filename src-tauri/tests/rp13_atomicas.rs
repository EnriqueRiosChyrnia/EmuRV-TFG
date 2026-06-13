//! RP13 Fase 2 — extensión A (atómicas): encodings == GNU as palabra a
//! palabra + ejecución con semántica mono-hart documentada.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::DATA_BASE;
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

/// Repertorio A completo, con y sin sufijos de ordenamiento.
const REPERTORIO: &str = "\
lr.w t0, (t2)
sc.w t0, t1, (t2)
amoswap.w t0, t1, (t2)
amoadd.w t0, t1, (t2)
amoxor.w t0, t1, (t2)
amoand.w t0, t1, (t2)
amoor.w t0, t1, (t2)
amomin.w t0, t1, (t2)
amomax.w t0, t1, (t2)
amominu.w t0, t1, (t2)
amomaxu.w t0, t1, (t2)
amoadd.w.aq t0, t1, (t2)
amoadd.w.rl t0, t1, (t2)
amoadd.w.aqrl t0, t1, (t2)
lr.w.aq t0, (t2)
sc.w.rl t0, t1, (t2)
";

fn toolchain() -> Option<(String, String)> {
    Command::new("riscv64-elf-as").arg("--version").output().ok()?;
    Command::new("riscv64-elf-objcopy").arg("--version").output().ok()?;
    Some(("riscv64-elf-as".into(), "riscv64-elf-objcopy".into()))
}

#[test]
fn repertorio_a_palabra_a_palabra_contra_gnu() {
    let nuestro = assemble(&format!(".text\n{REPERTORIO}\n"))
        .unwrap_or_else(|e| panic!("nuestro ensamblador rechazó A: {e:?}"))
        .text;

    let Some((gnu_as, objcopy)) = toolchain() else {
        eprintln!("SALTEADO: toolchain GNU no disponible (brew install riscv64-elf-gcc)");
        return;
    };
    let dir = std::env::temp_dir().join("rp13_oraculo_a");
    let _ = std::fs::create_dir_all(&dir);
    let s = dir.join("a.s");
    let o = dir.join("a.o");
    let bin = dir.join("a.bin");
    std::fs::write(&s, format!(".text\n{REPERTORIO}\n")).unwrap();

    let out = Command::new(&gnu_as)
        .args(["-march=rv32ima", "-o", o.to_str().unwrap(), s.to_str().unwrap()])
        .output()
        .expect("as");
    assert!(out.status.success(), "GNU as: {}", String::from_utf8_lossy(&out.stderr));
    Command::new(&objcopy)
        .args(["-O", "binary", "-j", ".text", o.to_str().unwrap(), bin.to_str().unwrap()])
        .output()
        .expect("objcopy");
    let gnu = std::fs::read(&bin).unwrap();

    assert_eq!(nuestro.len(), gnu.len(), "distinta cantidad de bytes");
    let lineas: Vec<&str> = REPERTORIO.lines().collect();
    for (i, (n, g)) in nuestro.chunks(4).zip(gnu.chunks(4)).enumerate() {
        let nw = u32::from_le_bytes(n.try_into().unwrap());
        let gw = u32::from_le_bytes(g.try_into().unwrap());
        assert_eq!(nw, gw, "palabra {i}: nuestra {nw:#010x} vs GNU {gw:#010x} — «{}»", lineas[i]);
    }
}

/// amoadd.w sobre una palabra de .data: la memoria se actualiza y rd trae
/// el valor VIEJO (semántica RMW atómico).
#[test]
fn amoadd_actualiza_memoria_y_devuelve_viejo() {
    let src = "
.data
contador: .word 100
.text
  la t2, contador
  li t1, 10
  amoadd.w t0, t1, (t2)
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let bytes = m.read_mem(DATA_BASE, 4).expect("leer");
    assert_eq!(u32::from_le_bytes(bytes.try_into().unwrap()), 110, "100 + 10");
    assert_eq!(m.registers().expect("regs")[5], 100, "t0 = valor viejo");
}

/// lr.w + sc.w: store-conditional EXITOSO. Mono-hart (documentado): sin
/// otro hart que invalide la reserva, sc siempre tiene éxito → t0 = 0.
#[test]
fn lr_sc_store_conditional_exitoso_mono_hart() {
    let src = "
.data
celda: .word 7
.text
  la t2, celda
  lr.w t0, (t2)       # reserva la celda
  li t1, 42
  sc.w t0, t1, (t2)   # éxito → t0 = 0, memoria = 42
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let bytes = m.read_mem(DATA_BASE, 4).expect("leer");
    assert_eq!(u32::from_le_bytes(bytes.try_into().unwrap()), 42, "sc escribió");
    assert_eq!(m.registers().expect("regs")[5], 0, "sc exitoso → 0");
}

/// amomax/amominu: el sabor con/sin signo importa. amomaxu con un valor de
/// signo alto debe ganar (lo trata como grande sin signo).
#[test]
fn amomaxu_sin_signo() {
    let src = "
.data
v: .word 5
.text
  la t2, v
  li t1, -1           # 0xFFFFFFFF: el mayor SIN signo
  amomaxu.w t0, t1, (t2)
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let bytes = m.read_mem(DATA_BASE, 4).expect("leer");
    assert_eq!(
        u32::from_le_bytes(bytes.try_into().unwrap()),
        0xFFFF_FFFF,
        "amomaxu trata -1 como el mayor sin signo"
    );
}

/// Errores didácticos: amo* sin paréntesis, lr con rs2 de más.
#[test]
fn errores_didacticos_a() {
    // amo con 3 operandos pero el último no es «(rs1)»: error de forma.
    let e = assemble(".text\namoadd.w t0, t1, t2\n").unwrap_err();
    assert!(e[0].msg.contains("rd, rs2"), "{}", e[0].msg);
    assert!(e[0].msg.contains("paréntesis"), "{}", e[0].msg);
    // offset en la dirección atómica: rechazado (la espec no lo permite).
    let e = assemble(".text\namoadd.w t0, t1, 4(t2)\n").unwrap_err();
    assert!(e[0].msg.contains("desplazamiento"), "{}", e[0].msg);
    // lr con desplazamiento: la dirección debe ser «(rs1)» pura.
    let e = assemble(".text\nlr.w t0, 4(t2)\n").unwrap_err();
    assert!(e[0].msg.contains("paréntesis"), "{}", e[0].msg);
}

/// El desensamblador (capstone arch_riscv) decodifica A — verificado.
#[test]
fn disasm_cubre_a() {
    use emurv_lib::core::disasm::{disasm, words_de};
    use emurv_lib::core::TEXT_BASE;
    let p = assemble(".text\namoadd.w t0, t1, (t2)\nlr.w t0, (t2)\nsc.w t0, t1, (t2)\n").unwrap();
    let lineas = disasm(&words_de(&p.text), TEXT_BASE);
    let texto: Vec<&str> = lineas.iter().map(|l| l.texto.as_str()).collect();
    assert!(texto.iter().any(|t| t.starts_with("amoadd.w")), "{texto:?}");
    assert!(texto.iter().any(|t| t.starts_with("lr.w")), "{texto:?}");
    assert!(texto.iter().any(|t| t.starts_with("sc.w")), "{texto:?}");
}
