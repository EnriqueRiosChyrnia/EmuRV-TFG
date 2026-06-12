//! SPIKE RP11-F1 (rama descartable): ¿capstone desensambla nuestro RV32IM
//! y cuánto pesa? Este archivo NO se mergea.

use capstone::prelude::*;
use emurv_lib::core::assembler::assemble;

#[test]
fn capstone_desensambla_10_palabras_propias() {
    let src = "
.data
v: .word 7
.text
main:
  li   t0, 42
  la   t1, v
  lw   t2, 0(t1)
  add  t3, t0, t2
  sub  t4, t3, t0
  mul  t5, t0, t2
  beq  t0, t2, main
  jal  ra, main
  csrrw t6, mtvec, t0
  ecall
";
    let p = assemble(src).expect("ensambla");
    let cs = Capstone::new()
        .riscv()
        .mode(arch::riscv::ArchMode::RiscV32)
        .build()
        .expect("capstone riscv32");
    let insns = cs
        .disasm_all(&p.text, 0x0040_0000)
        .expect("desensamblar");
    for i in insns.iter() {
        println!("{:#010x}: {} {}", i.address(), i.mnemonic().unwrap_or("?"), i.op_str().unwrap_or(""));
    }
    // li/la expanden: el conteo de instrucciones debe coincidir con .text/4
    assert_eq!(insns.len(), p.text.len() / 4, "decodificó TODAS las palabras");
    let mnems: Vec<&str> = insns.iter().map(|i| i.mnemonic().unwrap()).collect();
    for esperado in ["addi", "lw", "add", "sub", "mul", "beq", "jal", "csrrw", "ecall"] {
        assert!(mnems.contains(&esperado), "falta {esperado} en {mnems:?}");
    }
}
