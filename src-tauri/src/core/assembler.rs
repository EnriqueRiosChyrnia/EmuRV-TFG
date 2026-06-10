//! Ensamblador propio RV32IM.
//!
//! Decisión R1 (ver BITACORA.md): Keystone no soporta RISC-V en ninguna
//! release, así que el ensamblado es propio. Dos pasadas:
//!  1. lexer + parser por línea, cálculo de direcciones y tabla de símbolos;
//!  2. codificación de instrucciones y resolución de símbolos.
//!
//! Produce un [`Program`] con los bytes de .text y .data más el
//! [`SourceMap`] línea↔dirección, pieza central para breakpoints y
//! resaltado de la línea actual (ARQUITECTURA.md §Diseños clave 1).
//!
//! Todos los mensajes de error están en español y llevan línea y columna
//! (RF-02): son parte del valor didáctico de la herramienta.

use super::{DATA_BASE, TEXT_BASE};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

/// Error de ensamblado anclado a una posición del fuente. Línea y columna
/// son 1-based (convención de editores).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AsmError {
    pub line: u32,
    pub col: u32,
    pub msg: String,
}

impl AsmError {
    fn new(line: u32, col: u32, msg: impl Into<String>) -> Self {
        Self {
            line,
            col,
            msg: msg.into(),
        }
    }
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "línea {}, columna {}: {}", self.line, self.col, self.msg)
    }
}

/// Tabla bidireccional línea↔dirección generada por el ensamblado.
///
/// `addr_to_line` cubre cada palabra de 32 bits emitida (las
/// pseudo-instrucciones expandidas apuntan todas a su línea de origen);
/// `line_to_addr` apunta a la primera palabra emitida por la línea.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceMap {
    pub addr_to_line: HashMap<u32, u32>,
    pub line_to_addr: BTreeMap<u32, u32>,
}

impl SourceMap {
    pub fn line_for_addr(&self, addr: u32) -> Option<u32> {
        self.addr_to_line.get(&addr).copied()
    }
    pub fn addr_for_line(&self, line: u32) -> Option<u32> {
        self.line_to_addr.get(&line).copied()
    }
}

/// Resultado de un ensamblado exitoso.
#[derive(Debug, Clone, Serialize)]
pub struct Program {
    pub text: Vec<u8>,
    pub data: Vec<u8>,
    pub symbols: BTreeMap<String, u32>,
    pub source_map: SourceMap,
}

impl Program {
    /// Punto de entrada: siempre el comienzo de .text (convención RARS).
    pub fn entry(&self) -> u32 {
        TEXT_BASE
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Num(i64),
    Str(Vec<u8>),
    Comma,
    LParen,
    RParen,
    Colon,
}

#[derive(Debug, Clone)]
struct STok {
    tok: Tok,
    col: u32,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '.' || c == '$'
}
fn is_ident_cont(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.' || c == '$'
}

/// Convierte una línea en tokens. `line_no` es 1-based; las columnas se
/// cuentan en caracteres (no bytes) para que coincidan con el editor.
fn lex_line(src: &str, line_no: u32) -> Result<Vec<STok>, AsmError> {
    let mut toks = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        let col = (i + 1) as u32;
        match c {
            '#' => break, // comentario hasta fin de línea
            c if c.is_whitespace() => i += 1,
            ',' => {
                toks.push(STok {
                    tok: Tok::Comma,
                    col,
                });
                i += 1;
            }
            '(' => {
                toks.push(STok {
                    tok: Tok::LParen,
                    col,
                });
                i += 1;
            }
            ')' => {
                toks.push(STok {
                    tok: Tok::RParen,
                    col,
                });
                i += 1;
            }
            ':' => {
                toks.push(STok {
                    tok: Tok::Colon,
                    col,
                });
                i += 1;
            }
            '"' => {
                let (bytes, next) = lex_string(&chars, i, line_no)?;
                toks.push(STok {
                    tok: Tok::Str(bytes),
                    col,
                });
                i = next;
            }
            '\'' => {
                let (val, next) = lex_char(&chars, i, line_no)?;
                toks.push(STok {
                    tok: Tok::Num(val),
                    col,
                });
                i = next;
            }
            '%' => {
                return Err(AsmError::new(
                    line_no,
                    col,
                    "los operadores %hi/%lo no están soportados; usá «la» para cargar direcciones",
                ));
            }
            '-' | '+' => {
                let (val, next) = lex_number(&chars, i, line_no)?;
                toks.push(STok {
                    tok: Tok::Num(val),
                    col,
                });
                i = next;
            }
            c if c.is_ascii_digit() => {
                let (val, next) = lex_number(&chars, i, line_no)?;
                toks.push(STok {
                    tok: Tok::Num(val),
                    col,
                });
                i = next;
            }
            c if is_ident_start(c) => {
                let start = i;
                while i < chars.len() && is_ident_cont(chars[i]) {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                toks.push(STok {
                    tok: Tok::Ident(ident),
                    col,
                });
            }
            other => {
                return Err(AsmError::new(
                    line_no,
                    col,
                    format!("carácter inesperado: «{other}»"),
                ));
            }
        }
    }
    Ok(toks)
}

/// Número decimal, hexadecimal (0x), binario (0b) u octal (0o), con signo.
fn lex_number(chars: &[char], start: usize, line_no: u32) -> Result<(i64, usize), AsmError> {
    let col = (start + 1) as u32;
    let mut i = start;
    let mut neg = false;
    if chars[i] == '-' || chars[i] == '+' {
        neg = chars[i] == '-';
        i += 1;
        if i >= chars.len() || !chars[i].is_ascii_digit() {
            return Err(AsmError::new(
                line_no,
                col,
                "se esperaba un número tras el signo",
            ));
        }
    }
    let (radix, digits_start) = if i + 1 < chars.len() && chars[i] == '0' {
        match chars[i + 1] {
            'x' | 'X' => (16, i + 2),
            'b' | 'B' => (2, i + 2),
            'o' | 'O' => (8, i + 2),
            _ => (10, i),
        }
    } else {
        (10, i)
    };
    let mut j = digits_start;
    let mut text = String::new();
    while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
        if chars[j] != '_' {
            text.push(chars[j]);
        }
        j += 1;
    }
    if text.is_empty() {
        return Err(AsmError::new(line_no, col, "número mal formado"));
    }
    let mag = i64::from_str_radix(&text, radix).map_err(|_| {
        AsmError::new(
            line_no,
            col,
            format!("número mal formado o demasiado grande: «{}»", text),
        )
    })?;
    Ok((if neg { -mag } else { mag }, j))
}

