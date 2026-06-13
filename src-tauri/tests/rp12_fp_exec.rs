//! RP12 Fase 2 — ejecución F/D con bits IEEE 754 exactos (valores con
//! representación finita: 1.5, 0.25… — sin pelear con redondeo) y los
//! errores didácticos del repertorio nuevo.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::DATA_BASE;
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

/// flw + fadd.s + fsw: el resultado en MEMORIA tiene los bits exactos de
/// 1.75f32 (y de paso prueba que load() inicializa mstatus.FS — sin eso,
/// la primera flw cortaría con instrucción ilegal).
#[test]
fn fadd_s_bits_exactos_y_fs_inicializado() {
    let src = "
.data
a: .word 0x3FC00000   # 1.5f32
b: .word 0x3E800000   # 0.25f32
r: .word 0
.text
  la t0, a
  flw ft0, 0(t0)
  flw ft1, 4(t0)
  fadd.s ft2, ft0, ft1
  fsw ft2, 8(t0)
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let bytes = m.read_mem(DATA_BASE + 8, 4).expect("leer r");
    let bits = u32::from_le_bytes(bytes.try_into().unwrap());
    assert_eq!(f32::from_bits(bits), 1.75f32, "1.5 + 0.25 exacto");
    // Y el registro f2 visible por registers_f (flw boxeó las entradas).
    let f = m.registers_f().expect("registros f");
    assert_eq!(f[0] >> 32, 0xFFFF_FFFF, "ft0 NaN-boxed tras flw");
    assert_eq!(f[2] as u32, 1.75f32.to_bits(), "ft2 = resultado");
}

/// Doble precisión: fld/fadd.d/fsd con bits exactos de f64; flw/fsd con
/// etiqueta (expansión lui+acceso con auxiliar entero).
#[test]
fn fadd_d_y_acceso_con_etiqueta() {
    let src = "
.data
a: .word 0x00000000, 0x3FF80000   # 1.5f64 (little-endian: bajos primero)
b: .word 0x00000000, 0x3FD00000   # 0.25f64
r: .word 0, 0
.text
  fld ft0, a, t0      # forma con etiqueta + auxiliar
  fld ft1, b, t0
  fadd.d ft2, ft0, ft1
  fsd ft2, r, t1
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let bytes = m.read_mem(DATA_BASE + 16, 8).expect("leer r");
    let bits = u64::from_le_bytes(bytes.try_into().unwrap());
    assert_eq!(f64::from_bits(bits), 1.75f64, "1.5 + 0.25 exacto en f64");
}

/// Conversiones y comparación: int→float→int y feq devuelve 0/1 en
/// registro ENTERO.
#[test]
fn fcvt_y_feq() {
    let src = "
.text
  li t0, 7
  fcvt.s.w ft0, t0      # 7 → 7.0f32 (exacto)
  fcvt.w.s t1, ft0      # 7.0 → 7
  fcvt.d.w ft1, t0      # 7 → 7.0f64
  fcvt.s.d ft2, ft1     # 7.0f64 → 7.0f32
  feq.s t2, ft0, ft2    # iguales → 1
  flt.s t3, ft0, ft2    # no es menor → 0
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let regs = m.registers().expect("regs");
    assert_eq!(regs[6], 7, "t1: round-trip 7 → 7.0 → 7");
    assert_eq!(regs[7], 1, "t2: feq = 1");
    assert_eq!(regs[28], 0, "t3: flt = 0");
    let f = m.registers_f().expect("f");
    assert_eq!(f[1], 7.0f64.to_bits(), "ft1 = 7.0 en f64 crudo");
}

/// fmadd y sign-inject: ft3 = ft0*ft1 + ft2; fneg/fabs por sgnj.
#[test]
fn fmadd_y_pseudos_sgnj() {
    let src = "
.data
a: .word 0x40000000   # 2.0
b: .word 0x40400000   # 3.0
c: .word 0x3F000000   # 0.5
.text
  la t0, a
  flw ft0, 0(t0)
  flw ft1, 4(t0)
  flw ft2, 8(t0)
  fmadd.s ft3, ft0, ft1, ft2   # 2*3 + 0.5 = 6.5
  fneg.s ft4, ft3              # -6.5
  fabs.s ft5, ft4              # 6.5
  li a7, 10
  ecall
";
    let mut m = machine_for(src);
    assert!(matches!(correr(&mut m), StopReason::Exited { code: 0 }));
    let f = m.registers_f().expect("f");
    assert_eq!(f[3] as u32, 6.5f32.to_bits(), "fmadd exacto");
    assert_eq!(f[4] as u32, (-6.5f32).to_bits(), "fneg");
    assert_eq!(f[5] as u32, 6.5f32.to_bits(), "fabs");
}

/// Errores didácticos del plan, uno por uno.
#[test]
fn errores_didacticos_fd() {
    // registro entero donde va f
    let e = assemble(".text\nfadd.s t0, t1, t2\n").unwrap_err();
    assert!(e[0].msg.contains("registros f"), "{}", e[0].msg);
    // registro f donde va entero
    let e = assemble(".text\nadd ft0, ft1, ft2\n").unwrap_err();
    assert!(e[0].msg.contains("registros"), "{}", e[0].msg);
    // fmv.x.d no existe en RV32
    let e = assemble(".text\nfmv.x.d t0, ft0\n").unwrap_err();
    assert!(e[0].msg.contains("RV32"), "{}", e[0].msg);
    assert!(e[0].msg.contains("fsd"), "el error sugiere el camino: {}", e[0].msg);
    // modo de redondeo explícito
    let e = assemble(".text\nfadd.s ft0, ft1, ft2, rtz\n").unwrap_err();
    assert!(e[0].msg.contains("redondeo"), "{}", e[0].msg);
    assert!(e[0].msg.contains("dyn"), "{}", e[0].msg);
    // fcvt con destino equivocado
    let e = assemble(".text\nfcvt.w.s ft0, ft1\n").unwrap_err();
    assert!(e[0].msg.contains("ENTERO"), "{}", e[0].msg);
}
