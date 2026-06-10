// Test FB0: mapa de memoria gráfica
// Verifica que las regiones de video están mapeadas, son R/W y no solapan.

use emurv_lib::core::{
    machine::Machine,
    assembler::assemble,
    FRAMEBUFFER_BASE, FRAMEBUFFER_SIZE, FRAMEBUFFER_DATA_SIZE,
    PALETTE_BASE, PALETTE_SIZE, PALETTE_DATA_SIZE,
    VIDEO_CONTROL_BASE, VIDEO_CONTROL_SIZE,
    TEXT_BASE, TEXT_SIZE,
    DATA_REGION_BASE, DATA_REGION_SIZE,
    STACK_TOP, STACK_SIZE,
};
use std::sync::mpsc;

fn create_machine() -> Machine<'static> {
    let src = ".text\n  nop";
    let program = assemble(src).expect("programa mínimo debe ser válido");
    let (tx, _rx) = mpsc::channel();
    let (_, input_rx) = mpsc::channel();
    Machine::new(program, tx, input_rx).expect("máquina debe crearse")
}

#[test]
fn fb0_regiones_existen_y_son_rw() {
    let mut m = create_machine();

    // Test framebuffer: escribir y leer
    let pattern: u8 = 0xAB;
    for offset in [0, 100, 1000, FRAMEBUFFER_DATA_SIZE - 1] {
        let addr = (FRAMEBUFFER_BASE + offset) as u64;
        m.write_u8(addr, pattern).expect("escribir en FB debe funcionar");
        let val = m.read_u8(addr).expect("leer en FB debe funcionar");
        assert_eq!(val, pattern, "dato leído debe coincidir en offset {}", offset);
    }

    // Test paleta: escribir y leer
    let pattern: u8 = 0x12;
    for offset in [0, 10, 100, 500, PALETTE_DATA_SIZE - 1] {
        let addr = (PALETTE_BASE + offset) as u64;
        m.write_u8(addr, pattern).expect("escribir en paleta debe funcionar");
        let val = m.read_u8(addr).expect("leer en paleta debe funcionar");
        assert_eq!(val, pattern, "dato leído debe coincidir en offset {}", offset);
    }

    // Test registro de control: escribir y leer
    let pattern: u32 = 0x0000_0001;
    m.write_u32(VIDEO_CONTROL_BASE as u64, pattern).expect("escribir en control debe funcionar");
    let val = m.read_u32(VIDEO_CONTROL_BASE as u64).expect("leer en control debe funcionar");
    assert_eq!(val, pattern, "dato leído en control debe coincidir");
}

#[test]
fn fb0_regiones_no_solapan() {
    // Definir los rangos
    let text_range = (TEXT_BASE, TEXT_BASE + TEXT_SIZE);
    let data_range = (DATA_REGION_BASE, DATA_REGION_BASE + DATA_REGION_SIZE);
    let stack_range = (STACK_TOP - STACK_SIZE, STACK_TOP);
    let fb_range = (FRAMEBUFFER_BASE, FRAMEBUFFER_BASE + FRAMEBUFFER_SIZE);
    let pal_range = (PALETTE_BASE, PALETTE_BASE + PALETTE_SIZE);
    let ctrl_range = (VIDEO_CONTROL_BASE, VIDEO_CONTROL_BASE + VIDEO_CONTROL_SIZE);

    // Función auxiliar: verificar no solapamiento
    fn no_overlap(a: (u32, u32), b: (u32, u32)) -> bool {
        a.1 <= b.0 || b.1 <= a.0
    }

    // Verificar pares
    assert!(no_overlap(text_range, data_range), "text y data no deben solapar");
    assert!(no_overlap(text_range, stack_range), "text y pila no deben solapar");
    assert!(no_overlap(text_range, fb_range), "text y FB no deben solapar");
    assert!(no_overlap(text_range, pal_range), "text y paleta no deben solapar");
    assert!(no_overlap(text_range, ctrl_range), "text y control no deben solapar");

    assert!(no_overlap(data_range, stack_range), "data y pila no deben solapar");
    assert!(no_overlap(data_range, fb_range), "data y FB no deben solapar");
    assert!(no_overlap(data_range, pal_range), "data y paleta no deben solapar");
    assert!(no_overlap(data_range, ctrl_range), "data y control no deben solapar");

    assert!(no_overlap(stack_range, fb_range), "pila y FB no deben solapar");
    assert!(no_overlap(stack_range, pal_range), "pila y paleta no deben solapar");
    assert!(no_overlap(stack_range, ctrl_range), "pila y control no deben solapar");

    assert!(no_overlap(fb_range, pal_range), "FB y paleta no deben solapar");
    assert!(no_overlap(fb_range, ctrl_range), "FB y control no deben solapar");

    assert!(no_overlap(pal_range, ctrl_range), "paleta y control no deben solapar");
}

#[test]
fn fb0_tamaños_correctos() {
    assert_eq!(FRAMEBUFFER_DATA_SIZE, 320 * 240, "FB datos debe ser 320×240");
    assert_eq!(PALETTE_DATA_SIZE, 256 * 3, "paleta datos debe ser 256×3");
}