/// Literal de carácter `'a'` o `'\n'`; vale como número (su byte).
fn lex_char(chars: &[char], start: usize, line_no: u32) -> Result<(i64, usize), AsmError> {
    let col = (start + 1) as u32;
    let mut i = start + 1;
    if i >= chars.len() {
        return Err(AsmError::new(
            line_no,
            col,
            "literal de carácter sin cerrar",
        ));
    }
    let value: i64;
    if chars[i] == '\\' {
        i += 1;
        if i >= chars.len() {
            return Err(AsmError::new(
                line_no,
                col,
                "literal de carácter sin cerrar",
            ));
        }
        value = escape_byte(chars[i]).ok_or_else(|| {
            AsmError::new(
                line_no,
                col,
                format!("secuencia de escape desconocida: «\\{}»", chars[i]),
            )
        })? as i64;
        i += 1;
    } else {
        let mut buf = [0u8; 4];
        let encoded = chars[i].encode_utf8(&mut buf);
        if encoded.len() != 1 {
            return Err(AsmError::new(
                line_no,
                col,
                "solo se admiten caracteres ASCII en literales de carácter",
            ));
        }
        value = buf[0] as i64;
        i += 1;
    }
    if i >= chars.len() || chars[i] != '\'' {
        return Err(AsmError::new(
            line_no,
            col,
            "literal de carácter sin cerrar",
        ));
    }
    Ok((value, i + 1))
}

fn escape_byte(c: char) -> Option<u8> {
    Some(match c {
        'n' => b'\n',
        't' => b'\t',
        'r' => b'\r',
        '0' => 0,
        '\\' => b'\\',
        '"' => b'"',
        '\'' => b'\'',
        _ => return None,
    })
}

