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
//!
//! Fase 3: macros estilo RARS (`.macro` … `.end_macro`/`.endm`) con
//! parámetros `%nombre` sustituidos textualmente. Las líneas expandidas se
//! atribuyen a la línea de la INVOCACIÓN (misma regla main_line); sus
//! errores reportan el cuerpo real de la macro (archivo y línea donde se
//! definió). Limitación documentada: una macro que define etiquetas y se
//! invoca dos veces produce «etiqueta duplicada» (RARS las renombra por
//! expansión; queda como propuesta).

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
/// Profundidad máxima de expansión anidada de macros (una macro que se
/// invoca a sí misma la alcanza enseguida; N invocaciones SECUENCIALES no
/// la tocan: el límite es de anidamiento, no de cantidad).
const MAX_EXPANSION_DEPTH: u32 = 64;

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

/// Definición de macro: parámetros y cuerpo con su procedencia real.
struct Macro {
    params: Vec<String>,
    /// (texto, archivo donde se definió, línea real en ese archivo)
    body: Vec<(String, u16, u32)>,
}

struct Pp {
    lines: Vec<PpLine>,
    files: Vec<String>,
    /// Rutas canónicas en la pila de inclusión (detección de ciclos) con su
    /// nombre legible para el mensaje de error.
    stack: Vec<(PathBuf, String)>,
    errors: Vec<AsmError>,
    /// Macros visibles (una definida en un include vale DESPUÉS del punto
    /// de inclusión: el preprocesado es secuencial).
    macros: std::collections::HashMap<String, Macro>,
    /// Definición en curso (`.macro` abierto, juntando cuerpo).
    def_abierta: Option<(String, Macro, u32)>,
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
        macros: std::collections::HashMap::new(),
        def_abierta: None,
    };
    expand(&mut pp, source, 0, base_dir, None);
    if let Some((nombre, _, linea)) = pp.def_abierta.take() {
        pp.errors.push(AsmError::new(
            linea,
            1,
            format!(".macro «{nombre}» sin su .end_macro de cierre"),
        ));
    }
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

        // --- macros (Fase 3): definición en curso, apertura y cierre ---
        let recortada = raw.trim_start();
        if let Some((nombre, mac, linea_def)) = pp.def_abierta.take() {
            if recortada.starts_with(".end_macro") || recortada.starts_with(".endm") {
                pp.macros.insert(nombre, mac);
            } else if recortada.starts_with(".macro") {
                pp.push_err(
                    file_idx,
                    line_no,
                    "no se puede definir una macro dentro de otra macro",
                );
                // Se descarta la definición exterior para no cascadear.
            } else {
                let mut mac = mac;
                mac.body.push((raw.to_string(), file_idx, line_no));
                pp.def_abierta = Some((nombre, mac, linea_def));
            }
            continue;
        }
        if recortada.starts_with(".macro") {
            match parse_macro_def(recortada) {
                Ok((nombre, params)) => {
                    if pp.macros.contains_key(&nombre) {
                        pp.push_err(file_idx, line_no, format!("macro redefinida: «{nombre}»"));
                    } else {
                        pp.def_abierta = Some((
                            nombre,
                            Macro {
                                params,
                                body: Vec::new(),
                            },
                            line_no,
                        ));
                    }
                }
                Err(msg) => pp.push_err(file_idx, line_no, msg),
            }
            continue;
        }
        if recortada.starts_with(".end_macro") || recortada.starts_with(".endm") {
            pp.push_err(file_idx, line_no, ".end_macro sin un .macro abierto");
            continue;
        }
        // --- invocación de macro: el primer token es un nombre conocido,
        //     con etiquetas opcionales delante (et: macro arg) ---
        let (etiquetas, resto_linea) = separar_etiquetas(recortada);
        if let Some((nombre, args)) = parse_invocacion(resto_linea.trim_start(), &pp.macros) {
            if !etiquetas.is_empty() {
                // La etiqueta se emite como línea propia: define la
                // dirección de la PRIMERA instrucción expandida.
                pp.lines.push(PpLine {
                    text: etiquetas.to_string(),
                    file: file_idx,
                    line: line_no,
                    main_line,
                });
            }
            expandir_macro(pp, &nombre, &args, file_idx, line_no, main_line, 0);
            continue;
        }

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

// ---------------------------------------------------------------------------
// Macros (RP5 Fase 3)
// ---------------------------------------------------------------------------

/// Separa el prefijo de etiquetas («a: b: resto») de una línea. Devuelve
/// (prefijo_de_etiquetas, resto). Si no hay etiquetas, el prefijo es "".
fn separar_etiquetas(linea: &str) -> (&str, &str) {
    let bytes = linea.as_bytes();
    let mut fin_prefijo = 0usize;
    let mut i = 0usize;
    loop {
        // saltar espacios
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        let ini = i;
        // identificador (sin '.': una directiva nunca es etiqueta)
        if i >= bytes.len() || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
            break;
        }
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
        {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b':' && i > ini {
            i += 1;
            fin_prefijo = i;
        } else {
            break;
        }
    }
    (&linea[..fin_prefijo], &linea[fin_prefijo..])
}

