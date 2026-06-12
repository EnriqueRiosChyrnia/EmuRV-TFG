//! Preprocesador de fuentes (RP5): aplana `.include` ANTES de las dos
//! pasadas del ensamblador, llevando una tabla de procedencia por línea.
//!
//! La pieza central es la regla de atribución `main_line` (el corazón del
//! invariante del source_map):
//! - una línea del archivo principal se atribuye a SU propia línea;
//! - una línea traída por `.include` (directo o anidado) se atribuye a la
//!   línea de la directiva `.include` DEL ARCHIVO PRINCIPAL que la
//!   introdujo.
//!
//! Consecuencia: el `SourceMap` sigue hablando SOLO de líneas del archivo
//! principal — la UI actual (breakpoints, resaltado, editor de un buffer)
//! funciona sin cambios. Los ERRORES, en cambio, reportan archivo y línea
//! REALES (campos `file`/`line` de [`PpLine`]), no la atribución.

use super::assembler::AsmError;
use std::path::{Path, PathBuf};

/// Una línea del fuente aplanado, con su procedencia.
#[derive(Debug, Clone)]
pub struct PpLine {
    pub text: String,
    /// Índice en `Preprocessed::files` (0 = archivo principal).
    pub file: u16,
    /// Línea 1-based en SU archivo (para errores).
    pub line: u32,
    /// Línea del archivo PRINCIPAL a la que se atribuye (para source_map).
    pub main_line: u32,
}

/// Fuente aplanado: lo que consumen las dos pasadas del ensamblador.
#[derive(Debug, Clone)]
pub struct Preprocessed {
    pub lines: Vec<PpLine>,
    /// Nombres de archivo; `[0]` es el principal.
    pub files: Vec<String>,
}

/// Profundidad máxima de inclusión (anti-recursión accidental).
const MAX_DEPTH: usize = 16;

/// ¿La línea es una directiva `.include "..."`? Devuelve la ruta si lo es.
/// Solo se reconoce al comienzo de la línea (sin etiqueta delante), con la
/// ruta entre comillas dobles. Una línea comentada no dispara.
fn parse_include(text: &str) -> Option<Result<String, ()>> {
    let t = text.trim_start();
    let rest = t.strip_prefix(".include")?;
    // Debe seguir un separador (no aceptar ".includex").
    if !rest.starts_with([' ', '\t', '"']) {
        return None;
    }
    let rest = rest.trim_start();
    // Cortar comentario final fuera de comillas no hace falta: la ruta va
    // primero; lo que siga tras la comilla de cierre se valida abajo.
    let rest = match rest.strip_prefix('"') {
        Some(r) => r,
        None => return Some(Err(())),
    };
    let end = match rest.find('"') {
        Some(e) => e,
        None => return Some(Err(())),
    };
    let path = &rest[..end];
    let despues = rest[end + 1..].trim_start();
    if !despues.is_empty() && !despues.starts_with('#') {
        return Some(Err(()));
    }
    Some(Ok(path.to_string()))
}

struct Pp {
    lines: Vec<PpLine>,
    files: Vec<String>,
    /// Rutas canónicas en la pila de inclusión (detección de ciclos) con su
    /// nombre legible para el mensaje de error.
    stack: Vec<(PathBuf, String)>,
    errors: Vec<AsmError>,
}

/// Aplana `source` resolviendo `.include` relativo a `base_dir`.
///
/// Con `base_dir: None` (buffer sin guardar, tests), un `.include`
/// produce el error didáctico correspondiente.
pub fn preprocess(
    source: &str,
    base_dir: Option<&Path>,
    main_name: &str,
) -> Result<Preprocessed, Vec<AsmError>> {
    let mut pp = Pp {
        lines: Vec::new(),
        files: vec![main_name.to_string()],
        stack: Vec::new(),
        errors: Vec::new(),
    };
    expand(&mut pp, source, 0, base_dir, None);
    if pp.errors.is_empty() {
        Ok(Preprocessed {
            lines: pp.lines,
            files: pp.files,
        })
    } else {
        Err(std::mem::take(&mut pp.errors))
    }
}

/// Expande un archivo. `attrib` es la línea del principal a la que se
/// atribuye TODO este archivo (None = estamos EN el principal: cada línea
/// se atribuye a sí misma).
fn expand(pp: &mut Pp, source: &str, file_idx: u16, dir: Option<&Path>, attrib: Option<u32>) {
    for (idx, raw) in source.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        let main_line = attrib.unwrap_or(line_no);
        match parse_include(raw) {
            None => pp.lines.push(PpLine {
                text: raw.to_string(),
                file: file_idx,
                line: line_no,
                main_line,
            }),
            Some(Err(())) => pp.push_err(
                file_idx,
                line_no,
                ".include espera una ruta entre comillas dobles (ej.: .include \"utiles.s\")",
            ),
            Some(Ok(rel)) => {
                let Some(dir) = dir else {
                    pp.push_err(
                        file_idx,
                        line_no,
                        ".include necesita que el archivo esté guardado en disco \
                         (guardá el programa primero, así hay una carpeta base \
                         desde donde resolver la ruta)",
                    );
                    continue;
                };
                if pp.stack.len() >= MAX_DEPTH {
                    pp.push_err(
                        file_idx,
                        line_no,
                        format!("inclusión demasiado profunda (máximo {MAX_DEPTH} niveles)"),
                    );
                    continue;
                }
                let ruta = dir.join(&rel);
                let canon = ruta.canonicalize().unwrap_or_else(|_| ruta.clone());
                if pp.stack.iter().any(|(p, _)| *p == canon) {
                    let mut cadena: Vec<&str> = pp.stack.iter().map(|(_, n)| n.as_str()).collect();
                    cadena.push(&rel);
                    pp.push_err(
                        file_idx,
                        line_no,
                        format!(
                            "inclusión cíclica: {} (un archivo termina incluyéndose \
                             a sí mismo)",
                            cadena.join(" → ")
                        ),
                    );
                    continue;
                }
                let contenido = match std::fs::read_to_string(&ruta) {
                    Ok(c) => c,
                    Err(e) => {
                        pp.push_err(
                            file_idx,
                            line_no,
                            format!(
                                "no se pudo leer el archivo incluido «{}»: {e}",
                                ruta.display()
                            ),
                        );
                        continue;
                    }
                };
                let nuevo_idx = pp.files.len() as u16;
                pp.files.push(rel.clone());
                pp.stack.push((canon, rel));
                let sub_dir = ruta.parent().map(Path::to_path_buf);
                // La atribución del include es la línea de SU directiva en
                // el principal: si ya venimos atribuidos (include anidado),
                // se hereda; si estamos en el principal, es esta línea.
                let sub_attrib = Some(attrib.unwrap_or(line_no));
                expand(pp, &contenido, nuevo_idx, sub_dir.as_deref(), sub_attrib);
                pp.stack.pop();
            }
        }
    }
}

impl Pp {
    fn push_err(&mut self, file_idx: u16, line: u32, msg: impl Into<String>) {
        let mut e = AsmError::new(line, 1, msg);
        if file_idx != 0 {
            e.file = Some(self.files[file_idx as usize].clone());
        }
        self.errors.push(e);
    }
}
