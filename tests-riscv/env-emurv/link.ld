/* Script de enlace EmuRV para riscv-tests (RT-2).
 *
 * Reproduce el mapa de memoria real de la Machine
 * (src-tauri/src/core/mod.rs): .text en TEXT_BASE y datos en DATA_BASE,
 * para que los ELFs entren por elf_import como cualquier programa del
 * producto. Reemplaza al link.ld del env `p` (que enlaza en 0x8000_0000,
 * fuera del mapa de EmuRV).
 */
OUTPUT_ARCH("riscv")
ENTRY(_start)

SECTIONS
{
  /* TEXT_BASE */
  . = 0x00400000;
  .text : {
    *(.text.init)
    *(.text)
    *(.text.*)
  }

  /* DATA_BASE — elf_import solo acepta datos desde aquí */
  . = 0x10010000;
  .data : {
    *(.rodata)
    *(.rodata.*)
    *(.srodata*)
    *(.data)
    *(.data.*)
    *(.sdata*)
  }
  .bss : {
    *(.sbss*)
    *(.bss)
    *(.bss.*)
  }
  _end = .;

  /DISCARD/ : { *(.eh_frame) *(.riscv.attributes) *(.comment) }
}
