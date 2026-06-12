//! SPIKE RP11-F1: PoC de lectura ELF con object (read_core).
use emurv_lib::core::assembler::assemble;
use emurv_lib::core::elf_export::export_elf;
use object::{Object, ObjectSection, ObjectSegment, ObjectSymbol};

#[test]
fn object_lee_nuestro_elf_round_trip() {
    let p = assemble(".data\nv: .word 7\n.text\nmain:\n  li a7, 10\n  ecall\n").unwrap();
    let bytes = export_elf(&p).unwrap();
    let file = object::File::parse(&*bytes).expect("parsear");
    println!("formato: {:?} arch: {:?} entry: {:#x}", file.format(), file.architecture(), file.entry());
    for seg in file.segments() {
        println!("segmento: vaddr {:#x} tam {} ", seg.address(), seg.size());
    }
    for sym in file.symbols() {
        println!("símbolo: {:?} = {:#x}", sym.name(), sym.address());
    }
    assert_eq!(file.entry(), 0x0040_0000);
    assert!(file.symbols().count() >= 2, "main y v del .symtab");
}
