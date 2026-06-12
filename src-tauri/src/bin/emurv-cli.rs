//! emurv-cli — modo headless de EmuRV (RP-3 del roadmap).
//!
//! Subcomandos: `asm` (ensambla y exporta ELF32) y `run` (ensambla y ejecuta
//! con la consola puenteada a stdin/stdout). Reglas de canal:
//! - stdout es EXCLUSIVAMENTE la salida del programa invitado (corrección
//!   automática por diff depende de esto);
//! - TODOS los diagnósticos (errores, avisos, uso) van por stderr.
//!
//! Códigos de salida del proceso (diseño acordado en PLAN-RP3):
//! 0 = el programa invitado terminó · 1 = error de runtime o de E/S ·
//! 2 = error de ensamblado · 3 = uso inválido · 124 = límite agotado
//! (`--max-instr`/`--timeout-s`, convención de timeout(1)).
//!
//! El parsing de argumentos es manual con `std::env` (regla CLAUDE.md de
//! dependencias: `clap` no se justifica para dos subcomandos y cuatro flags).

use emurv_lib::core::assembler::assemble;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USO: &str = "\
emurv-cli — EmuRV sin interfaz gráfica (RV32IM, syscalls estilo RARS)

USO:
  emurv-cli asm <entrada.s> [-o <salida.elf>]
      Ensambla el fuente y exporta un ELF32 RISC-V estándar.
      Sin -o, la salida es la entrada con extensión .elf.

  emurv-cli run <entrada.s> [--max-instr N] [--timeout-s S]
      Ensambla y ejecuta. La consola del programa usa stdin/stdout:
      stdout lleva SOLO la salida del programa; los diagnósticos van
      por stderr. --max-instr y --timeout-s acotan la corrida
      (al agotarse: código de salida 124).

  emurv-cli --help | --version

CÓDIGOS DE SALIDA:
  0 programa terminado · 1 error de runtime o de E/S · 2 error de
  ensamblado · 3 uso inválido · 124 límite agotado";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = ejecutar_cli(&args);
    ExitCode::from(code)
}

fn ejecutar_cli(args: &[String]) -> u8 {
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => {
            eprintln!("{USO}");
            0
        }
        Some("--version" | "-V") => {
            eprintln!("emurv-cli {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("asm") => subcomando_asm(&args[1..]),
        // El subcomando run llega en la Fase 2 del plan RP3.
        Some("run") => {
            eprintln!("emurv-cli: «run» todavía no está disponible (Fase 2 del plan RP3)\n\n{USO}");
            3
        }
        Some(otro) => {
            eprintln!("emurv-cli: subcomando desconocido «{otro}»\n\n{USO}");
            3
        }
        None => {
            eprintln!("emurv-cli: falta el subcomando\n\n{USO}");
            3
        }
    }
}

/// `asm <entrada.s> [-o <salida.elf>]`
fn subcomando_asm(args: &[String]) -> u8 {
    let mut entrada: Option<PathBuf> = None;
    let mut salida: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => match it.next() {
                Some(p) => salida = Some(PathBuf::from(p)),
                None => {
                    eprintln!("emurv-cli asm: -o requiere una ruta de salida\n\n{USO}");
                    return 3;
                }
            },
            _ if entrada.is_none() => entrada = Some(PathBuf::from(a)),
            otro => {
                eprintln!("emurv-cli asm: argumento inesperado «{otro}»\n\n{USO}");
                return 3;
            }
        }
    }
    let Some(entrada) = entrada else {
        eprintln!("emurv-cli asm: falta el archivo de entrada\n\n{USO}");
        return 3;
    };
    let salida = salida.unwrap_or_else(|| entrada.with_extension("elf"));

    let programa = match ensamblar_archivo(&entrada) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let bytes = match emurv_lib::core::elf_export::export_elf(&programa) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("emurv-cli asm: no se pudo generar el ELF: {e}");
            return 1;
        }
    };
    if let Err(e) = std::fs::write(&salida, bytes) {
        eprintln!(
            "emurv-cli asm: no se pudo escribir «{}»: {e}",
            salida.display()
        );
        return 1;
    }
    eprintln!("ELF escrito en «{}»", salida.display());
    0
}

/// Lee y ensambla un fuente. Errores de E/S → exit 1; de ensamblado → 2,
/// con el formato didáctico en español intacto (uno por línea, a stderr).
fn ensamblar_archivo(ruta: &Path) -> Result<emurv_lib::core::assembler::Program, u8> {
    let fuente = std::fs::read_to_string(ruta).map_err(|e| {
        eprintln!("emurv-cli: no se pudo leer «{}»: {e}", ruta.display());
        1u8
    })?;
    assemble(&fuente).map_err(|errores| {
        for err in &errores {
            eprintln!("{err}");
        }
        eprintln!(
            "emurv-cli: el ensamblado falló con {} error(es)",
            errores.len()
        );
        2u8
    })
}