/// Parsea `.macro NOMBRE (%p1, %p2)` (paréntesis y comas opcionales,
/// sintaxis RARS). Devuelve nombre y parámetros (sin el `%`).
fn parse_macro_def(linea: &str) -> Result<(String, Vec<String>), String> {
    let resto = linea.strip_prefix(".macro").unwrap_or(linea).trim();
    let resto = resto.split('#').next().unwrap_or("").trim(); // sin comentario
    let mut partes = resto.split_whitespace();
    let nombre = partes
        .next()
        .ok_or(".macro espera un nombre (ej.: .macro saludar (%quien))")?;
    let nombre = nombre.trim_end_matches('(');
    if nombre.is_empty() || !nombre.chars().next().unwrap().is_alphabetic() {
        return Err(format!(
            ".macro espera un nombre que empiece con letra; se leyó «{nombre}»"
        ));
    }
    let resto_params: String = partes.collect::<Vec<_>>().join(" ");
    let mut params = Vec::new();
    for crudo in resto_params
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split([',', ' '])
    {
        let p = crudo.trim().trim_start_matches('(').trim_end_matches(')');
        if p.is_empty() {
            continue;
        }
        let Some(sin_pct) = p.strip_prefix('%') else {
            return Err(format!(
                "los parámetros de una macro van con %: «{p}» debería ser «%{p}»"
            ));
        };
        params.push(sin_pct.to_string());
    }
    Ok((nombre.to_string(), params))
}

/// ¿La línea invoca una macro conocida? Devuelve nombre y argumentos.
/// La invocación es el nombre al comienzo de la línea (sin etiqueta
/// delante), con argumentos separados por coma (paréntesis opcionales).
fn parse_invocacion(
    linea: &str,
    macros: &std::collections::HashMap<String, Macro>,
) -> Option<(String, Vec<String>)> {
    let sin_comentario = linea.split('#').next().unwrap_or("");
    let mut it = sin_comentario.splitn(2, [' ', '\t', '(']);
    let primero = it.next()?.trim();
    if primero.is_empty() || !macros.contains_key(primero) {
        return None;
    }
    let resto = sin_comentario[primero.len()..]
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');
    let args: Vec<String> = if resto.trim().is_empty() {
        Vec::new()
    } else {
        resto.split(',').map(|a| a.trim().to_string()).collect()
    };
    Some((primero.to_string(), args))
}

/// Expande una invocación: sustitución textual de `%param` por su argumento
/// y re-proceso de cada línea del cuerpo (una macro puede invocar a otra;
/// `MAX_EXPANSIONES` corta la recursión). Las líneas expandidas se
/// atribuyen a `main_line` de la invocación; sus errores reportan el cuerpo
/// real (archivo/línea de la definición).
fn expandir_macro(
    pp: &mut Pp,
    nombre: &str,
    args: &[String],
    file_idx: u16,
    line_no: u32,
    main_line: u32,
    profundidad: u32,
) {
    if profundidad >= MAX_EXPANSION_DEPTH {
        pp.push_err(
            file_idx,
            line_no,
            format!(
                "expansión de macros demasiado profunda (límite {MAX_EXPANSION_DEPTH} \
                 niveles): ¿una macro se invoca a sí misma?"
            ),
        );
        return;
    }
    let mac = &pp.macros[nombre];
    if args.len() != mac.params.len() {
        pp.push_err(
            file_idx,
            line_no,
            format!(
                "la macro «{nombre}» espera {} argumento(s) y recibió {}",
                mac.params.len(),
                args.len()
            ),
        );
        return;
    }
    // Sustitución textual, parámetros más largos primero (evita que %a
    // pise el comienzo de %abc).
    let mut orden: Vec<usize> = (0..mac.params.len()).collect();
    orden.sort_by_key(|&i| std::cmp::Reverse(mac.params[i].len()));
    let cuerpo: Vec<(String, u16, u32)> = mac
        .body
        .iter()
        .map(|(texto, bfile, blinea)| {
            let mut t = texto.clone();
            for &i in &orden {
                t = t.replace(&format!("%{}", mac.params[i]), &args[i]);
            }
            (t, *bfile, *blinea)
        })
        .collect();
    for (texto, bfile, blinea) in cuerpo {
        // Cada línea expandida pasa por el mismo pipeline: puede invocar
        // otra macro (anidamiento) o ser una línea común. Un .include
        // dentro de un cuerpo de macro no se soporta (pasaría como línea
        // común y el ensamblador lo rechaza como directiva desconocida).
        if let Some((sub_nombre, sub_args)) = parse_invocacion(texto.trim_start(), &pp.macros) {
            expandir_macro(
                pp,
                &sub_nombre,
                &sub_args,
                bfile,
                blinea,
                main_line,
                profundidad + 1,
            );
        } else {
            pp.lines.push(PpLine {
                text: texto,
                file: bfile,
                line: blinea,
                main_line,
            });
        }
    }
}
