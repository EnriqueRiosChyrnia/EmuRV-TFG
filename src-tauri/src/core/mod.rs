//! Núcleo de EmuRV: ensamblador RV32IM, máquina (Unicorn), syscalls y export ELF.
//!
//! Mapa de memoria estilo RARS (ver ARQUITECTURA.md §Diseños clave):
//! la compatibilidad con RARS es deliberada, para poder reutilizar material
//! didáctico existente en español sin cambiar direcciones.

pub mod assembler;
pub mod machine;
pub mod syscalls;
// elf_export se agrega en la fase F5 (PLAN.md).

/// Base del segmento de código (.text). Coincide con RARS.
pub const TEXT_BASE: u32 = 0x0040_0000;
/// Tamaño mapeado para .text (4 MB).
pub const TEXT_SIZE: u32 = 0x0040_0000;

/// Base de la región de datos mapeada (incluye la zona estática previa a .data).
pub const DATA_REGION_BASE: u32 = 0x1000_0000;
/// Tamaño mapeado para la región de datos (4 MB; el heap crece después de .data).
pub const DATA_REGION_SIZE: u32 = 0x0040_0000;
/// Base del segmento .data visible al programa. Coincide con RARS.
pub const DATA_BASE: u32 = 0x1001_0000;

/// Tope de la pila (no incluido en el mapeo: la pila crece hacia abajo).
pub const STACK_TOP: u32 = 0x8000_0000;
/// Tamaño mapeado de pila (8 MB por debajo de STACK_TOP).
pub const STACK_SIZE: u32 = 0x0080_0000;
/// Valor inicial de `sp` (alineado a 16, estilo RARS).
pub const SP_INIT: u32 = 0x7FFF_FFF0;
/// Valor inicial de `gp` (puntero global, convención RARS).
pub const GP_INIT: u32 = 0x1000_8000;
