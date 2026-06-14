//! Import de ELF32 RISC-V (RP11): el espejo de `elf_export`.
//!
//! Acepta ejecutables ET_EXEC con direcciones absolutas dentro del mapa
//! RARS y devuelve un `Program` normal (la abstracción de Machine no
//! cambia): .text/.data poblados, símbolos del .symtab si existen,
//! `from_binary = true` y `source_map` vacío (no hay fuente).
//!
//! Estrategia de carga (BITACORA RP11-F1): por SECCIONES con SHF_ALLOC
//! cuando el ELF las trae — gcc mapea la página de cabeceras en un PT_LOAD
//! que arranca DEBAJO de TEXT_BASE (medido: p_vaddr 0x003ff000) y las
//! secciones esquivan ese ruido. Si el binario viene sin secciones
//! (strip), se cae a los segmentos PT_LOAD recortados al mapa.
//!
//! Fuera de alcance (plan RP11): relocaciones/PIE, DWARF, RV64.

use super::assembler::{Program, SourceMap};
use super::{DATA_BASE, DATA_REGION_BASE, DATA_REGION_SIZE, TEXT_BASE, TEXT_SIZE};
use object::elf;
use object::read::elf::ElfFile32;
use object::{Endianness, Object, ObjectSection, ObjectSegment, ObjectSymbol};
use std::collections::BTreeMap;

/// Flags de linker que producen un ELF cargable en EmuRV — van en los
/// errores didácticos para que el estudiante copie y pegue.
const FLAGS_LINKER: &str =
    "-march=rv32im -mabi=ilp32 -Ttext=0x00400000 -Tdata=0x10010000 -nostdlib";

/// Carga un ELF32 RISC-V ejecutable y lo convierte en `Program`.
pub fn import_elf(bytes: &[u8]) -> Result<Program, String> {
    validar_cabecera(bytes)?;
    let file: ElfFile32<Endianness> = ElfFile32::parse(bytes)
        .map_err(|e| format!("el ELF no se pudo interpretar: {e}"))?;

    let mut text = Region::new(TEXT_BASE, TEXT_BASE + TEXT_SIZE);
    // Machine escribe program.data en DATA_BASE: lo cargable es de ahí al
    // fin de la región (la franja 0x1000_0000..DATA_BASE no se acepta).
    let mut data = Region::new(DATA_BASE, DATA_REGION_BASE + DATA_REGION_SIZE);

    // --- Carga por secciones ALLOC (la vía normal) ---
    let mut hubo_secciones = false;
    for sec in file.sections() {
        let flags = match sec.flags() {
            object::SectionFlags::Elf { sh_flags } => sh_flags,
            _ => 0,
        };
        if flags & u64::from(elf::SHF_ALLOC) == 0 || sec.size() == 0 {
            continue;
        }
        hubo_secciones = true;
        let addr = sec.address() as u32;
        let nombre = sec.name().unwrap_or("?").to_string();
        let destino = if flags & u64::from(elf::SHF_EXECINSTR) != 0 {
            &mut text
        } else {
            &mut data
        };
        let contenido = sec
            .data()
            .map_err(|e| format!("no se pudo leer la sección {nombre}: {e}"))?;
        if contenido.is_empty() {
            // NOBITS (.bss): reservar ceros.
            destino.reservar(addr, sec.size() as u32, &nombre)?;
        } else {
            destino.copiar(addr, contenido, &nombre)?;
        }
    }

    // --- Fallback: binario sin secciones (strip) → segmentos PT_LOAD ---
    if !hubo_secciones {
        for seg in file.segments() {
            let addr = seg.address() as u32;
            let contenido = seg
                .data()
                .map_err(|e| format!("no se pudo leer un segmento: {e}"))?;
            // Recorte didáctico: la página de cabeceras que gcc mapea por
            // debajo de TEXT_BASE no es código — se descarta en silencio.
            let (addr, contenido) = if addr < TEXT_BASE
                && addr.wrapping_add(contenido.len() as u32) > TEXT_BASE
            {
                let salto = (TEXT_BASE - addr) as usize;
                (TEXT_BASE, &contenido[salto..])
            } else {
                (addr, contenido)
            };
            if contenido.is_empty() {
                continue;
            }
            let destino = if text.contiene(addr) { &mut text } else { &mut data };
            destino.copiar(addr, contenido, "PT_LOAD")?;
        }
    }

    if text.bytes.is_empty() {
        return Err(format!(
            "el ELF no trae código ejecutable dentro de .text del mapa de \
             EmuRV (0x{TEXT_BASE:08X}). Enlazá con: {FLAGS_LINKER}"
        ));
    }

    // --- Entry: TEXT_BASE o, si difiere, dentro de .text ---
    let entry = file.entry() as u32;
    let fin_text = TEXT_BASE + text.bytes.len() as u32;
    if !(TEXT_BASE..fin_text).contains(&entry) {
        return Err(format!(
            "el punto de entrada (e_entry = 0x{entry:08X}) cae fuera del \
             código cargado [0x{TEXT_BASE:08X}, 0x{fin_text:08X}). Enlazá \
             con: {FLAGS_LINKER}"
        ));
    }

    // --- Símbolos del .symtab: FUNC y OBJECT globales (como exporta RP-6) ---
    let mut symbols = BTreeMap::new();
    for sym in file.symbols() {
        let es_global = sym.is_global();
        let tipo_util = matches!(
            sym.kind(),
            object::SymbolKind::Text | object::SymbolKind::Data
        );
        if es_global && tipo_util {
            if let Ok(nombre) = sym.name() {
                if !nombre.is_empty() {
                    symbols.insert(nombre.to_string(), sym.address() as u32);
                }
            }
        }
    }

    Ok(Program {
        text: text.bytes,
        data: data.bytes,
        symbols,
        source_map: SourceMap::default(),
        origins: Vec::new(),
        files: Vec::new(),
        entry,
        from_binary: true,
        // Solo aceptamos ET_EXEC absoluto al importar (ver más abajo): un
        // binario importado nunca es «reubicable» para EmuRV.
        reubicable: false,
    })
}

