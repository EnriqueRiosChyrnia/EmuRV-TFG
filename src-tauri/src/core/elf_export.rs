//! Export a ELF32 RISC-V (RF-08) con `object::write::elf::Writer`.
//!
//! Genera un ejecutable estándar (ET_EXEC, EM_RISCV, little-endian) con
//! dos segmentos PT_LOAD que reproducen el mapa de memoria del emulador:
//! .text en TEXT_BASE (R+X) y .data en DATA_BASE (R+W), e_entry en el
//! comienzo de .text. Incluye además la tabla de secciones (.text, .data,
//! .shstrtab) para que readelf/Ripes muestren información completa.
//!
//! Nota (BITACORA F0): las syscalls de EmuRV siguen la convención RARS,
//! no la de Linux; el ELF es para cargar el código en otras herramientas,
//! no para esperar la misma salida de consola bajo qemu/Linux.

use super::assembler::Program;
use super::{DATA_BASE, TEXT_BASE};
use object::write::elf::{FileHeader, ProgramHeader, SectionHeader, Sym, Writer};
use object::{elf, Endianness};

/// Alineación de página usada por los segmentos PT_LOAD.
const PAGE: usize = 0x1000;

/// Serializa el programa ensamblado como ELF32 ejecutable.
pub fn export_elf(program: &Program) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let mut w = Writer::new(Endianness::Little, false, &mut buffer);
    let err = |e: object::write::Error| format!("error generando el ELF: {e}");

    let has_data = !program.data.is_empty();

    // --- Reservas (definen el layout del archivo) ---
    let name_text = w.add_section_name(b".text");
    let name_data = if has_data {
        Some(w.add_section_name(b".data"))
    } else {
        None
    };

    w.reserve_file_header();
    w.reserve_program_headers(if has_data { 2 } else { 1 });
    // p_offset ≡ p_vaddr (mod p_align): TEXT_BASE y DATA_BASE son múltiplos
    // de 0x1000, así que basta alinear los offsets a página.
    let text_offset = w.reserve(program.text.len(), PAGE);
    let data_offset = if has_data {
        w.reserve(program.data.len(), PAGE)
    } else {
        0
    };

    w.reserve_null_section_index();
    let text_index = w.reserve_section_index();
    let data_index = if has_data {
        Some(w.reserve_section_index())
    } else {
        None
    };
    // .symtab con las etiquetas del programa (RP6): permite «b etiqueta» e
    // «info symbol» en una sesión gdb/lldb real. Orden de reservas del
    // Writer de bajo nivel: índices → nombres → strtab/symtab → headers.
    let etiquetas: Vec<(&String, &u32)> = program.symbols.iter().collect();
    let mut nombres_sym = Vec::with_capacity(etiquetas.len());
    for (nombre, _) in &etiquetas {
        nombres_sym.push(w.add_string(nombre.as_bytes()));
    }
    w.reserve_null_symbol_index();
    for _ in &etiquetas {
        w.reserve_symbol_index(None);
    }
    w.reserve_symtab_section_index();
    w.reserve_strtab_section_index();
    w.reserve_shstrtab_section_index();
    w.reserve_symtab();
    w.reserve_strtab();
    w.reserve_shstrtab();
    w.reserve_section_headers();

    // --- Escritura (mismo orden que las reservas) ---
    // Modo reubicable (RP29): el código es PC-relativo, así que el artefacto
    // se marca ET_DYN (PIE) — declara que puede cargarse en otra base mientras
    // .text y .data se desplacen juntos. El export normal sigue ET_EXEC con
    // direcciones absolutas. (EmuRV solo IMPORTA ET_EXEC; el reubicable es para
    // otras herramientas — ver elf_import.rs.)
    let e_type = if program.reubicable {
        elf::ET_DYN
    } else {
        elf::ET_EXEC
    };
    w.write_file_header(&FileHeader {
        os_abi: elf::ELFOSABI_NONE,
        abi_version: 0,
        e_type,
        e_machine: elf::EM_RISCV,
        e_entry: TEXT_BASE as u64,
        e_flags: 0,
    })
    .map_err(err)?;

    w.write_program_header(&ProgramHeader {
        p_type: elf::PT_LOAD,
        p_flags: elf::PF_R | elf::PF_X,
        p_offset: text_offset as u64,
        p_vaddr: TEXT_BASE as u64,
        p_paddr: TEXT_BASE as u64,
        p_filesz: program.text.len() as u64,
        p_memsz: program.text.len() as u64,
        p_align: PAGE as u64,
    });
    if has_data {
        w.write_program_header(&ProgramHeader {
            p_type: elf::PT_LOAD,
            p_flags: elf::PF_R | elf::PF_W,
            p_offset: data_offset as u64,
            p_vaddr: DATA_BASE as u64,
            p_paddr: DATA_BASE as u64,
            p_filesz: program.data.len() as u64,
            p_memsz: program.data.len() as u64,
            p_align: PAGE as u64,
        });
    }

    w.write_align(PAGE);
    w.write(&program.text);
    if has_data {
        w.write_align(PAGE);
        w.write(&program.data);
    }

    // Símbolos: etiquetas de .text como FUNC y de .data como OBJECT,
    // binding GLOBAL (alcanza para breakpoints por nombre; sin tamaños).
    w.write_null_symbol();
    let fin_text = TEXT_BASE.wrapping_add(program.text.len() as u32);
    for (i, (_, &addr)) in etiquetas.iter().enumerate() {
        let en_text = addr >= TEXT_BASE && addr < fin_text;
        let (tipo, seccion) = if en_text {
            (elf::STT_FUNC, Some(text_index))
        } else {
            (elf::STT_OBJECT, data_index)
        };
        w.write_symbol(&Sym {
            name: Some(nombres_sym[i]),
            section: seccion,
            st_info: (elf::STB_GLOBAL << 4) | tipo,
            st_other: elf::STV_DEFAULT,
            st_shndx: 0,
            st_value: addr as u64,
            st_size: 0,
        });
    }
    w.write_strtab();
    w.write_shstrtab();

    w.write_null_section_header();
    w.write_section_header(&SectionHeader {
        name: Some(name_text),
        sh_type: elf::SHT_PROGBITS,
        sh_flags: (elf::SHF_ALLOC | elf::SHF_EXECINSTR) as u64,
        sh_addr: TEXT_BASE as u64,
        sh_offset: text_offset as u64,
        sh_size: program.text.len() as u64,
        sh_link: 0,
        sh_info: 0,
        sh_addralign: 4,
        sh_entsize: 0,
    });
    if has_data {
        w.write_section_header(&SectionHeader {
            name: name_data,
            sh_type: elf::SHT_PROGBITS,
            sh_flags: (elf::SHF_ALLOC | elf::SHF_WRITE) as u64,
            sh_addr: DATA_BASE as u64,
            sh_offset: data_offset as u64,
            sh_size: program.data.len() as u64,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 4,
            sh_entsize: 0,
        });
    }
    w.write_symtab_section_header(1); // sh_info: primer símbolo NO-local (solo el nulo es local)
    w.write_strtab_section_header();
    w.write_shstrtab_section_header();

    debug_assert_eq!(w.reserved_len(), buffer.len());
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::assembler::assemble;

    fn elf_de(src: &str) -> Vec<u8> {
        export_elf(&assemble(src).expect("debe ensamblar")).expect("debe exportar")
    }

    fn u16le(b: &[u8], off: usize) -> u16 {
        u16::from_le_bytes([b[off], b[off + 1]])
    }
    fn u32le(b: &[u8], off: usize) -> u32 {
        u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    }

    #[test]
    fn header_elf32_riscv_ejecutable() {
        let b = elf_de(".data\nv: .word 7\n.text\nli a7, 10\necall");
        // e_ident
        assert_eq!(&b[0..4], b"\x7fELF");
        assert_eq!(b[4], 1, "ELFCLASS32");
        assert_eq!(b[5], 1, "ELFDATA2LSB (little-endian)");
        assert_eq!(b[6], 1, "EV_CURRENT");
        // campos del header (offsets de la espec ELF32)
        assert_eq!(u16le(&b, 16), elf::ET_EXEC, "e_type ejecutable");
        assert_eq!(u16le(&b, 18), elf::EM_RISCV, "e_machine RISC-V (243)");
        assert_eq!(u32le(&b, 24), TEXT_BASE, "e_entry = comienzo de .text");
    }

    #[test]
    fn segmentos_cargables_con_direcciones_del_mapa() {
        let b = elf_de(".data\nmsj: .asciiz \"hola\"\n.text\nnop\nli a7, 10\necall");
        let phoff = u32le(&b, 28) as usize;
        let phentsize = u16le(&b, 42) as usize;
        let phnum = u16le(&b, 44) as usize;
        assert_eq!(phnum, 2, "PT_LOAD de .text y .data");

        // segmento 0: .text
        let p = phoff;
        assert_eq!(u32le(&b, p), elf::PT_LOAD);
        assert_eq!(u32le(&b, p + 8), TEXT_BASE, "p_vaddr .text");
        let text_off = u32le(&b, p + 4) as usize;
        let text_sz = u32le(&b, p + 16) as usize;
        assert_eq!(u32le(&b, p + 24), elf::PF_R | elf::PF_X, "p_flags R+X");
        // p_offset ≡ p_vaddr (mod align)
        assert_eq!(text_off % 0x1000, (TEXT_BASE as usize) % 0x1000);

        // segmento 1: .data
        let p = phoff + phentsize;
        assert_eq!(u32le(&b, p), elf::PT_LOAD);
        assert_eq!(u32le(&b, p + 8), DATA_BASE, "p_vaddr .data");
        assert_eq!(u32le(&b, p + 24), elf::PF_R | elf::PF_W, "p_flags R+W");
        let data_off = u32le(&b, p + 4) as usize;
        let data_sz = u32le(&b, p + 16) as usize;

        // el contenido de los segmentos es el programa ensamblado
        let programa =
            assemble(".data\nmsj: .asciiz \"hola\"\n.text\nnop\nli a7, 10\necall").unwrap();
        assert_eq!(&b[text_off..text_off + text_sz], &programa.text[..]);
        assert_eq!(&b[data_off..data_off + data_sz], &programa.data[..]);
    }

    #[test]
    fn sin_datos_emite_un_solo_segmento() {
        let b = elf_de("li a7, 10\necall");
        assert_eq!(u16le(&b, 44), 1, "solo PT_LOAD de .text");
    }
}
