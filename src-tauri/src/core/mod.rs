//! Núcleo de EmuRV: ensamblador RV32IM, máquina (Unicorn), syscalls y export ELF.
//!
//! Mapa de memoria estilo RARS (ver ARQUITECTURA.md §Diseños clave):
//! la compatibilidad con RARS es deliberada, para poder reutilizar material
//! didáctico existente en español sin cambiar direcciones.

pub mod assembler;
pub mod elf_export;
pub mod machine;
pub mod syscalls;
pub mod video;

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

/// Región de video (framebuffer VGA 13h): 320×240 píxeles, 1 byte/píxel (paleta indexada).
pub const FRAMEBUFFER_BASE: u32 = 0x9000_0000;
/// Bytes de datos en framebuffer (320 × 240).
pub const FRAMEBUFFER_DATA_SIZE: u32 = 320 * 240;
/// Tamaño mapeado del framebuffer (alineado a página 4K).
pub const FRAMEBUFFER_SIZE: u32 = 0x00020000;

/// Paleta de colores: 256 entradas × 3 bytes RGB (888).
pub const PALETTE_BASE: u32 = 0x9002_0000;
/// Bytes de datos en paleta (256 × 3).
pub const PALETTE_DATA_SIZE: u32 = 256 * 3;
/// Tamaño mapeado de paleta (alineado a página 4K).
pub const PALETTE_SIZE: u32 = 0x00001000;

/// Registro de control de video: escribir 1 = vsync/refresh.
pub const VIDEO_CONTROL_BASE: u32 = 0x9003_0000;
/// Tamaño mapeado del control (alineado a página 4K).
pub const VIDEO_CONTROL_SIZE: u32 = 0x00001000;
