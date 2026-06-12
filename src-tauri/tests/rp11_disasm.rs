//! RP11 Fase 3 — desensamblador (capstone arch_riscv, decisión del spike).
//!
//! Round-trip de mnemónicos: las MISMAS palabras que emite nuestro
//! ensamblador deben volver con su mnemónico (capstone usa alias estándar
//! para las pseudo: mv/li/j/ret aparecen como tales — se aceptan ambos).

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::disasm::{disasm, listado, words_de};
use emurv_lib::core::elf_export::export_elf;
use emurv_lib::core::elf_import::import_elf;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::TEXT_BASE;
use std::sync::mpsc::channel;

/// El repertorio RV32IM+Zicsr completo (una instrucción por línea del
/// fuente; li/la expanden a más de una palabra y está bien). Cada palabra
/// debe decodificar al mnemónico esperado O a un alias conocido.
#[test]
fn repertorio_propio_round_trip_de_mnemonicos() {
    // (fuente, mnemónicos aceptados para SU primera palabra)
    let casos: &[(&str, &[&str])] = &[
        ("add t0, t1, t2", &["add"]),
        ("sub t0, t1, t2", &["sub"]),
        ("sll t0, t1, t2", &["sll"]),
        ("slt t0, t1, t2", &["slt"]),
        ("sltu t0, t1, t2", &["sltu"]),
        ("xor t0, t1, t2", &["xor"]),
        ("srl t0, t1, t2", &["srl"]),
        ("sra t0, t1, t2", &["sra"]),
        ("or t0, t1, t2", &["or"]),
        ("and t0, t1, t2", &["and"]),
        ("addi t0, t1, 5", &["addi"]),
        ("slti t0, t1, 5", &["slti"]),
        ("sltiu t0, t1, 5", &["sltiu"]),
        ("xori t0, t1, 5", &["xori"]),
        ("ori t0, t1, 5", &["ori"]),
        ("andi t0, t1, 5", &["andi"]),
        ("slli t0, t1, 5", &["slli"]),
        ("srli t0, t1, 5", &["srli"]),
        ("srai t0, t1, 5", &["srai"]),
        ("lui t0, 0x12345", &["lui"]),
        ("auipc t0, 1", &["auipc"]),
        ("lb t0, 0(t1)", &["lb"]),
        ("lh t0, 0(t1)", &["lh"]),
        ("lw t0, 0(t1)", &["lw"]),
        ("lbu t0, 0(t1)", &["lbu"]),
        ("lhu t0, 0(t1)", &["lhu"]),
        ("sb t0, 0(t1)", &["sb"]),
        ("sh t0, 0(t1)", &["sh"]),
        ("sw t0, 0(t1)", &["sw"]),
        ("beq t0, t1, .+8", &["beq"]),
        ("bne t0, t1, .+8", &["bne", "bnez"]),
        ("blt t0, t1, .+8", &["blt"]),
        ("bge t0, t1, .+8", &["bge"]),
        ("bltu t0, t1, .+8", &["bltu"]),
        ("bgeu t0, t1, .+8", &["bgeu"]),
        ("jal ra, .+8", &["jal"]),
        ("jalr ra, t0, 0", &["jalr", "jr"]),
        ("mul t0, t1, t2", &["mul"]),
        ("mulh t0, t1, t2", &["mulh"]),
        ("mulhsu t0, t1, t2", &["mulhsu"]),
        ("mulhu t0, t1, t2", &["mulhu"]),
        ("div t0, t1, t2", &["div"]),
        ("divu t0, t1, t2", &["divu"]),
        ("rem t0, t1, t2", &["rem"]),
        ("remu t0, t1, t2", &["remu"]),
        ("ecall", &["ecall"]),
        ("ebreak", &["ebreak"]),
        ("csrrw t0, mtvec, t1", &["csrrw", "csrw"]),
        ("csrrs t0, mcause, t1", &["csrrs", "csrr", "csrs"]),
        ("csrrc t0, mstatus, t1", &["csrrc", "csrc"]),
        ("csrrwi t0, mtvec, 5", &["csrrwi", "csrwi"]),
        ("mret", &["mret"]),
        ("nop", &["nop", "addi"]),
        ("mv t0, t1", &["mv", "addi"]),
    ];
    for (fuente, esperados) in casos {
        // `.+8` no es sintaxis nuestra: usar una etiqueta real adelante.
        let src = if fuente.contains(".+8") {
            format!(".text\n{}\nnop\ndestino:\n  nop\n", fuente.replace(".+8", "destino"))
        } else {
            format!(".text\n{fuente}\n")
        };
        let p = assemble(&src).unwrap_or_else(|e| panic!("«{fuente}» no ensambla: {e:?}"));
        let lineas = disasm(&words_de(&p.text), TEXT_BASE);
        let mn = lineas[0].texto.split_whitespace().next().unwrap_or("");
        assert!(
            esperados.contains(&mn),
            "«{fuente}» decodificó como «{}» (esperaba uno de {esperados:?})",
            lineas[0].texto
        );
    }
}

/// Palabra inválida → `.word 0x...` sin pánico, y las VECINAS se siguen
/// decodificando (el listado no se corta en la basura).
#[test]
fn palabra_invalida_es_word_sin_panico() {
    let palabras = [0x0000_0013u32, 0xFFFF_FFFF, 0x0000_0000, 0x0010_0073];
    let lineas = disasm(&palabras, TEXT_BASE);
    assert_eq!(lineas.len(), 4);
    assert_eq!(lineas[0].texto, "nop");
    assert_eq!(lineas[1].texto, ".word 0xFFFFFFFF");
    assert_eq!(lineas[2].texto, ".word 0x00000000");
    assert_eq!(lineas[3].texto, "ebreak");
    assert_eq!(lineas[3].addr, TEXT_BASE + 12, "direcciones correlativas");
}

/// Modo binario de punta a punta: cada línea del listado es una dirección;
/// un breakpoint en la dirección de la línea 3 frena EXACTAMENTE ahí.
#[test]
fn breakpoint_en_linea_del_listado_frena_en_su_direccion() {
    let src = ".text\n  li t0, 1\n  li t1, 2\n  li t2, 3\n  li a7, 10\n  ecall\n";
    let bytes = export_elf(&assemble(src).unwrap()).unwrap();
    let p = import_elf(&bytes).expect("importa");
    let lineas = disasm(&words_de(&p.text), TEXT_BASE);
    assert!(!listado(&lineas).is_empty());
    // «línea 3 del listado» = lineas[2].
    let addr_linea_3 = lineas[2].addr;

    let (out_tx, _o) = channel::<ConsoleEvent>();
    let (_i, in_rx) = channel::<InputMsg>();
    let mut m = Machine::new(p, out_tx, in_rx).expect("máquina");
    m.set_breakpoints([addr_linea_3]).expect("bp");
    let r = m.run_slice(0, 100_000);
    assert_eq!(
        r,
        StopReason::Breakpoint { addr: addr_linea_3 },
        "frena en la dirección de la línea 3 del listado"
    );
    let regs = m.registers().expect("regs");
    assert_eq!(regs[5], 1, "t0 ya ejecutado");
    assert_eq!(regs[7], 0, "t2 todavía no");
}