/// ¿Los bytes son un ELF? (para que «Abrir»/CLI detecten por magic, sin
/// depender de la extensión).
pub fn es_elf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[0..4] == b"\x7fELF"
}

/// Validaciones didácticas de la cabecera, una por una y en orden de
/// diagnóstico (la primera que falla explica el problema REAL).
fn validar_cabecera(b: &[u8]) -> Result<(), String> {
    if !es_elf(b) {
        return Err(
            "el archivo no es un ELF (no empieza con la firma 7F 45 4C 46). \
             ¿Seguro que es un ejecutable y no un fuente .s?"
                .to_string(),
        );
    }
    if b.len() < 20 {
        return Err("el archivo está truncado: no llega ni a la cabecera ELF".to_string());
    }
    if b[4] != elf::ELFCLASS32 {
        return Err(format!(
            "es un ELF de 64 bits y EmuRV emula RV32 (32 bits). Compilá \
             para 32 bits: {FLAGS_LINKER}"
        ));
    }
    if b[5] != elf::ELFDATA2LSB {
        return Err(
            "el ELF es big-endian y RISC-V (y EmuRV) es little-endian. \
             Revisá la toolchain con la que lo generaste"
                .to_string(),
        );
    }
    let e_type = u16::from_le_bytes([b[16], b[17]]);
    let e_machine = u16::from_le_bytes([b[18], b[19]]);
    if e_machine != elf::EM_RISCV {
        return Err(format!(
            "el ELF no es de RISC-V (e_machine = {e_machine}; RISC-V es \
             243). Compilá con un gcc/clang RISC-V: {FLAGS_LINKER}"
        ));
    }
    if e_type != elf::ET_EXEC {
        let que = if e_type == elf::ET_DYN {
            "es un ejecutable PIE/dinámico (ET_DYN) y EmuRV solo carga \
             direcciones absolutas (ET_EXEC). Agregá -no-pie al enlazar"
        } else if e_type == elf::ET_REL {
            "es un objeto relocalizable (.o, ET_REL), no un ejecutable: \
             falta el paso de linkeo"
        } else {
            "no es un ejecutable (ET_EXEC)"
        };
        return Err(format!("{que}. Flags completos: {FLAGS_LINKER}"));
    }
    Ok(())
}

/// Acumulador de bytes de una región del mapa: las secciones se copian a
/// su offset (huecos = ceros) y todo lo que cae afuera corta con el error
/// didáctico de los flags de linker.
struct Region {
    base: u32,
    fin: u32,
    bytes: Vec<u8>,
}

impl Region {
    fn new(base: u32, fin: u32) -> Self {
        Region {
            base,
            fin,
            bytes: Vec::new(),
        }
    }

    fn contiene(&self, addr: u32) -> bool {
        (self.base..self.fin).contains(&addr)
    }

    fn validar(&self, addr: u32, len: u32, nombre: &str) -> Result<(), String> {
        let dentro = addr >= self.base && addr.wrapping_add(len) <= self.fin;
        if !dentro {
            return Err(format!(
                "la sección {nombre} (0x{addr:08X}, {len} bytes) cae fuera \
                 del mapa de memoria de EmuRV (.text en 0x{TEXT_BASE:08X}, \
                 .data en 0x{DATA_BASE:08X}). Enlazá con: {FLAGS_LINKER}"
            ));
        }
        Ok(())
    }

    fn copiar(&mut self, addr: u32, contenido: &[u8], nombre: &str) -> Result<(), String> {
        self.validar(addr, contenido.len() as u32, nombre)?;
        let desde = (addr - self.base) as usize;
        let hasta = desde + contenido.len();
        if self.bytes.len() < hasta {
            self.bytes.resize(hasta, 0);
        }
        self.bytes[desde..hasta].copy_from_slice(contenido);
        Ok(())
    }

    fn reservar(&mut self, addr: u32, len: u32, nombre: &str) -> Result<(), String> {
        self.validar(addr, len, nombre)?;
        let hasta = (addr - self.base) as usize + len as usize;
        if self.bytes.len() < hasta {
            self.bytes.resize(hasta, 0);
        }
        Ok(())
    }
}
