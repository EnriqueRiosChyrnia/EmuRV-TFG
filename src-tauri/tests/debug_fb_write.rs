use emurv_lib::core::{assembler::assemble, machine::Machine};
use std::sync::mpsc::channel;

#[test]
fn debug_fb_write_simple() {
    let src = r#"
.text
  li t0, 0x9000_0000
  li t1, 42
  sb t1, 0(t0)
  li a7, 10
  ecall
"#;

    let program = assemble(src).expect("ensamblar");
    let (tx, _) = channel();
    let (_, rx) = channel();
    let mut m = Machine::new(program, tx, rx).expect("máquina");
    let _ = m.run(0);

    let fb_data = m.read_mem(0x9000_0000, 1).expect("leer FB");
    eprintln!("Framebuffer[0] = {}", fb_data[0]);
    assert_eq!(fb_data[0], 42, "debe escribir 42 en framebuffer[0]");
}
