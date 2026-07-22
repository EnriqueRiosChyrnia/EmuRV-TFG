/* Enlace bare-metal para Spike (RT-4): mapa estándar 0x8000_0000 + tohost. */
OUTPUT_ARCH("riscv")
ENTRY(_start)

SECTIONS
{
  . = 0x80000000;
  .text : { *(.text) *(.text.*) }
  . = ALIGN(0x1000);
  .tohost : { *(.tohost) }
  . = ALIGN(0x1000);
  .data : { *(.data) *(.data.*) *(.sdata*) *(.rodata*) }
  .bss : { *(.bss) *(.sbss*) }
  _end = .;
}