/// Cadena entre comillas con escapes; devuelve los bytes UTF-8.
fn lex_string(chars: &[char], start: usize, line_no: u32) -> Result<(Vec<u8>, usize), AsmError> {
    let col = (start + 1) as u32;
    let mut bytes = Vec::new();
    let mut i = start + 1;
    loop {
        if i >= chars.len() {
            return Err(AsmError::new(
                line_no,
                col,
                "cadena sin cerrar (falta «\"»)",
            ));
        }
        match chars[i] {
            '"' => return Ok((bytes, i + 1)),
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    return Err(AsmError::new(
                        line_no,
                        col,
                        "cadena sin cerrar (falta «\"»)",
                    ));
                }
                let b = escape_byte(chars[i]).ok_or_else(|| {
                    AsmError::new(
                        line_no,
                        (i + 1) as u32,
                        format!("secuencia de escape desconocida: «\\{}»", chars[i]),
                    )
                })?;
                bytes.push(b);
                i += 1;
            }
            c => {
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                i += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Registros
// ---------------------------------------------------------------------------

/// Traduce un nombre de registro (ABI o xN) a su número, si lo es.
pub fn reg_number(name: &str) -> Option<u8> {
    let n = match name {
        "zero" => 0,
        "ra" => 1,
        "sp" => 2,
        "gp" => 3,
        "tp" => 4,
        "t0" => 5,
        "t1" => 6,
        "t2" => 7,
        "s0" | "fp" => 8,
        "s1" => 9,
        "a0" => 10,
        "a1" => 11,
        "a2" => 12,
        "a3" => 13,
        "a4" => 14,
        "a5" => 15,
        "a6" => 16,
        "a7" => 17,
        "s2" => 18,
        "s3" => 19,
        "s4" => 20,
        "s5" => 21,
        "s6" => 22,
        "s7" => 23,
        "s8" => 24,
        "s9" => 25,
        "s10" => 26,
        "s11" => 27,
        "t3" => 28,
        "t4" => 29,
        "t5" => 30,
        "t6" => 31,
        _ => {
            let rest = name.strip_prefix('x')?;
            let n: u8 = rest.parse().ok()?;
            if n > 31 {
                return None;
            }
            n
        }
    };
    Some(n)
}

/// Nombres ABI en orden x0..x31 (los usa la vista de registros y la ayuda).
pub const ABI_NAMES: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];

// ---------------------------------------------------------------------------
// Parser de sentencias
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum OpKind {
    Reg(u8),
    Imm(i64),
    Sym(String),
    Mem { off: i64, base: u8 },
}

#[derive(Debug, Clone)]
struct POp {
    kind: OpKind,
    col: u32,
}

#[derive(Debug, Clone)]
struct PInstr {
    mnem: String,
    ops: Vec<POp>,
    line: u32,
    addr: u32,
    size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Segment {
    Text,
    Data,
}

/// Fixup pendiente en .data (p. ej. `.word etiqueta`).
struct DataFixup {
    offset: usize,
    sym: String,
    line: u32,
    col: u32,
}

struct Pass1 {
    segment: Segment,
    text_off: u32,
    data: Vec<u8>,
    instrs: Vec<PInstr>,
    symbols: BTreeMap<String, u32>,
    sym_lines: HashMap<String, u32>,
    fixups: Vec<DataFixup>,
    errors: Vec<AsmError>,
}

/// Ensambla el código fuente completo. Devuelve el programa o TODOS los
/// errores encontrados (no solo el primero), ordenados por línea.
pub fn assemble(source: &str) -> Result<Program, Vec<AsmError>> {
    let mut p = Pass1 {
        segment: Segment::Text,
        text_off: 0,
        data: Vec::new(),
        instrs: Vec::new(),
        symbols: BTreeMap::new(),
        sym_lines: HashMap::new(),
        fixups: Vec::new(),
        errors: Vec::new(),
    };

    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        let toks = match lex_line(raw_line, line_no) {
            Ok(t) => t,
            Err(e) => {
                p.errors.push(e);
                continue;
            }
        };
        if let Err(e) = parse_line(&mut p, &toks, line_no) {
            p.errors.push(e);
        }
    }

    if p.instrs.is_empty() && p.errors.is_empty() {
        p.errors.push(AsmError::new(
            1,
            1,
            "el programa no tiene instrucciones: agregá código en la sección .text",
        ));
    }

    // Pasada 2: codificación y resolución de símbolos.
    let mut text = Vec::with_capacity((p.text_off as usize).max(4));
    let mut source_map = SourceMap::default();
    for ins in &p.instrs {
        match encode_instr(ins, &p.symbols) {
            Ok(words) => {
                debug_assert_eq!(words.len() as u32 * 4, ins.size);
                for (k, w) in words.iter().enumerate() {
                    let addr = ins.addr + (k as u32) * 4;
                    source_map.addr_to_line.insert(addr, ins.line);
                    text.extend_from_slice(&w.to_le_bytes());
                }
                source_map.line_to_addr.entry(ins.line).or_insert(ins.addr);
            }
            Err(e) => p.errors.push(e),
        }
    }

    // Fixups de datos (.word etiqueta).
    for f in &p.fixups {
        match p.symbols.get(&f.sym) {
            Some(&v) => p.data[f.offset..f.offset + 4].copy_from_slice(&v.to_le_bytes()),
            None => p.errors.push(AsmError::new(
                f.line,
                f.col,
                format!("etiqueta no definida: «{}»", f.sym),
            )),
        }
    }

    if p.errors.is_empty() {
        Ok(Program {
            text,
            data: p.data,
            symbols: p.symbols,
            source_map,
        })
    } else {
        p.errors.sort_by_key(|e| (e.line, e.col));
        Err(p.errors)
    }
}

fn cur_addr(p: &Pass1) -> u32 {
    match p.segment {
        Segment::Text => TEXT_BASE + p.text_off,
        Segment::Data => DATA_BASE + p.data.len() as u32,
    }
}

fn define_label(p: &mut Pass1, name: &str, line: u32, col: u32) -> Result<(), AsmError> {
    if let Some(prev) = p.sym_lines.get(name) {
        return Err(AsmError::new(
            line,
            col,
            format!("etiqueta duplicada: «{name}» (ya definida en la línea {prev})"),
        ));
    }
    p.symbols.insert(name.to_string(), cur_addr(p));
    p.sym_lines.insert(name.to_string(), line);
    Ok(())
}

fn parse_line(p: &mut Pass1, toks: &[STok], line: u32) -> Result<(), AsmError> {
    let mut i = 0usize;
    // Etiquetas al comienzo: `nombre:` (puede haber varias). Se acumulan y
    // se definen DESPUÉS de la alineación automática de la directiva que
    // las acompañe, para que `w: .word 7` apunte al word y no al relleno.
    let mut labels: Vec<(String, u32)> = Vec::new();
    while i + 1 < toks.len() {
        if let (Tok::Ident(name), Tok::Colon) = (&toks[i].tok, &toks[i + 1].tok) {
            if name.starts_with('.') {
                break; // una directiva nunca es etiqueta
            }
            labels.push((name.clone(), toks[i].col));
            i += 2;
        } else {
            break;
        }
    }
    if i >= toks.len() {
        define_labels(p, &labels, line)?;
        return Ok(());
    }
    let head = &toks[i];
    let rest = &toks[i + 1..];
    match &head.tok {
        Tok::Ident(name) if name.starts_with('.') => {
            parse_directive(p, name, head.col, rest, line, &labels)
        }
        Tok::Ident(name) => {
            define_labels(p, &labels, line)?;
            parse_instruction(p, name, head.col, rest, line)
        }
        Tok::Colon => Err(AsmError::new(
            line,
            head.col,
            "se encontró «:» sin nombre de etiqueta delante",
        )),
        _ => Err(AsmError::new(
            line,
            head.col,
            "se esperaba una instrucción, directiva o etiqueta al comienzo de la línea",
        )),
    }
}

fn define_labels(p: &mut Pass1, labels: &[(String, u32)], line: u32) -> Result<(), AsmError> {
    for (name, col) in labels {
        define_label(p, name, line, *col)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Directivas
// ---------------------------------------------------------------------------

/// Divide los tokens restantes en argumentos separados por coma.
fn split_args(toks: &[STok]) -> Vec<&[STok]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, t) in toks.iter().enumerate() {
        if t.tok == Tok::Comma {
            out.push(&toks[start..i]);
            start = i + 1;
        }
    }
    if start < toks.len() {
        out.push(&toks[start..]);
    } else if !toks.is_empty() && start == toks.len() {
        out.push(&toks[toks.len()..]); // coma final: argumento vacío → error después
    }
    out
}

fn align_data(p: &mut Pass1, align: usize) {
    while !p.data.len().is_multiple_of(align) {
        p.data.push(0);
    }
}

fn parse_directive(
    p: &mut Pass1,
    name: &str,
    col: u32,
    rest: &[STok],
    line: u32,
    labels: &[(String, u32)],
) -> Result<(), AsmError> {
    match name {
        ".text" => {
            define_labels(p, labels, line)?;
            p.segment = Segment::Text;
            Ok(())
        }
        ".data" => {
            define_labels(p, labels, line)?;
            p.segment = Segment::Data;
            Ok(())
        }
        ".globl" | ".global" => {
            define_labels(p, labels, line)?;
            Ok(()) // aceptada y sin efecto: el punto de entrada es fijo
        }
        ".align" => {
            let n = expect_one_number(rest, line, col, ".align")?;
            if !(0..=12).contains(&n) {
                return Err(AsmError::new(
                    line,
                    col,
                    ".align espera un valor entre 0 y 12",
                ));
            }
            if p.segment == Segment::Data {
                align_data(p, 1usize << n);
            }
            define_labels(p, labels, line)?;
            Ok(())
        }
        ".space" => {
            require_data_segment(p, name, line, col)?;
            define_labels(p, labels, line)?;
            let n = expect_one_number(rest, line, col, ".space")?;
            if !(0..=1_048_576).contains(&n) {
                return Err(AsmError::new(
                    line,
                    col,
                    ".space espera un tamaño entre 0 y 1048576 bytes",
                ));
            }
            p.data.extend(std::iter::repeat_n(0u8, n as usize));
            Ok(())
        }
        ".word" | ".half" | ".byte" => {
            require_data_segment(p, name, line, col)?;
            let (size, min, max): (usize, i64, i64) = match name {
                ".word" => (4, i32::MIN as i64, u32::MAX as i64),
                ".half" => (2, i16::MIN as i64, u16::MAX as i64),
                _ => (1, i8::MIN as i64, u8::MAX as i64),
            };
            align_data(p, size);
            define_labels(p, labels, line)?;
            let args = split_args(rest);
            if args.is_empty() {
                return Err(AsmError::new(
                    line,
                    col,
                    format!("{name} espera al menos un valor"),
                ));
            }
            for arg in args {
                match arg {
                    [STok {
                        tok: Tok::Num(v),
                        col: vcol,
                    }] => {
                        if *v < min || *v > max {
                            return Err(AsmError::new(
                                line,
                                *vcol,
                                format!("el valor {v} no cabe en {name} ({min}..{max})"),
                            ));
                        }
                        p.data.extend_from_slice(&v.to_le_bytes()[..size]);
                    }
                    [STok {
                        tok: Tok::Ident(sym),
                        col: scol,
                    }] if name == ".word" => {
                        p.fixups.push(DataFixup {
                            offset: p.data.len(),
                            sym: sym.clone(),
                            line,
                            col: *scol,
                        });
                        p.data.extend_from_slice(&[0; 4]);
                    }
                    [] => {
                        return Err(AsmError::new(
                            line,
                            col,
                            format!("argumento vacío en {name} (¿coma de más?)"),
                        ));
                    }
                    other => {
                        return Err(AsmError::new(
                            line,
                            other[0].col,
                            format!("{name} espera valores numéricos separados por coma"),
                        ));
                    }
                }
            }
            Ok(())
        }
        ".ascii" | ".asciiz" | ".string" => {
            require_data_segment(p, name, line, col)?;
            define_labels(p, labels, line)?;
            match rest {
                [STok {
                    tok: Tok::Str(bytes),
                    ..
                }] => {
                    p.data.extend_from_slice(bytes);
                    if name != ".ascii" {
                        p.data.push(0); // terminador NUL
                    }
                    Ok(())
                }
                _ => Err(AsmError::new(
                    line,
                    col,
                    format!("{name} espera una cadena entre comillas dobles"),
                )),
            }
        }
        other => Err(AsmError::new(
            line,
            col,
            format!(
                "directiva desconocida: «{other}». Soportadas: .text .data .word .half .byte \
                 .ascii .asciiz .string .space .align .globl"
            ),
        )),
    }
}

fn require_data_segment(p: &Pass1, name: &str, line: u32, col: u32) -> Result<(), AsmError> {
    if p.segment != Segment::Data {
        return Err(AsmError::new(
            line,
            col,
            format!("{name} solo puede usarse en la sección .data"),
        ));
    }
    Ok(())
}

fn expect_one_number(rest: &[STok], line: u32, col: u32, dir: &str) -> Result<i64, AsmError> {
    match rest {
        [STok {
            tok: Tok::Num(v), ..
        }] => Ok(*v),
        _ => Err(AsmError::new(
            line,
            col,
            format!("{dir} espera exactamente un número"),
        )),
    }
}

// ---------------------------------------------------------------------------
// Instrucciones: parsing de operandos
// ---------------------------------------------------------------------------

fn parse_instruction(
    p: &mut Pass1,
    mnem: &str,
    col: u32,
    rest: &[STok],
    line: u32,
) -> Result<(), AsmError> {
    if p.segment != Segment::Text {
        return Err(AsmError::new(
            line,
            col,
            format!("la instrucción «{mnem}» está en .data; las instrucciones van en .text"),
        ));
    }
    let mnem = mnem.to_lowercase();
    let mut ops = Vec::new();
    for arg in split_args(rest) {
        ops.push(parse_operand(arg, line)?);
    }
    let size = instr_size(&mnem, &ops, line, col)?;
    let addr = TEXT_BASE + p.text_off;
    p.text_off += size;
    p.instrs.push(PInstr {
        mnem,
        ops,
        line,
        addr,
        size,
    });
    Ok(())
}

fn parse_operand(toks: &[STok], line: u32) -> Result<POp, AsmError> {
    match toks {
        [STok {
            tok: Tok::Ident(name),
            col,
        }] => match reg_number(name) {
            Some(r) => Ok(POp {
                kind: OpKind::Reg(r),
                col: *col,
            }),
            None => Ok(POp {
                kind: OpKind::Sym(name.clone()),
                col: *col,
            }),
        },
        [STok {
            tok: Tok::Num(v),
            col,
        }] => Ok(POp {
            kind: OpKind::Imm(*v),
            col: *col,
        }),
        // desplazamiento(base): 4(sp), -8(s0), (t0)
        [STok {
            tok: Tok::Num(off),
            col,
        }, STok {
            tok: Tok::LParen, ..
        }, STok {
            tok: Tok::Ident(base),
            col: bcol,
        }, STok {
            tok: Tok::RParen, ..
        }] => {
            let base = reg_number(base).ok_or_else(|| {
                AsmError::new(line, *bcol, format!("registro inválido: «{base}»"))
            })?;
            Ok(POp {
                kind: OpKind::Mem { off: *off, base },
                col: *col,
            })
        }
        [STok {
            tok: Tok::LParen,
            col,
        }, STok {
            tok: Tok::Ident(base),
            col: bcol,
        }, STok {
            tok: Tok::RParen, ..
        }] => {
            let base = reg_number(base).ok_or_else(|| {
                AsmError::new(line, *bcol, format!("registro inválido: «{base}»"))
            })?;
            Ok(POp {
                kind: OpKind::Mem { off: 0, base },
                col: *col,
            })
        }
        [] => Err(AsmError::new(line, 1, "operando vacío (¿coma de más?)")),
        other => Err(AsmError::new(
            line,
            other[0].col,
            "operando mal formado: se esperaba registro, número, etiqueta o desplazamiento(base)",
        )),
    }
}

// ---------------------------------------------------------------------------
// Tamaño de cada instrucción (pasada 1)
// ---------------------------------------------------------------------------

/// ¿Cabe en inmediato de 12 bits con signo?
fn fits12(v: i64) -> bool {
    (-2048..=2047).contains(&v)
}

/// Mnemónicos reales de 4 bytes con su validación de forma en
/// `check_shape`. Las pseudo-instrucciones que expanden a 2 palabras se
/// listan aparte.
fn instr_size(mnem: &str, ops: &[POp], line: u32, col: u32) -> Result<u32, AsmError> {
    check_shape(mnem, ops, line, col)?;
    let size = match mnem {
        "li" => match &ops[1].kind {
            OpKind::Imm(v) if fits12(*v) => 4,
            _ => 8,
        },
        "la" | "lla" => 8,
        // cargas/almacenamientos con etiqueta expanden a lui+op
        "lb" | "lh" | "lw" | "lbu" | "lhu" => match &ops[1].kind {
            OpKind::Sym(_) => 8,
            _ => 4,
        },
        "sb" | "sh" | "sw" => match &ops[1].kind {
            OpKind::Sym(_) => 8,
            _ => 4,
        },
        _ => 4,
    };
    Ok(size)
}

/// Valida cantidad y forma de operandos en la pasada 1, para que la
/// pasada 2 solo pueda fallar por símbolos o rangos.
fn check_shape(mnem: &str, ops: &[POp], line: u32, col: u32) -> Result<(), AsmError> {
    use OpKind::*;
    let err = |msg: String| Err(AsmError::new(line, col, msg));
    let n = ops.len();
    let want = |k: usize, forma: &str| -> Result<(), AsmError> {
        if n != k {
            return Err(AsmError::new(
                line,
                col,
                format!("«{mnem}» espera {k} operandos ({forma}); hay {n}"),
            ));
        }
        Ok(())
    };
    let is_reg = |i: usize| matches!(ops[i].kind, Reg(_));
    let is_imm = |i: usize| matches!(ops[i].kind, Imm(_));
    let is_sym = |i: usize| matches!(ops[i].kind, Sym(_));
    let is_mem = |i: usize| matches!(ops[i].kind, Mem { .. });

    match mnem {
        // R: rd, rs1, rs2
        "add" | "sub" | "sll" | "slt" | "sltu" | "xor" | "srl" | "sra" | "or" | "and" | "mul"
        | "mulh" | "mulhsu" | "mulhu" | "div" | "divu" | "rem" | "remu" => {
            want(3, "registro, registro, registro")?;
            if !(is_reg(0) && is_reg(1) && is_reg(2)) {
                return err(format!(
                    "«{mnem}» espera tres registros (ej.: add t0, t1, t2)"
                ));
            }
        }
        // I aritmética: rd, rs1, imm
        "addi" | "slti" | "sltiu" | "xori" | "ori" | "andi" | "slli" | "srli" | "srai" => {
            want(3, "registro, registro, inmediato")?;
            if !(is_reg(0) && is_reg(1) && is_imm(2)) {
                return err(format!(
                    "«{mnem}» espera registro, registro e inmediato (ej.: addi t0, t1, 10)"
                ));
            }
        }
        // cargas: rd, off(base) | rd, etiqueta
        "lb" | "lh" | "lw" | "lbu" | "lhu" => {
            want(2, "registro, desplazamiento(base) o etiqueta")?;
            if !is_reg(0) || !(is_mem(1) || is_sym(1)) {
                return err(format!(
                    "«{mnem}» espera registro y desplazamiento(base) o etiqueta \
                     (ej.: lw t0, 4(sp) o lw t0, dato)"
                ));
            }
        }
        // almacenamientos: rs2, off(base) | rs2, etiqueta, reg_tmp
        "sb" | "sh" | "sw" => {
            if n == 2 {
                if !is_reg(0) || !is_mem(1) {
                    return err(format!(
                        "«{mnem}» espera registro y desplazamiento(base) (ej.: sw t0, 4(sp))"
                    ));
                }
            } else if n == 3 {
                if !is_reg(0) || !is_sym(1) || !is_reg(2) {
                    return err(format!(
                        "«{mnem}» con etiqueta espera registro, etiqueta y registro auxiliar \
                         (ej.: sw t0, dato, t1)"
                    ));
                }
            } else {
                return err(format!(
                    "«{mnem}» espera 2 operandos (sw t0, 4(sp)) o 3 con etiqueta (sw t0, dato, t1); hay {n}"
                ));
            }
        }
        // saltos condicionales: rs1, rs2, etiqueta
        "beq" | "bne" | "blt" | "bge" | "bltu" | "bgeu" | "bgt" | "ble" | "bgtu" | "bleu" => {
            want(3, "registro, registro, etiqueta")?;
            if !(is_reg(0) && is_reg(1) && (is_sym(2) || is_imm(2))) {
                return err(format!(
                    "«{mnem}» espera dos registros y una etiqueta (ej.: beq t0, t1, fin)"
                ));
            }
        }
        // comparación con cero: rs1, etiqueta
        "beqz" | "bnez" | "blez" | "bgez" | "bltz" | "bgtz" => {
            want(2, "registro, etiqueta")?;
            if !(is_reg(0) && (is_sym(1) || is_imm(1))) {
                return err(format!(
                    "«{mnem}» espera un registro y una etiqueta (ej.: beqz t0, fin)"
                ));
            }
        }
        "lui" | "auipc" => {
            want(2, "registro, inmediato")?;
            if !(is_reg(0) && is_imm(1)) {
                return err(format!("«{mnem}» espera registro e inmediato de 20 bits"));
            }
        }
        "jal" => match n {
            1 if is_sym(0) || is_imm(0) => {}
            2 if is_reg(0) && (is_sym(1) || is_imm(1)) => {}
            _ => return err(
                "«jal» espera una etiqueta (jal funcion) o registro y etiqueta (jal ra, funcion)"
                    .into(),
            ),
        },
        "jalr" => match n {
            1 if is_reg(0) => {}
            2 if is_reg(0) && is_mem(1) => {}
            3 if is_reg(0) && is_reg(1) && is_imm(2) => {}
            _ => {
                return err(
                    "«jalr» espera un registro (jalr t0), registro y desplazamiento(base) \
                     (jalr ra, 0(t0)) o tres operandos (jalr ra, t0, 0)"
                        .into(),
                )
            }
        },
        "j" | "call" | "tail" => {
            want(1, "etiqueta")?;
            if !(is_sym(0) || is_imm(0)) {
                return err(format!("«{mnem}» espera una etiqueta (ej.: {mnem} bucle)"));
            }
        }
        "li" => {
            want(2, "registro, inmediato")?;
            if !(is_reg(0) && is_imm(1)) {
                if is_reg(0) && is_sym(1) {
                    return err(
                        "«li» carga constantes numéricas; para direcciones de etiquetas usá «la»"
                            .into(),
                    );
                }
                return err("«li» espera registro e inmediato (ej.: li a0, 42)".into());
            }
            if let Imm(v) = ops[1].kind {
                if v < i32::MIN as i64 || v > u32::MAX as i64 {
                    return Err(AsmError::new(
                        line,
                        ops[1].col,
                        format!("el inmediato {v} no cabe en 32 bits"),
                    ));
                }
            }
        }
        "la" | "lla" => {
            want(2, "registro, etiqueta")?;
            if !(is_reg(0) && is_sym(1)) {
                return err("«la» espera registro y etiqueta (ej.: la a0, mensaje)".into());
            }
        }
        "mv" | "not" | "neg" | "seqz" | "snez" | "sltz" | "sgtz" => {
            want(2, "registro, registro")?;
            if !(is_reg(0) && is_reg(1)) {
                return err(format!(
                    "«{mnem}» espera dos registros (ej.: {mnem} t0, t1)"
                ));
            }
        }
        "jr" => {
            want(1, "registro")?;
            if !is_reg(0) {
                return err("«jr» espera un registro (ej.: jr ra)".into());
            }
        }
        "nop" | "ret" | "ecall" | "ebreak" | "fence" => {
            want(0, "sin operandos")?;
        }
        other => {
            return err(format!(
                "instrucción desconocida: «{other}». EmuRV soporta RV32IM \
                 (enteros y multiplicación/división de 32 bits)"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Codificación (pasada 2)
// ---------------------------------------------------------------------------

fn enc_r(f7: u32, rs2: u8, rs1: u8, f3: u32, rd: u8, op: u32) -> u32 {
    (f7 << 25) | ((rs2 as u32) << 20) | ((rs1 as u32) << 15) | (f3 << 12) | ((rd as u32) << 7) | op
}
fn enc_i(imm: i32, rs1: u8, f3: u32, rd: u8, op: u32) -> u32 {
    ((imm as u32 & 0xFFF) << 20) | ((rs1 as u32) << 15) | (f3 << 12) | ((rd as u32) << 7) | op
}
fn enc_s(imm: i32, rs2: u8, rs1: u8, f3: u32) -> u32 {
    let imm = imm as u32 & 0xFFF;
    ((imm >> 5) << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (f3 << 12)
        | ((imm & 0x1F) << 7)
        | 0x23
}
fn enc_b(off: i32, rs2: u8, rs1: u8, f3: u32) -> u32 {
    let o = off as u32;
    (((o >> 12) & 1) << 31)
        | (((o >> 5) & 0x3F) << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (f3 << 12)
        | (((o >> 1) & 0xF) << 8)
        | (((o >> 11) & 1) << 7)
        | 0x63
}
fn enc_u(imm20: u32, rd: u8, op: u32) -> u32 {
    (imm20 << 12) | ((rd as u32) << 7) | op
}
fn enc_j(off: i32, rd: u8) -> u32 {
    let o = off as u32;
    (((o >> 20) & 1) << 31)
        | (((o >> 1) & 0x3FF) << 21)
        | (((o >> 11) & 1) << 20)
        | (((o >> 12) & 0xFF) << 12)
        | ((rd as u32) << 7)
        | 0x6F
}

/// Parte alta para lui en el par lui+addi (compensa el signo del addi).
fn hi20(v: u32) -> u32 {
    v.wrapping_add(0x800) >> 12
}
/// Parte baja con signo para addi/lw/sw en el par con lui.
fn lo12(v: u32) -> i32 {
    ((v as i32) << 20) >> 20
}

struct Ctx<'a> {
    ins: &'a PInstr,
    symbols: &'a BTreeMap<String, u32>,
}

impl Ctx<'_> {
    fn reg(&self, i: usize) -> u8 {
        match self.ins.ops[i].kind {
            OpKind::Reg(r) => r,
            _ => unreachable!("forma validada en pasada 1"),
        }
    }
    fn imm(&self, i: usize) -> i64 {
        match self.ins.ops[i].kind {
            OpKind::Imm(v) => v,
            _ => unreachable!("forma validada en pasada 1"),
        }
    }
    fn mem(&self, i: usize) -> (i64, u8) {
        match &self.ins.ops[i].kind {
            OpKind::Mem { off, base } => (*off, *base),
            _ => unreachable!("forma validada en pasada 1"),
        }
    }
    /// Resuelve una etiqueta (u offset numérico absoluto) a dirección.
    fn target(&self, i: usize) -> Result<u32, AsmError> {
        match &self.ins.ops[i].kind {
            OpKind::Sym(s) => self.symbols.get(s).copied().ok_or_else(|| {
                AsmError::new(
                    self.ins.line,
                    self.ins.ops[i].col,
                    format!("etiqueta no definida: «{s}»"),
                )
            }),
            OpKind::Imm(v) => Ok(*v as u32),
            _ => unreachable!("forma validada en pasada 1"),
        }
    }
    fn check_imm12(&self, i: usize, v: i64) -> Result<i32, AsmError> {
        if !fits12(v) {
            return Err(AsmError::new(
                self.ins.line,
                self.ins.ops[i].col,
                format!(
                    "inmediato fuera de rango: {v} no cabe en 12 bits con signo (−2048..2047); \
                     para valores grandes usá «li» en un registro"
                ),
            ));
        }
        Ok(v as i32)
    }
    fn branch_off(&self, i: usize) -> Result<i32, AsmError> {
        let target = self.target(i)?;
        let off = target.wrapping_sub(self.ins.addr) as i32;
        if !(-4096..=4094).contains(&off) {
            return Err(AsmError::new(
                self.ins.line,
                self.ins.ops[i].col,
                format!(
                    "salto condicional fuera de alcance: el destino está a {off} bytes \
                     y el máximo es ±4 KB; usá un salto incondicional «j» intermedio"
                ),
            ));
        }
        Ok(off)
    }
    fn jal_off(&self, i: usize) -> Result<i32, AsmError> {
        let target = self.target(i)?;
        let off = target.wrapping_sub(self.ins.addr) as i32;
        if !(-1_048_576..=1_048_574).contains(&off) {
            return Err(AsmError::new(
                self.ins.line,
                self.ins.ops[i].col,
                format!(
                    "salto fuera de alcance: el destino está a {off} bytes y el máximo es ±1 MB"
                ),
            ));
        }
        Ok(off)
    }
    fn shamt(&self, i: usize) -> Result<u8, AsmError> {
        let v = self.imm(i);
        if !(0..=31).contains(&v) {
            return Err(AsmError::new(
                self.ins.line,
                self.ins.ops[i].col,
                format!("el desplazamiento de bits debe estar entre 0 y 31 (hay {v})"),
            ));
        }
        Ok(v as u8)
    }
}

fn encode_instr(ins: &PInstr, symbols: &BTreeMap<String, u32>) -> Result<Vec<u32>, AsmError> {
    let c = Ctx { ins, symbols };
    let m = ins.mnem.as_str();
    let w = match m {
        // ---- R-type ----
        "add" => enc_r(0x00, c.reg(2), c.reg(1), 0, c.reg(0), 0x33),
        "sub" => enc_r(0x20, c.reg(2), c.reg(1), 0, c.reg(0), 0x33),
        "sll" => enc_r(0x00, c.reg(2), c.reg(1), 1, c.reg(0), 0x33),
        "slt" => enc_r(0x00, c.reg(2), c.reg(1), 2, c.reg(0), 0x33),
        "sltu" => enc_r(0x00, c.reg(2), c.reg(1), 3, c.reg(0), 0x33),
        "xor" => enc_r(0x00, c.reg(2), c.reg(1), 4, c.reg(0), 0x33),
        "srl" => enc_r(0x00, c.reg(2), c.reg(1), 5, c.reg(0), 0x33),
        "sra" => enc_r(0x20, c.reg(2), c.reg(1), 5, c.reg(0), 0x33),
        "or" => enc_r(0x00, c.reg(2), c.reg(1), 6, c.reg(0), 0x33),
        "and" => enc_r(0x00, c.reg(2), c.reg(1), 7, c.reg(0), 0x33),
        // ---- RV32M ----
        "mul" => enc_r(0x01, c.reg(2), c.reg(1), 0, c.reg(0), 0x33),
        "mulh" => enc_r(0x01, c.reg(2), c.reg(1), 1, c.reg(0), 0x33),
        "mulhsu" => enc_r(0x01, c.reg(2), c.reg(1), 2, c.reg(0), 0x33),
        "mulhu" => enc_r(0x01, c.reg(2), c.reg(1), 3, c.reg(0), 0x33),
        "div" => enc_r(0x01, c.reg(2), c.reg(1), 4, c.reg(0), 0x33),
        "divu" => enc_r(0x01, c.reg(2), c.reg(1), 5, c.reg(0), 0x33),
        "rem" => enc_r(0x01, c.reg(2), c.reg(1), 6, c.reg(0), 0x33),
        "remu" => enc_r(0x01, c.reg(2), c.reg(1), 7, c.reg(0), 0x33),
        // ---- I-type aritmética ----
        "addi" => enc_i(c.check_imm12(2, c.imm(2))?, c.reg(1), 0, c.reg(0), 0x13),
        "slti" => enc_i(c.check_imm12(2, c.imm(2))?, c.reg(1), 2, c.reg(0), 0x13),
        "sltiu" => enc_i(c.check_imm12(2, c.imm(2))?, c.reg(1), 3, c.reg(0), 0x13),
        "xori" => enc_i(c.check_imm12(2, c.imm(2))?, c.reg(1), 4, c.reg(0), 0x13),
        "ori" => enc_i(c.check_imm12(2, c.imm(2))?, c.reg(1), 6, c.reg(0), 0x13),
        "andi" => enc_i(c.check_imm12(2, c.imm(2))?, c.reg(1), 7, c.reg(0), 0x13),
        "slli" => enc_i(c.shamt(2)? as i32, c.reg(1), 1, c.reg(0), 0x13),
        "srli" => enc_i(c.shamt(2)? as i32, c.reg(1), 5, c.reg(0), 0x13),
        "srai" => enc_i((c.shamt(2)? as i32) | 0x400, c.reg(1), 5, c.reg(0), 0x13),
        // ---- U / J ----
        "lui" | "auipc" => {
            let v = c.imm(1);
            if !(-(1 << 19)..(1 << 20)).contains(&v) {
                return Err(AsmError::new(
                    ins.line,
                    ins.ops[1].col,
                    format!("el inmediato de «{m}» debe caber en 20 bits (hay {v})"),
                ));
            }
            let op = if m == "lui" { 0x37 } else { 0x17 };
            enc_u((v as u32) & 0xFFFFF, c.reg(0), op)
        }
        "jal" => {
            let (rd, ti) = if ins.ops.len() == 1 {
                (1u8, 0)
            } else {
                (c.reg(0), 1)
            };
            enc_j(c.jal_off(ti)?, rd)
        }
        "j" => enc_j(c.jal_off(0)?, 0),
        "call" => enc_j(c.jal_off(0)?, 1),
        "tail" => enc_j(c.jal_off(0)?, 0),
        "jalr" => match ins.ops.len() {
            1 => enc_i(0, c.reg(0), 0, 1, 0x67),
            2 => {
                let (off, base) = c.mem(1);
                enc_i(c.check_imm12(1, off)?, base, 0, c.reg(0), 0x67)
            }
            _ => enc_i(c.check_imm12(2, c.imm(2))?, c.reg(1), 0, c.reg(0), 0x67),
        },
        "jr" => enc_i(0, c.reg(0), 0, 0, 0x67),
        "ret" => enc_i(0, 1, 0, 0, 0x67),
        // ---- saltos condicionales ----
        "beq" => enc_b(c.branch_off(2)?, c.reg(1), c.reg(0), 0),
        "bne" => enc_b(c.branch_off(2)?, c.reg(1), c.reg(0), 1),
        "blt" => enc_b(c.branch_off(2)?, c.reg(1), c.reg(0), 4),
        "bge" => enc_b(c.branch_off(2)?, c.reg(1), c.reg(0), 5),
        "bltu" => enc_b(c.branch_off(2)?, c.reg(1), c.reg(0), 6),
        "bgeu" => enc_b(c.branch_off(2)?, c.reg(1), c.reg(0), 7),
        // pseudo con operandos invertidos
        "bgt" => enc_b(c.branch_off(2)?, c.reg(0), c.reg(1), 4),
        "ble" => enc_b(c.branch_off(2)?, c.reg(0), c.reg(1), 5),
        "bgtu" => enc_b(c.branch_off(2)?, c.reg(0), c.reg(1), 6),
        "bleu" => enc_b(c.branch_off(2)?, c.reg(0), c.reg(1), 7),
        // comparación con cero
        "beqz" => enc_b(c.branch_off(1)?, 0, c.reg(0), 0),
        "bnez" => enc_b(c.branch_off(1)?, 0, c.reg(0), 1),
        "bltz" => enc_b(c.branch_off(1)?, 0, c.reg(0), 4),
        "bgez" => enc_b(c.branch_off(1)?, 0, c.reg(0), 5),
        "blez" => enc_b(c.branch_off(1)?, c.reg(0), 0, 5), // 0 >= rs ⇔ rs <= 0
        "bgtz" => enc_b(c.branch_off(1)?, c.reg(0), 0, 4), // 0 < rs ⇔ rs > 0
        // ---- pseudo de un solo word ----
        "nop" => enc_i(0, 0, 0, 0, 0x13),
        "mv" => enc_i(0, c.reg(1), 0, c.reg(0), 0x13),
        "not" => enc_i(-1, c.reg(1), 4, c.reg(0), 0x13),
        "neg" => enc_r(0x20, c.reg(1), 0, 0, c.reg(0), 0x33),
        "seqz" => enc_i(1, c.reg(1), 3, c.reg(0), 0x13),
        "snez" => enc_r(0x00, c.reg(1), 0, 3, c.reg(0), 0x33),
        "sltz" => enc_r(0x00, 0, c.reg(1), 2, c.reg(0), 0x33),
        "sgtz" => enc_r(0x00, c.reg(1), 0, 2, c.reg(0), 0x33),
        // ---- sistema ----
        "ecall" => 0x0000_0073,
        "ebreak" => 0x0010_0073,
        "fence" => 0x0FF0_000F,
        // ---- cargas / almacenamientos / li / la: pueden expandir a 2 words
        "lb" | "lh" | "lw" | "lbu" | "lhu" => return encode_load(&c, m),
        "sb" | "sh" | "sw" => return encode_store(&c, m),
        "li" => return encode_li(&c),
        "la" | "lla" => {
            let addr = c.target(1)?;
            let rd = c.reg(0);
            return Ok(vec![
                enc_u(hi20(addr), rd, 0x37),
                enc_i(lo12(addr), rd, 0, rd, 0x13),
            ]);
        }
        _ => unreachable!("mnemónico validado en pasada 1: {m}"),
    };
    Ok(vec![w])
}

fn load_f3(m: &str) -> u32 {
    match m {
        "lb" => 0,
        "lh" => 1,
        "lw" => 2,
        "lbu" => 4,
        "lhu" => 5,
        "sb" => 0,
        "sh" => 1,
        "sw" => 2,
        _ => unreachable!(),
    }
}

fn encode_load(c: &Ctx, m: &str) -> Result<Vec<u32>, AsmError> {
    let rd = c.reg(0);
    match &c.ins.ops[1].kind {
        OpKind::Mem { off, base } => {
            let imm = c.check_imm12(1, *off)?;
            Ok(vec![enc_i(imm, *base, load_f3(m), rd, 0x03)])
        }
        OpKind::Sym(_) => {
            // lw rd, etiqueta  →  lui rd, %hi ; lw rd, %lo(rd)
            let addr = c.target(1)?;
            Ok(vec![
                enc_u(hi20(addr), rd, 0x37),
                enc_i(lo12(addr), rd, load_f3(m), rd, 0x03),
            ])
        }
        _ => unreachable!("forma validada en pasada 1"),
    }
}

fn encode_store(c: &Ctx, m: &str) -> Result<Vec<u32>, AsmError> {
    let rs2 = c.reg(0);
    if c.ins.ops.len() == 2 {
        let (off, base) = c.mem(1);
        let imm = c.check_imm12(1, off)?;
        Ok(vec![enc_s(imm, rs2, base, load_f3(m))])
    } else {
        // sw rs2, etiqueta, rt  →  lui rt, %hi ; sw rs2, %lo(rt)
        let addr = c.target(1)?;
        let rt = c.reg(2);
        if rt == 0 {
            return Err(AsmError::new(
                c.ins.line,
                c.ins.ops[2].col,
                "el registro auxiliar de un almacenamiento con etiqueta no puede ser zero/x0",
            ));
        }
        Ok(vec![
            enc_u(hi20(addr), rt, 0x37),
            enc_s(lo12(addr), rs2, rt, load_f3(m)),
        ])
    }
}

fn encode_li(c: &Ctx) -> Result<Vec<u32>, AsmError> {
    let rd = c.reg(0);
    let v = c.imm(1);
    if fits12(v) {
        return Ok(vec![enc_i(v as i32, 0, 0, rd, 0x13)]);
    }
    let bits = v as u32; // rango ya validado en pasada 1 (cabe en 32 bits)
    Ok(vec![
        enc_u(hi20(bits), rd, 0x37),
        enc_i(lo12(bits), rd, 0, rd, 0x13),
    ])
}

// ---------------------------------------------------------------------------
// Tests unitarios
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn asm_words(src: &str) -> Vec<u32> {
        let p = assemble(src).expect("debe ensamblar");
        p.text
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    fn first_error(src: &str) -> AsmError {
        assemble(src).unwrap_err().remove(0)
    }

    #[test]
    fn codifica_instrucciones_base_conocidas() {
        // Valores de referencia verificados a mano contra la espec RV32I.
        assert_eq!(asm_words("addi x1, x0, 5")[0], 0x0050_0093);
        assert_eq!(asm_words("add x3, x1, x2")[0], 0x0020_81B3);
        assert_eq!(asm_words("lui a0, 0x10010")[0], 0x1001_0537);
        assert_eq!(asm_words("lw x2, 4(x1)")[0], 0x0040_A103);
        assert_eq!(asm_words("sw x2, 4(x1)")[0], 0x0020_A223);
        assert_eq!(asm_words("mul x5, x6, x7")[0], 0x0273_02B3);
        assert_eq!(asm_words("ecall")[0], 0x0000_0073);
        assert_eq!(asm_words("srai x1, x1, 3")[0], 0x4030_D093);
    }

    #[test]
    fn codifica_saltos_relativos() {
        // beq x1, x2, +8 (etiqueta dos instrucciones más adelante)
        let w = asm_words("beq x1, x2, fin\nnop\nfin: nop");
        assert_eq!(w[0], 0x0020_8463);
        // jal hacia atrás
        let w = asm_words("ini: nop\njal x0, ini");
        assert_eq!(w[1], 0xFFDF_F06F); // offset -4
    }

    #[test]
    fn nombres_abi_y_numericos_equivalen() {
        assert_eq!(asm_words("addi sp, sp, -16"), asm_words("addi x2, x2, -16"));
        assert_eq!(asm_words("mv t0, a7"), asm_words("mv x5, x17"));
        assert_eq!(asm_words("add fp, s0, x8"), asm_words("add x8, x8, x8"));
    }

    #[test]
    fn li_expande_segun_tamano() {
        assert_eq!(asm_words("li a0, 42"), vec![0x02A0_0513]); // addi a0,x0,42
        let w = asm_words("li a0, 0x12345678");
        assert_eq!(w.len(), 2); // lui + addi
                                // lui a0, 0x12345 ; addi a0, a0, 0x678
        assert_eq!(w[0], 0x1234_5537);
        assert_eq!(w[1], 0x6785_0513);
        // caso con compensación de signo en la parte baja
        let w = asm_words("li a0, 0x12345FFF");
        assert_eq!(w[0], 0x1234_6537); // hi20 redondea hacia arriba
        assert_eq!(w[1], 0xFFF5_0513); // addi a0, a0, -1
    }

    #[test]
    fn la_resuelve_direcciones_de_datos() {
        let p = assemble(".data\nmsj: .asciiz \"hola\"\n.text\nla a0, msj").unwrap();
        assert_eq!(p.symbols["msj"], DATA_BASE);
        let w: Vec<u32> = p
            .text
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(w[0], enc_u(hi20(DATA_BASE), 10, 0x37));
        assert_eq!(w[1], enc_i(lo12(DATA_BASE), 10, 0, 10, 0x13));
    }

    #[test]
    fn directivas_de_datos_y_alineacion() {
        let p = assemble(".data\nb: .byte 1\nw: .word 7, -1\n.text\nnop").unwrap();
        // .word alinea a 4: tras 1 byte hay 3 de relleno
        assert_eq!(p.symbols["w"], DATA_BASE + 4);
        assert_eq!(&p.data[4..8], &7u32.to_le_bytes());
        assert_eq!(&p.data[8..12], &(-1i32).to_le_bytes());
    }

    #[test]
    fn asciiz_agrega_nul_y_escapes() {
        let p = assemble(".data\ns: .asciiz \"a\\n\"\n.text\nnop").unwrap();
        assert_eq!(p.data, vec![b'a', b'\n', 0]);
    }

    #[test]
    fn word_con_etiqueta_se_resuelve() {
        let p = assemble(".data\nptr: .word msj\nmsj: .asciiz \"x\"\n.text\nnop").unwrap();
        assert_eq!(&p.data[0..4], &(DATA_BASE + 4).to_le_bytes());
    }

    #[test]
    fn source_map_mapea_lineas_y_expansiones() {
        let src = ".text\nli a0, 0x12345678\nnop";
        let p = assemble(src).unwrap();
        // li (línea 2) expande a 2 words: ambos apuntan a la línea 2
        assert_eq!(p.source_map.line_for_addr(TEXT_BASE), Some(2));
        assert_eq!(p.source_map.line_for_addr(TEXT_BASE + 4), Some(2));
        assert_eq!(p.source_map.line_for_addr(TEXT_BASE + 8), Some(3));
        assert_eq!(p.source_map.addr_for_line(2), Some(TEXT_BASE));
        assert_eq!(p.source_map.addr_for_line(3), Some(TEXT_BASE + 8));
    }

    #[test]
    fn errores_en_espanol_con_linea_y_columna() {
        let e = first_error("nop\nsumar x1, x2, x3");
        assert_eq!(e.line, 2);
        assert!(e.msg.contains("instrucción desconocida"), "{}", e.msg);

        let e = first_error("addi x1, x2");
        assert!(e.msg.contains("espera 3 operandos"), "{}", e.msg);

        let e = first_error("addi x1, x2, 5000");
        assert!(e.msg.contains("fuera de rango"), "{}", e.msg);

        let e = first_error("lw t0, 4(t9)");
        assert!(e.msg.contains("registro inválido"), "{}", e.msg);

        let e = first_error("j ninguna_parte");
        assert!(e.msg.contains("etiqueta no definida"), "{}", e.msg);

        let e = first_error("eti: nop\neti: nop");
        assert!(e.msg.contains("etiqueta duplicada"), "{}", e.msg);
        assert!(e.msg.contains("línea 1"), "{}", e.msg);
    }

    #[test]
    fn reporta_multiples_errores_ordenados() {
        let errs = assemble("bogus1\nnop\nbogus2 x1").unwrap_err();
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line, 1);
        assert_eq!(errs[1].line, 3);
    }

    #[test]
    fn programa_vacio_es_error_didactico() {
        let e = first_error("# solo un comentario\n");
        assert!(e.msg.contains("no tiene instrucciones"), "{}", e.msg);
    }

    #[test]
    fn pseudo_instrucciones_basicas() {
        assert_eq!(asm_words("nop")[0], 0x0000_0013);
        assert_eq!(asm_words("ret")[0], 0x0000_8067);
        assert_eq!(asm_words("mv a0, a1")[0], 0x0005_8513); // addi a0, a1, 0
        assert_eq!(asm_words("not a0, a1")[0], 0xFFF5_C513); // xori a0, a1, -1
        assert_eq!(asm_words("neg a0, a1")[0], 0x40B0_0533); // sub a0, x0, a1
        assert_eq!(asm_words("seqz a0, a1")[0], 0x0015_B513); // sltiu a0, a1, 1
                                                              // beqz / bnez
        let w = asm_words("beqz t0, fin\nfin: nop");
        assert_eq!(w[0], enc_b(4, 0, 5, 0));
    }

    #[test]
    fn instrucciones_en_data_y_datos_en_text_son_error() {
        let e = first_error(".data\nadd x1, x2, x3");
        assert!(e.msg.contains(".text"), "{}", e.msg);
        let e = first_error(".text\n.word 5");
        assert!(e.msg.contains(".data"), "{}", e.msg);
    }

    #[test]
    fn store_con_etiqueta_usa_registro_auxiliar() {
        let p = assemble(".data\nv: .word 0\n.text\nsw a0, v, t0").unwrap();
        let w: Vec<u32> = p
            .text
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(w[0], enc_u(hi20(DATA_BASE), 5, 0x37));
        assert_eq!(w[1], enc_s(lo12(DATA_BASE), 10, 5, 2));
    }

    #[test]
    fn comentarios_y_lineas_vacias_se_ignoran() {
        let p = assemble("# encabezado\n\n  nop # al final\n").unwrap();
        assert_eq!(p.text.len(), 4);
    }
}
