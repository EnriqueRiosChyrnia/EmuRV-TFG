/* Script de enlace del DUT EmuRV para riscv-arch-test (RT-3).
 * Mismo mapa que tests-riscv/env-emurv/link.ld (RT-2): .text en TEXT_BASE,
 * datos en DATA_BASE — los ELFs entran por elf_import.
 */
OUTPUT_ARCH("riscv")
ENTRY(rvtest_entry_point)

SECTIONS
{
  /* TEXT_BASE */
  . = 0x00400000;
  .text : {
    *(.text.init)
    *(.text)
    *(.text.*)
  }

  /* DATA_BASE */
  . = 0x10010000;
  .data : {
    *(.rodata)
    *(.rodata.*)
    *(.srodata*)
    *(.data)
    *(.data.*)
    *(.sdata*)
  }
  .data.string : { *(.data.string) }
  .bss : {
    *(.sbss*)
    *(.bss)
    *(.bss.*)
  }
  _end = .;

  /DISCARD/ : { *(.eh_frame) *(.riscv.attributes) *(.comment) }
}
