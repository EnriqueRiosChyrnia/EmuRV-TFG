//! Núcleo de EmuRV: ensamblador RV32IM, máquina (Unicorn), syscalls y export ELF.
//!
//! Mapa de memoria estilo RARS (ver ARQUITECTURA.md §Diseños clave):
//! la compatibilidad con RARS es deliberada, para poder reutilizar material
//! didáctico existente en español sin cambiar direcciones.

pub mod assembler;
pub mod cond;
pub mod disasm;
pub mod elf_export;
pub mod elf_import;
pub mod keyboard;
pub mod machine;
pub mod preprocess;
pub mod syscalls;
pub mod video;

/// Palabra de la instrucción `ebreak` (RV32I). La comparten el hook de
/// traps y el rescate de pausa por software en `Machine::interpret()`
/// (Unicorn 2.1.5 no entrega el trap de ebreak al hook INTR — BITACORA RP1).
pub const EBREAK_WORD: u32 = 0x0010_0073;

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
/// Base del heap (syscall 9, sbrk). Canónico RARS: 0x1004_0000, dentro de
/// la región de datos (el heap crece hacia arriba desde acá; la pila vive
/// en otra región — sin colisión heap/pila posible). La carga del programa
/// rechaza un .data que llegue hasta acá («tus datos pisan el heap»).
pub const HEAP_BASE: u32 = 0x1004_0000;

/// Tope de la pila (no incluido en el mapeo: la pila crece hacia abajo).
pub const STACK_TOP: u32 = 0x8000_0000;
/// Tamaño mapeado de pila (8 MB por debajo de STACK_TOP).
pub const STACK_SIZE: u32 = 0x0080_0000;
/// Valor inicial de `sp` (alineado a 16, estilo RARS).
pub const SP_INIT: u32 = 0x7FFF_FFF0;
/// Valor inicial de `gp` (puntero global, convención RARS).
pub const GP_INIT: u32 = 0x1000_8000;

/// Región de video (framebuffer inspirado en el modo 13h de VGA, que es 320×200;
/// acá 320×240): 1 byte/píxel (paleta indexada).
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

/// Registro de control de video: el programa escribe 1 para marcar uso de pantalla;
/// el emulador lo lee pasivamente (no hay hook de escritura) y publica el frame al
/// final de cada tramo de ejecución (ver `publicar_frame`).
pub const VIDEO_CONTROL_BASE: u32 = 0x9003_0000;
/// Tamaño mapeado del control (alineado a página 4K).
pub const VIDEO_CONTROL_SIZE: u32 = 0x00001000;

/// Base de la página MMIO del teclado (TEC2). Misma dirección que el
/// receptor del teclado de RARS/MARS, a propósito: el material didáctico
/// existente para esos simuladores funciona sin cambiar direcciones.
pub const KEYBOARD_BASE: u32 = 0xFFFF_0000;
/// Tamaño mapeado de la página del teclado (4 KB, mínimo de Unicorn).
pub const KEYBOARD_SIZE: u32 = 0x1000;
/// RECEIVER_CONTROL (compatible RARS/MARS): el bit 0 en 1 indica que hay
/// una tecla lista en RECEIVER_DATA. El bit 1 (habilitar interrupciones de
/// teclado, semántica MARS) queda RESERVADO: no está implementado, el
/// programa debe consultar por encuesta (polling).
pub const RECEIVER_CONTROL: u32 = 0xFFFF_0000;
/// RECEIVER_DATA (compatible RARS/MARS): código de la tecla más vieja
/// pendiente. Leerlo CONSUME la tecla: la siguiente lectura de
/// RECEIVER_CONTROL devuelve 0 si la cola quedó vacía.
pub const RECEIVER_DATA: u32 = 0xFFFF_0004;

/// IDs de los CSRs M-mode en el binding de Unicorn 2.1.5 (verificados por
/// el spike RP8-F1: el guest escribe con csrw y el host lee el mismo valor
/// por estos números). Solo para uso interno de la delegación de traps y
/// del panel de CSRs.
pub const CSR_ID_MSTATUS: i32 = 116;
pub const CSR_ID_MIE: i32 = 120;
pub const CSR_ID_MTVEC: i32 = 121;
pub const CSR_ID_MSCRATCH: i32 = 127;
pub const CSR_ID_MEPC: i32 = 128;
pub const CSR_ID_MCAUSE: i32 = 129;
pub const CSR_ID_MTVAL: i32 = 130;
pub const CSR_ID_MIP: i32 = 131;
