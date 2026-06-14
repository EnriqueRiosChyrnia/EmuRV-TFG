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
    /// Archivo de origen cuando el error está DENTRO de un include (RP5).
    /// `None` = archivo principal (el buffer del editor). El display solo
    /// antepone el nombre cuando es `Some`.
    pub file: Option<String>,
}

impl AsmError {
    pub(crate) fn new(line: u32, col: u32, msg: impl Into<String>) -> Self {
        Self {
            line,
            col,
            msg: msg.into(),
            file: None,
        }
    }
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{file}: ")?;
        }
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
    /// Procedencia REAL de cada palabra de .text: (dirección, índice de
    /// archivo, línea en ese archivo). La UI actual no lo consume — queda
    /// para la futura vista multi-archivo (RP5, diseño acordado).
    pub origins: Vec<(u32, u16, u32)>,
    /// Nombres de archivo de `origins`; `[0]` es el principal.
    pub files: Vec<String>,
    /// Punto de entrada real (RP11): el ensamblador siempre produce
    /// TEXT_BASE; un ELF importado puede traer otro e_entry dentro de .text.
    pub entry: u32,
    /// El programa vino de un binario importado (RP11): sin fuente ni
    /// source_map — la UI muestra el listado desensamblado.
    pub from_binary: bool,
    /// Ensamblado en modo REUBICABLE (RP29): las referencias a símbolos de
    /// las pseudo-instrucciones (`la`/`lla`, cargas/almacenamientos con
    /// etiqueta) se codifican PC-relativo (`auipc`+`%pcrel`) en vez de
    /// absoluto (`lui`+`%hi/%lo`). El `export_elf` lo marca ET_DYN. El modo
    /// por defecto (`false`) no cambia: sigue absoluto/ET_EXEC.
    pub reubicable: bool,
}

impl Program {
    /// Punto de entrada (convención RARS: comienzo de .text; un ELF
    /// importado puede sobreescribirlo — RP11).
    pub fn entry(&self) -> u32 {
        self.entry
    }
}

/// Lista CANÓNICA de mnemónicos que el ensamblador acepta (RP14): reales
/// (RV32I+M+Zicsr+F+D+A+C) y pseudo-instrucciones. Es la fuente de verdad
/// del anti-drift con el editor (`src/editor/isa.ts` debe igualarla — ver
/// tests/rp14_anti_drift.rs). DEBE mantenerse en sincronía con
/// `check_shape`/`parse_amo`/`es_comprimida`: el test
/// `cada_mnemonico_conocido_se_acepta` verifica que cada entrada de acá es
/// reconocida por el ensamblador (no «instrucción desconocida»).
///
/// NO incluye `fmv.x.d`/`fmv.d.x` (el ensamblador las rechaza: no existen
/// en RV32) ni los modos de redondeo (rne/rtz/…: son operandos, no
/// instrucciones). Las atómicas van en forma base; los sufijos
/// .aq/.rl/.aqrl los maneja el editor.
pub fn known_mnemonics() -> Vec<&'static str> {
    let mut v = vec![
        // RV32I
        "lui", "auipc", "jal", "jalr", "beq", "bne", "blt", "bge", "bltu", "bgeu", "lb", "lh",
        "lw", "lbu", "lhu", "sb", "sh", "sw", "addi", "slti", "sltiu", "xori", "ori", "andi",
        "slli", "srli", "srai", "add", "sub", "sll", "slt", "sltu", "xor", "srl", "sra", "or",
        "and", "fence", "ecall", "ebreak",
        // RV32M
        "mul", "mulh", "mulhsu", "mulhu", "div", "divu", "rem", "remu",
        // Zicsr (RP8)
        "csrrw", "csrrs", "csrrc", "csrrwi", "csrrsi", "csrrci", "mret",
        // RV32F (RP12)
        "flw", "fsw", "fadd.s", "fsub.s", "fmul.s", "fdiv.s", "fsqrt.s", "fmin.s", "fmax.s",
        "fmadd.s", "fmsub.s", "fnmadd.s", "fnmsub.s", "fcvt.w.s", "fcvt.wu.s", "fcvt.s.w",
        "fcvt.s.wu", "fmv.x.w", "fmv.w.x", "feq.s", "flt.s", "fle.s", "fsgnj.s", "fsgnjn.s",
        "fsgnjx.s", "fclass.s",
        // RV32D (RP12)
        "fld", "fsd", "fadd.d", "fsub.d", "fmul.d", "fdiv.d", "fsqrt.d", "fmin.d", "fmax.d",
        "fmadd.d", "fmsub.d", "fnmadd.d", "fnmsub.d", "fcvt.w.d", "fcvt.wu.d", "fcvt.d.w",
        "fcvt.d.wu", "fcvt.s.d", "fcvt.d.s", "feq.d", "flt.d", "fle.d", "fsgnj.d", "fsgnjn.d",
        "fsgnjx.d", "fclass.d",
        // RV32A (RP13) — forma base
        "lr.w", "sc.w", "amoswap.w", "amoadd.w", "amoand.w", "amoor.w", "amoxor.w", "amomin.w",
        "amomax.w", "amominu.w", "amomaxu.w",
        // RV32C (RP13)
        "c.nop", "c.addi", "c.li", "c.mv", "c.jr", "c.j", "c.lw", "c.sw", "c.beqz", "c.bnez",
        // pseudo-instrucciones
        "nop", "li", "la", "lla", "mv", "not", "neg", "seqz", "snez", "sltz", "sgtz", "beqz",
        "bnez", "blez", "bgez", "bltz", "bgtz", "bgt", "ble", "bgtu", "bleu", "j", "jr", "ret",
        "call", "tail", "csrr", "csrw", "csrs", "csrc", "fmv.s", "fmv.d", "fabs.s", "fabs.d",
        "fneg.s", "fneg.d",
    ];
    v.sort_unstable();
    v
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
    /// `%` de los operadores %hi/%lo (RP5).
    Percent,
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
                toks.push(STok {
                    tok: Tok::Percent,
                    col,
                });
                i += 1;
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

/// Traduce un nombre de registro FLOTANTE (ABI o fN) a su número (RP12).
/// Tabla propia: ft0-7=f0-7, fs0-1=f8-9, fa0-7=f10-17, fs2-11=f18-27,
/// ft8-11=f28-31 (convención de llamada estándar RV32).
pub fn freg_number(name: &str) -> Option<u8> {
    let n = match name {
        "ft0" => 0, "ft1" => 1, "ft2" => 2, "ft3" => 3,
        "ft4" => 4, "ft5" => 5, "ft6" => 6, "ft7" => 7,
        "fs0" => 8, "fs1" => 9,
        "fa0" => 10, "fa1" => 11, "fa2" => 12, "fa3" => 13,
        "fa4" => 14, "fa5" => 15, "fa6" => 16, "fa7" => 17,
        "fs2" => 18, "fs3" => 19, "fs4" => 20, "fs5" => 21,
        "fs6" => 22, "fs7" => 23, "fs8" => 24, "fs9" => 25,
        "fs10" => 26, "fs11" => 27,
        "ft8" => 28, "ft9" => 29, "ft10" => 30, "ft11" => 31,
        _ => {
            let rest = name.strip_prefix('f')?;
            let n: u8 = rest.parse().ok()?;
            if n > 31 {
                return None;
            }
            n
        }
    };
    Some(n)
}

/// CSRs con nombre soportados (RP8): número de la espec privilegiada.
/// Cualquier otro CSR puede darse por número crudo 0xNNN (solo estos ocho
/// están garantizados por el backend de Unicorn — ver docs/TRAPS.md).
pub fn csr_number(name: &str) -> Option<u32> {
    Some(match name {
        "mstatus" => 0x300,
        "mie" => 0x304,
        "mtvec" => 0x305,
        "mscratch" => 0x340,
        "mepc" => 0x341,
        "mcause" => 0x342,
        "mtval" => 0x343,
        "mip" => 0x344,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Parser de sentencias
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum OpKind {
    Reg(u8),
    /// Registro flotante f0-f31 / ft-fs-fa (RP12).
    FReg(u8),
    Imm(i64),
    Sym(String),
    Mem {
        off: i64,
        base: u8,
    },
    /// `%hi(x)` / `%lo(x)` como inmediato (lui/addi/ori). La resolución es
    /// en pasada 2 (las etiquetas pueden definirse después, como siempre).
    HiLo {
        hi: bool,
        arg: HiLoArg,
    },
    /// `%lo(x)(base)` como operando de memoria de cargas/almacenamientos.
    MemLo {
        arg: HiLoArg,
        base: u8,
    },
}

/// Argumento de %hi/%lo: etiqueta (se resuelve en pasada 2) o valor ya
/// conocido (número literal o constante .eqv sustituida en pasada 1).
#[derive(Debug, Clone, PartialEq)]
enum HiLoArg {
    Sym(String),
    Val(i64),
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
    /// Línea REAL en su archivo (errores de pasada 2).
    line: u32,
    /// Archivo de origen (índice en files; 0 = principal).
    file: u16,
    /// Línea del principal a la que se atribuye (source_map). Igual a
    /// `line` cuando no hay includes.
    main_line: u32,
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
    file: u16,
}

struct Pass1 {
    segment: Segment,
    text_off: u32,
    data: Vec<u8>,
    instrs: Vec<PInstr>,
    symbols: BTreeMap<String, u32>,
    sym_lines: HashMap<String, u32>,
    /// Constantes simbólicas `.eqv`/`.equ` (RP5): valor y línea de
    /// definición. Se sustituyen al parsear operandos (deben definirse
    /// ANTES de usarse, a diferencia de las etiquetas).
    consts: HashMap<String, (i64, u32)>,
    fixups: Vec<DataFixup>,
    errors: Vec<AsmError>,
    /// Procedencia de la línea EN CURSO (la setea el bucle principal antes
    /// de parsear cada PpLine; evita enhebrar 3 valores por cada firma).
    cur_file: u16,
    cur_main_line: u32,
}

/// Ensambla un fuente SIN resolución de archivos (`.include` da error
/// didáctico). Wrapper retrocompatible: los tests y llamadas existentes no
/// cambian. El nombre del "archivo" solo aparece en errores de includes.
pub fn assemble(source: &str) -> Result<Program, Vec<AsmError>> {
    assemble_with_includes(source, None, "programa.s")
}

/// Como [`assemble`] pero en modo REUBICABLE (RP29): las referencias a
/// símbolos se codifican PC-relativo. Atajo para tests/usos sin includes.
pub fn assemble_reubicable(source: &str) -> Result<Program, Vec<AsmError>> {
    assemble_impl(source, None, "programa.s", true)
}

/// Ensambla el fuente completo, resolviendo `.include` relativo a
/// `base_dir` (RP5). Devuelve el programa o TODOS los errores encontrados
/// (no solo el primero), ordenados por archivo y línea.
pub fn assemble_with_includes(
    source: &str,
    base_dir: Option<&std::path::Path>,
    main_name: &str,
) -> Result<Program, Vec<AsmError>> {
    assemble_impl(source, base_dir, main_name, false)
}

/// Igual que [`assemble_with_includes`] pero en modo REUBICABLE (RP29).
/// Lo usa la CLI con `--reubicable`; la app (modo normal) NO lo llama.
pub fn assemble_with_includes_reubicable(
    source: &str,
    base_dir: Option<&std::path::Path>,
    main_name: &str,
) -> Result<Program, Vec<AsmError>> {
    assemble_impl(source, base_dir, main_name, true)
}

/// Cuerpo común del ensamblado. `reubicable` solo afecta la pasada 2: la
/// expansión de las pseudo-instrucciones con etiqueta usa `auipc`+`%pcrel`
/// en vez de `lui`+`%hi/%lo` (las direcciones y el `source_map` no cambian).
fn assemble_impl(
    source: &str,
    base_dir: Option<&std::path::Path>,
    main_name: &str,
    reubicable: bool,
) -> Result<Program, Vec<AsmError>> {
    let pp = super::preprocess::preprocess(source, base_dir, main_name)?;
    let files = pp.files;
    // Adjunta el nombre de archivo a un error nacido en un include (los
    // del principal quedan con file=None: el formato actual no cambia).
    let con_archivo = |mut e: AsmError, file: u16| -> AsmError {
        if file != 0 && e.file.is_none() {
            e.file = Some(files[file as usize].clone());
        }
        e
    };

    let mut p = Pass1 {
        segment: Segment::Text,
        text_off: 0,
        data: Vec::new(),
        instrs: Vec::new(),
        symbols: BTreeMap::new(),
        sym_lines: HashMap::new(),
        consts: HashMap::new(),
        fixups: Vec::new(),
        errors: Vec::new(),
        cur_file: 0,
        cur_main_line: 0,
    };

    for pl in &pp.lines {
        p.cur_file = pl.file;
        p.cur_main_line = pl.main_line;
        let toks = match lex_line(&pl.text, pl.line) {
            Ok(t) => t,
            Err(e) => {
                let e = con_archivo(e, pl.file);
                p.errors.push(e);
                continue;
            }
        };
        if let Err(e) = parse_line(&mut p, &toks, pl.line) {
            let e = con_archivo(e, pl.file);
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

    // Pasada 2: codificación y resolución de símbolos. El source_map se
    // construye con `main_line` (atribución al archivo principal: regla
    // RP5); los errores usan `line`/`file` reales.
    let mut text = Vec::with_capacity((p.text_off as usize).max(4));
    let mut source_map = SourceMap::default();
    let mut origins = Vec::with_capacity(p.instrs.len());
    for ins in &p.instrs {
        // RP13: las comprimidas (C) emiten 2 bytes. El resto, una o dos
        // palabras de 4 (li/la/loads con etiqueta). El source_map se llena
        // con la dirección REAL de cada media/palabra — así breakpoints,
        // resaltado y step quedan exactos con tamaños mixtos.
        if ins.size == 2 {
            match encode_compressed(ins, &p.symbols) {
                Ok(half) => {
                    source_map.addr_to_line.insert(ins.addr, ins.main_line);
                    origins.push((ins.addr, ins.file, ins.line));
                    text.extend_from_slice(&half.to_le_bytes());
                    source_map
                        .line_to_addr
                        .entry(ins.main_line)
                        .or_insert(ins.addr);
                }
                Err(e) => p.errors.push(con_archivo(e, ins.file)),
            }
            continue;
        }
        match encode_instr(ins, &p.symbols, reubicable) {
            Ok(words) => {
                debug_assert_eq!(words.len() as u32 * 4, ins.size);
                for (k, w) in words.iter().enumerate() {
                    let addr = ins.addr + (k as u32) * 4;
                    source_map.addr_to_line.insert(addr, ins.main_line);
                    origins.push((addr, ins.file, ins.line));
                    text.extend_from_slice(&w.to_le_bytes());
                }
                source_map
                    .line_to_addr
                    .entry(ins.main_line)
                    .or_insert(ins.addr);
            }
            Err(e) => {
                let e = con_archivo(e, ins.file);
                p.errors.push(e);
            }
        }
    }

    // Fixups de datos (.word etiqueta).
    for f in &p.fixups {
        match p.symbols.get(&f.sym) {
            Some(&v) => p.data[f.offset..f.offset + 4].copy_from_slice(&v.to_le_bytes()),
            None => {
                let e = AsmError::new(f.line, f.col, format!("etiqueta no definida: «{}»", f.sym));
                let e = con_archivo(e, f.file);
                p.errors.push(e);
            }
        }
    }

    if p.errors.is_empty() {
        Ok(Program {
            text,
            data: p.data,
            symbols: p.symbols,
            source_map,
            origins,
            files,
            entry: TEXT_BASE,
            from_binary: false,
            reubicable,
        })
    } else {
        p.errors.sort_by_key(|e| (e.file.clone(), e.line, e.col));
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
    if let Some((_, prev)) = p.consts.get(name) {
        return Err(AsmError::new(
            line,
            col,
            format!("«{name}» ya es una constante .eqv (definida en la línea {prev}); no puede ser etiqueta"),
        ));
    }
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
                    }] if p.consts.contains_key(sym) => {
                        // Constante .eqv: vale en .word/.half/.byte.
                        let v = p.consts[sym].0;
                        if v < min || v > max {
                            return Err(AsmError::new(
                                line,
                                *scol,
                                format!("el valor {v} de «{sym}» no cabe en {name} ({min}..{max})"),
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
                            file: p.cur_file,
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
        ".eqv" | ".equ" => {
            // Constante simbólica (RP5): `.eqv NOMBRE valor` (RARS) y
            // `.equ NOMBRE, valor` (GNU). Se aceptan ambas grafías con o
            // sin coma. Debe definirse antes de usarse; redefinir es error.
            define_labels(p, labels, line)?;
            let mut it = rest;
            let (cname, ncol) = match it.first() {
                Some(STok {
                    tok: Tok::Ident(n),
                    col,
                }) => (n.clone(), *col),
                _ => {
                    return Err(AsmError::new(
                        line,
                        col,
                        format!("{name} espera un nombre y un valor (ej.: {name} LIMITE, 10)"),
                    ))
                }
            };
            it = &it[1..];
            if matches!(
                it.first(),
                Some(STok {
                    tok: Tok::Comma,
                    ..
                })
            ) {
                it = &it[1..];
            }
            let value = match it {
                [STok { tok: Tok::Num(v), .. }] => *v,
                [STok { tok: Tok::Ident(other), col: ocol }] => match p.consts.get(other) {
                    // Encadenado: una constante puede definirse desde otra.
                    Some((v, _)) => *v,
                    None => {
                        return Err(AsmError::new(
                            line,
                            *ocol,
                            format!("«{other}» no es una constante conocida (las constantes deben definirse antes de usarse)"),
                        ))
                    }
                },
                _ => {
                    return Err(AsmError::new(
                        line,
                        col,
                        format!("{name} espera un valor numérico o una constante ya definida"),
                    ))
                }
            };
            if reg_number(&cname).is_some() {
                return Err(AsmError::new(
                    line,
                    ncol,
                    format!("«{cname}» es un nombre de registro: no puede usarse como constante"),
                ));
            }
            if let Some(prev) = p.sym_lines.get(&cname) {
                return Err(AsmError::new(
                    line,
                    ncol,
                    format!("«{cname}» ya es una etiqueta (línea {prev}); no puede ser constante"),
                ));
            }
            if let Some((_, prev)) = p.consts.get(&cname) {
                return Err(AsmError::new(
                    line,
                    ncol,
                    format!("constante redefinida: «{cname}» (ya definida en la línea {prev})"),
                ));
            }
            p.consts.insert(cname, (value, line));
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
                 .ascii .asciiz .string .space .align .globl .eqv .equ"
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
        ops.push(parse_operand(arg, line, &p.consts)?);
    }
    let size = instr_size(&mnem, &ops, line, col)?;
    let addr = TEXT_BASE + p.text_off;
    p.text_off += size;
    p.instrs.push(PInstr {
        mnem,
        ops,
        line,
        file: p.cur_file,
        main_line: p.cur_main_line,
        addr,
        size,
    });
    Ok(())
}

fn parse_operand(
    toks: &[STok],
    line: u32,
    consts: &HashMap<String, (i64, u32)>,
) -> Result<POp, AsmError> {
    // Resuelve el argumento de %hi/%lo: número, constante .eqv o etiqueta.
    let hilo_arg = |name_or_num: &Tok| -> Option<HiLoArg> {
        match name_or_num {
            Tok::Num(v) => Some(HiLoArg::Val(*v)),
            Tok::Ident(n) => Some(match consts.get(n) {
                Some((v, _)) => HiLoArg::Val(*v),
                None => HiLoArg::Sym(n.clone()),
            }),
            _ => None,
        }
    };
    match toks {
        // %hi(x) / %lo(x) — inmediato para lui/addi/ori (RP5).
        [STok {
            tok: Tok::Percent,
            col,
        }, STok {
            tok: Tok::Ident(f),
            col: fcol,
        }, STok {
            tok: Tok::LParen, ..
        }, arg, STok {
            tok: Tok::RParen, ..
        }] => {
            let hi = match f.as_str() {
                "hi" => true,
                "lo" => false,
                otro => {
                    return Err(AsmError::new(
                        line,
                        *fcol,
                        format!("tras «%» se esperaba hi o lo, no «{otro}»"),
                    ))
                }
            };
            let arg = hilo_arg(&arg.tok).ok_or_else(|| {
                AsmError::new(
                    line,
                    arg.col,
                    "%hi/%lo esperan una etiqueta, número o constante entre paréntesis",
                )
            })?;
            Ok(POp {
                kind: OpKind::HiLo { hi, arg },
                col: *col,
            })
        }
        // %lo(x)(base) — desplazamiento de carga/almacenamiento (RP5).
        [STok {
            tok: Tok::Percent,
            col,
        }, STok {
            tok: Tok::Ident(f),
            col: fcol,
        }, STok {
            tok: Tok::LParen, ..
        }, arg, STok {
            tok: Tok::RParen, ..
        }, STok {
            tok: Tok::LParen, ..
        }, STok {
            tok: Tok::Ident(base),
            col: bcol,
        }, STok {
            tok: Tok::RParen, ..
        }] => {
            if f != "lo" {
                return Err(AsmError::new(
                    line,
                    *fcol,
                    "como desplazamiento de memoria solo vale %lo(...) (la parte alta va en el lui previo)",
                ));
            }
            let arg = hilo_arg(&arg.tok).ok_or_else(|| {
                AsmError::new(
                    line,
                    arg.col,
                    "%lo espera una etiqueta, número o constante entre paréntesis",
                )
            })?;
            let base = reg_number(base).ok_or_else(|| {
                AsmError::new(line, *bcol, format!("registro inválido: «{base}»"))
            })?;
            Ok(POp {
                kind: OpKind::MemLo { arg, base },
                col: *col,
            })
        }
        [STok {
            tok: Tok::Percent,
            col,
        }, ..] => Err(AsmError::new(
            line,
            *col,
            "uso de «%» mal formado: la sintaxis es %hi(simbolo) o %lo(simbolo)",
        )),
        [STok {
            tok: Tok::Ident(name),
            col,
        }] => match reg_number(name) {
            Some(r) => Ok(POp {
                kind: OpKind::Reg(r),
                col: *col,
            }),
            // Registro flotante (RP12): ft/fs/fa/fN. Va DESPUÉS de los
            // enteros (no hay colisión de nombres) y ANTES de las
            // constantes (un .eqv llamado «fa0» sería una trampa).
            None if freg_number(name).is_some() => Ok(POp {
                kind: OpKind::FReg(freg_number(name).unwrap()),
                col: *col,
            }),
            // Constante .eqv: se sustituye acá, ANTES de decidir tamaño en
            // pasada 1 — por eso deben definirse antes de usarse.
            None => match consts.get(name) {
                Some((v, _)) => Ok(POp {
                    kind: OpKind::Imm(*v),
                    col: *col,
                }),
                None => Ok(POp {
                    kind: OpKind::Sym(name.clone()),
                    col: *col,
                }),
            },
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
// Extensión A (atómicas, RP13)
// ---------------------------------------------------------------------------

/// ¿`mnem` es una instrucción comprimida (extensión C, RP13)? Conjunto
/// didáctico: las que un alumno escribiría a mano para ver compresión.
fn es_comprimida(mnem: &str) -> bool {
    matches!(
        mnem,
        "c.nop" | "c.addi" | "c.li" | "c.mv" | "c.jr" | "c.j" | "c.lw" | "c.sw"
            | "c.beqz" | "c.bnez"
    )
}

/// Registro comprimido (x8–x15): los formatos CL/CS/CB usan 3 bits.
/// Devuelve el código de 3 bits o un error didáctico si el registro no
/// está en el rango permitido por C.
fn creg(r: u8) -> Option<u32> {
    if (8..=15).contains(&r) {
        Some(u32::from(r) - 8)
    } else {
        None
    }
}

/// Si `mnem` es una instrucción atómica RV32A, devuelve
/// (funct5, aq, rl, es_lr, es_sc). El sufijo opcional .aq/.rl/.aqrl define
/// los bits de ordenamiento (sin sufijo = ambos 0). Todas son `.w` (RV32).
fn parse_amo(mnem: &str) -> Option<(u32, bool, bool, bool, bool)> {
    let resto = mnem.strip_suffix(".aqrl").map(|b| (b, true, true))
        .or_else(|| mnem.strip_suffix(".aq").map(|b| (b, true, false)))
        .or_else(|| mnem.strip_suffix(".rl").map(|b| (b, false, true)))
        .unwrap_or((mnem, false, false));
    let (base, aq, rl) = resto;
    let base = base.strip_suffix(".w")?;
    let funct5 = match base {
        "amoadd" => 0x00,
        "amoswap" => 0x01,
        "lr" => 0x02,
        "sc" => 0x03,
        "amoxor" => 0x04,
        "amoor" => 0x08,
        "amoand" => 0x0C,
        "amomin" => 0x10,
        "amomax" => 0x14,
        "amominu" => 0x18,
        "amomaxu" => 0x1C,
        _ => return None,
    };
    Some((funct5, aq, rl, base == "lr", base == "sc"))
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
            OpKind::Sym(_) | OpKind::Imm(_) => 8,
            _ => 4,
        },
        "sb" | "sh" | "sw" => match &ops[1].kind {
            OpKind::Sym(_) | OpKind::Imm(_) => 8,
            _ => 4,
        },
        // C (RP13): toda instrucción comprimida ocupa 2 bytes. El ensamblador
        // NO comprime automáticamente — solo emite C si el usuario la escribe.
        m if es_comprimida(m) => 2,
        // F/D (RP12): con etiqueta expanden a lui(aux)+acceso, como sw.
        "flw" | "fld" | "fsw" | "fsd" => match &ops[1].kind {
            OpKind::Sym(_) | OpKind::Imm(_) => 8,
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
    let is_mem = |i: usize| matches!(ops[i].kind, Mem { .. } | MemLo { .. });
    // %hi/%lo valen como inmediato SOLO en lui, addi y ori (semántica
    // estándar del par lui+addi/ori; en otras posiciones sería un error
    // semántico silencioso — p. ej. %hi con auipc es %pcrel_hi, que no
    // soportamos).
    let is_hilo = |i: usize| matches!(ops[i].kind, HiLo { .. });
    // RP13: operando de memoria atómica «(rs1)» = Mem con offset 0.
    let is_paren = |i: usize| matches!(ops[i].kind, Mem { off: 0, .. });
    // RP12: registros flotantes y sus errores didácticos.
    let is_freg = |i: usize| matches!(ops[i].kind, FReg(_));
    // RP13: comprimidas (C). Cada forma valida sus operandos; los registros
    // de los formatos CL/CS/CB deben estar en x8–x15.
    if es_comprimida(mnem) {
        let creg_ok = |i: usize| matches!(ops[i].kind, Reg(r) if (8..=15).contains(&r));
        match mnem {
            "c.nop" => want(0, "sin operandos")?,
            "c.addi" | "c.li" => {
                want(2, "registro, inmediato")?;
                if !(is_reg(0) && is_imm(1)) {
                    return err(format!("«{mnem}» espera registro e inmediato (ej.: {mnem} a0, 5)"));
                }
            }
            "c.mv" => {
                want(2, "registro, registro")?;
                if !(is_reg(0) && is_reg(1)) {
                    return err("«c.mv» espera dos registros (ej.: c.mv a1, a0)".into());
                }
            }
            "c.jr" => {
                want(1, "registro")?;
                if !is_reg(0) {
                    return err("«c.jr» espera un registro (ej.: c.jr ra)".into());
                }
            }
            "c.j" => {
                want(1, "etiqueta")?;
                if !(is_sym(0) || is_imm(0)) {
                    return err("«c.j» espera una etiqueta (ej.: c.j fin)".into());
                }
            }
            "c.lw" | "c.sw" => {
                want(2, "registro, desplazamiento(base)")?;
                if !(creg_ok(0) && is_mem(1)) {
                    return err(format!(
                        "«{mnem}» espera registro x8–x15 y desplazamiento(base) con base                          x8–x15 (ej.: {mnem} a0, 4(a1)) — la extensión C solo alcanza                          esos registros"
                    ));
                }
            }
            "c.beqz" | "c.bnez" => {
                want(2, "registro, etiqueta")?;
                if !(creg_ok(0) && (is_sym(1) || is_imm(1))) {
                    return err(format!(
                        "«{mnem}» espera registro x8–x15 y etiqueta (ej.: {mnem} a0, fin)"
                    ));
                }
            }
            _ => {}
        }
        return Ok(());
    }
    // RP13: atómicas — lr.w rd, (rs1) ; resto rd, rs2, (rs1).
    if let Some((_, _, _, es_lr, _)) = parse_amo(mnem) {
        if es_lr {
            want(2, "registro, (registro)")?;
            if !(is_reg(0) && is_paren(1)) {
                return err(format!(
                    "«{mnem}» espera registro y dirección entre paréntesis                      (ej.: {mnem} t0, (t1))"
                ));
            }
        } else {
            want(3, "registro, registro, (registro)")?;
            if !(is_reg(0) && is_reg(1) && is_paren(2)) {
                return err(format!(
                    "«{mnem}» espera rd, rs2 y dirección entre paréntesis                      (ej.: {mnem} t0, t1, (t2)) — la dirección NO lleva                      desplazamiento"
                ));
            }
        }
        return Ok(());
    }
    let pista_f = |hasta: usize| -> &'static str {
        if (0..hasta.min(n)).any(|i| matches!(ops[i].kind, Reg(_))) {
            " — necesita registros f (ft0…ft11, fs0…fs11, fa0…fa7) y usaste \
             un registro entero"
        } else {
            ""
        }
    };
    // Sufijo/operando de modo de redondeo (rtz, rne…): no soportado en v1.
    let rm_err = |esperados: usize| -> Result<(), AsmError> {
        if n == esperados + 1 {
            if let Sym(nombre) = &ops[esperados].kind {
                if matches!(nombre.as_str(), "rne" | "rtz" | "rdn" | "rup" | "rmm" | "dyn") {
                    return Err(AsmError::new(
                        line,
                        ops[esperados].col,
                        format!(
                            "modo de redondeo explícito («{nombre}») todavía no soportado: \
                             EmuRV usa siempre dyn (el modo del fcsr, RNE por defecto)"
                        ),
                    ));
                }
            }
        }
        Ok(())
    };

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
        // I aritmética: rd, rs1, imm — addi/ori aceptan además %hi/%lo (RP5).
        "addi" | "ori" => {
            want(3, "registro, registro, inmediato")?;
            if !(is_reg(0) && is_reg(1) && (is_imm(2) || is_hilo(2))) {
                return err(format!(
                    "«{mnem}» espera registro, registro e inmediato (ej.: addi t0, t1, 10)"
                ));
            }
        }
        "slti" | "sltiu" | "xori" | "andi" | "slli" | "srli" | "srai" => {
            want(3, "registro, registro, inmediato")?;
            if !(is_reg(0) && is_reg(1) && is_imm(2)) {
                return err(format!(
                    "«{mnem}» espera registro, registro e inmediato (ej.: addi t0, t1, 10)"
                ));
            }
        }
        // cargas: rd, off(base) | rd, etiqueta | rd, constante (dirección)
        "lb" | "lh" | "lw" | "lbu" | "lhu" => {
            want(2, "registro, desplazamiento(base) o etiqueta")?;
            if !is_reg(0) || !(is_mem(1) || is_sym(1) || is_imm(1)) {
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
                if !is_reg(0) || !(is_sym(1) || is_imm(1)) || !is_reg(2) {
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
        "lui" => {
            want(2, "registro, inmediato")?;
            if !(is_reg(0) && (is_imm(1) || is_hilo(1))) {
                return err(
                    "«lui» espera registro e inmediato de 20 bits (o %hi(etiqueta))".into(),
                );
            }
        }
        "auipc" => {
            want(2, "registro, inmediato")?;
            if !(is_reg(0) && is_imm(1)) {
                if is_hilo(1) {
                    return err(
                        "%hi con «auipc» sería %pcrel_hi (relativo al pc), que EmuRV no soporta; \
                         %hi(etiqueta) va con «lui»"
                            .into(),
                    );
                }
                return err("«auipc» espera registro e inmediato de 20 bits".into());
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
            if !(is_reg(0) && (is_sym(1) || is_imm(1))) {
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
        "nop" | "ret" | "ecall" | "ebreak" | "fence" | "mret" => {
            want(0, "sin operandos")?;
        }
        // Zicsr (RP8): rd, csr, rs1 / rd, csr, uimm5. El CSR es un nombre
        // conocido (Sym) o un número crudo (Imm, las constantes .eqv ya
        // llegaron sustituidas).
        // ---- RV32F/D (RP12) ----
        // R fp: fd, fs1, fs2
        "fadd.s" | "fsub.s" | "fmul.s" | "fdiv.s" | "fmin.s" | "fmax.s" | "fsgnj.s"
        | "fsgnjn.s" | "fsgnjx.s" | "fadd.d" | "fsub.d" | "fmul.d" | "fdiv.d" | "fmin.d"
        | "fmax.d" | "fsgnj.d" | "fsgnjn.d" | "fsgnjx.d" => {
            rm_err(3)?;
            want(3, "registro f, registro f, registro f")?;
            if !(is_freg(0) && is_freg(1) && is_freg(2)) {
                return err(format!(
                    "«{mnem}» espera tres registros f (ej.: {mnem} ft0, ft1, ft2){}",
                    pista_f(3)
                ));
            }
        }
        // fd, fs (sqrt, conversiones entre formatos, pseudos sgnj)
        "fsqrt.s" | "fsqrt.d" | "fcvt.s.d" | "fcvt.d.s" | "fmv.s" | "fmv.d" | "fabs.s"
        | "fabs.d" | "fneg.s" | "fneg.d" => {
            rm_err(2)?;
            want(2, "registro f, registro f")?;
            if !(is_freg(0) && is_freg(1)) {
                return err(format!(
                    "«{mnem}» espera dos registros f (ej.: {mnem} ft0, ft1){}",
                    pista_f(2)
                ));
            }
        }
        // R4: fd, fs1, fs2, fs3
        "fmadd.s" | "fmsub.s" | "fnmadd.s" | "fnmsub.s" | "fmadd.d" | "fmsub.d"
        | "fnmadd.d" | "fnmsub.d" => {
            rm_err(4)?;
            want(4, "registro f × 4")?;
            if !(is_freg(0) && is_freg(1) && is_freg(2) && is_freg(3)) {
                return err(format!(
                    "«{mnem}» espera cuatro registros f (ej.: {mnem} ft0, ft1, ft2, ft3){}",
                    pista_f(4)
                ));
            }
        }
        // entero ← flotante: rd, fs
        "fcvt.w.s" | "fcvt.wu.s" | "fcvt.w.d" | "fcvt.wu.d" | "fmv.x.w" | "fclass.s"
        | "fclass.d" => {
            rm_err(2)?;
            want(2, "registro entero, registro f")?;
            if !(is_reg(0) && is_freg(1)) {
                return err(format!(
                    "«{mnem}» espera destino ENTERO y origen f (ej.: {mnem} t0, ft0)"
                ));
            }
        }
        // flotante ← entero: fd, rs
        "fcvt.s.w" | "fcvt.s.wu" | "fcvt.d.w" | "fcvt.d.wu" | "fmv.w.x" => {
            rm_err(2)?;
            want(2, "registro f, registro entero")?;
            if !(is_freg(0) && is_reg(1)) {
                return err(format!(
                    "«{mnem}» espera destino f y origen ENTERO (ej.: {mnem} ft0, t0)"
                ));
            }
        }
        // comparaciones: rd entero, fs1, fs2
        "feq.s" | "flt.s" | "fle.s" | "feq.d" | "flt.d" | "fle.d" => {
            want(3, "registro entero, registro f, registro f")?;
            if !(is_reg(0) && is_freg(1) && is_freg(2)) {
                return err(format!(
                    "«{mnem}» espera destino ENTERO (0/1) y dos registros f \
                     (ej.: {mnem} t0, ft0, ft1)"
                ));
            }
        }
        // cargas f: fd, off(base) | fd, etiqueta, reg_aux (el aux es ENTERO:
        // el destino f no puede hacer de puntero como en lw)
        "flw" | "fld" => {
            if n == 2 {
                if !is_freg(0) || !is_mem(1) {
                    return err(format!(
                        "«{mnem}» espera registro f y desplazamiento(base) \
                         (ej.: {mnem} ft0, 0(t0)){}",
                        pista_f(1)
                    ));
                }
            } else if n == 3 {
                if !is_freg(0) || !(is_sym(1) || is_imm(1)) || !is_reg(2) {
                    return err(format!(
                        "«{mnem}» con etiqueta espera registro f, etiqueta y registro \
                         ENTERO auxiliar (ej.: {mnem} ft0, dato, t0)"
                    ));
                }
            } else {
                return err(format!(
                    "«{mnem}» espera 2 operandos ({mnem} ft0, 0(t0)) o 3 con etiqueta \
                     ({mnem} ft0, dato, t0); hay {n}"
                ));
            }
        }
        "fsw" | "fsd" => {
            if n == 2 {
                if !is_freg(0) || !is_mem(1) {
                    return err(format!(
                        "«{mnem}» espera registro f y desplazamiento(base) \
                         (ej.: {mnem} ft0, 0(t0)){}",
                        pista_f(1)
                    ));
                }
            } else if n == 3 {
                if !is_freg(0) || !(is_sym(1) || is_imm(1)) || !is_reg(2) {
                    return err(format!(
                        "«{mnem}» con etiqueta espera registro f, etiqueta y registro \
                         ENTERO auxiliar (ej.: {mnem} ft0, dato, t0)"
                    ));
                }
            } else {
                return err(format!(
                    "«{mnem}» espera 2 operandos ({mnem} ft0, 0(t0)) o 3 con etiqueta \
                     ({mnem} ft0, dato, t0); hay {n}"
                ));
            }
        }
        // los fmv de 64 bits no existen en RV32 (la espec los define solo
        // en RV64): el camino es por memoria.
        "fmv.x.d" | "fmv.d.x" => {
            return err(format!(
                "«{mnem}» no existe en RV32 (un f64 no entra en un registro x de \
                 32 bits): pasá el valor por memoria con fsd + lw/lw (o lw/lw + fld)"
            ));
        }
        "csrrw" | "csrrs" | "csrrc" => {
            want(3, "registro, csr, registro")?;
            if !(is_reg(0) && (is_sym(1) || is_imm(1)) && is_reg(2)) {
                return err(format!(
                    "«{mnem}» espera registro, csr y registro (ej.: {mnem} t0, mscratch, t1)"
                ));
            }
        }
        "csrrwi" | "csrrsi" | "csrrci" => {
            want(3, "registro, csr, inmediato de 5 bits")?;
            if !(is_reg(0) && (is_sym(1) || is_imm(1)) && is_imm(2)) {
                return err(format!(
                    "«{mnem}» espera registro, csr e inmediato 0-31 (ej.: {mnem} t0, mstatus, 8)"
                ));
            }
        }
        // Pseudo Zicsr (RARS/GNU): csrr rd, csr · csrw/csrs/csrc csr, rs.
        "csrr" => {
            want(2, "registro, csr")?;
            if !(is_reg(0) && (is_sym(1) || is_imm(1))) {
                return err("«csrr» espera registro y csr (ej.: csrr t0, mcause)".into());
            }
        }
        "csrw" | "csrs" | "csrc" => {
            want(2, "csr, registro")?;
            if !((is_sym(0) || is_imm(0)) && is_reg(1)) {
                return err(format!(
                    "«{mnem}» espera csr y registro (ej.: {mnem} mtvec, t0)"
                ));
            }
        }
        other => {
            return err(format!(
                "instrucción desconocida: «{other}». EmuRV soporta RV32IM \
                 (enteros y multiplicación/división de 32 bits) y Zicsr (csrr/csrw/…)"
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
/// R-type atómica (RP13): opcode AMO 0x2F, funct3=010 (.w).
fn enc_amo(funct5: u32, aq: bool, rl: bool, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct5 << 27)
        | ((aq as u32) << 26)
        | ((rl as u32) << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (2 << 12)
        | ((rd as u32) << 7)
        | 0x2F
}

/// S-type sobre STORE-FP (0x27): fsw/fsd (RP12).
fn enc_s_fp(imm: i32, rs2: u8, rs1: u8, f3: u32) -> u32 {
    let imm = imm as u32 & 0xFFF;
    ((imm >> 5) << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (f3 << 12)
        | ((imm & 0x1F) << 7)
        | 0x27
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
/// Zicsr: formato I sobre SYSTEM (0x73); rs1field lleva rs1 o el uimm5.
fn enc_csr(csr: u32, rs1field: u32, f3: u32, rd: u8) -> u32 {
    (csr << 20) | (rs1field << 15) | (f3 << 12) | ((rd as u32) << 7) | 0x73
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
    /// Modo reubicable (RP29): la expansión con etiqueta usa PC-relativo.
    reubicable: bool,
}

impl Ctx<'_> {
    /// Error interno: la pasada 2 encontró una forma que la pasada 1 no
    /// debió dejar pasar. Antes era un `unreachable!` que tiraba la app.
    fn internal_err(&self, i: usize, expected: &str) -> AsmError {
        let col = self.ins.ops.get(i).map_or(1, |o| o.col);
        AsmError::new(
            self.ins.line,
            col,
            format!(
                "error interno del ensamblador: se esperaba {expected} ya validado \
                 en la pasada 1 (reportá este bug)"
            ),
        )
    }
    fn reg(&self, i: usize) -> Result<u8, AsmError> {
        match self.ins.ops.get(i).map(|o| &o.kind) {
            Some(OpKind::Reg(r)) => Ok(*r),
            _ => Err(self.internal_err(i, "un registro")),
        }
    }
    fn freg(&self, i: usize) -> Result<u8, AsmError> {
        match self.ins.ops.get(i).map(|o| &o.kind) {
            Some(OpKind::FReg(r)) => Ok(*r),
            _ => Err(self.internal_err(i, "un registro flotante")),
        }
    }

    fn imm(&self, i: usize) -> Result<i64, AsmError> {
        match self.ins.ops.get(i).map(|o| &o.kind) {
            Some(OpKind::Imm(v)) => Ok(*v),
            // %hi → 20 bits altos con ajuste de signo; %lo → 12 bajos con
            // signo. lui+addi con %hi/%lo reconstruye el valor EXACTO
            // (misma matemática que la expansión de «la»: hi20/lo12).
            Some(OpKind::HiLo { hi, arg }) => {
                let v = self.resolve_hilo(i, arg)?;
                Ok(if *hi { hi20(v) as i64 } else { lo12(v) as i64 })
            }
            _ => Err(self.internal_err(i, "un inmediato")),
        }
    }
    fn mem(&self, i: usize) -> Result<(i64, u8), AsmError> {
        match self.ins.ops.get(i).map(|o| &o.kind) {
            Some(OpKind::Mem { off, base }) => Ok((*off, *base)),
            Some(OpKind::MemLo { arg, base }) => {
                let v = self.resolve_hilo(i, arg)?;
                Ok((lo12(v) as i64, *base))
            }
            _ => Err(self.internal_err(i, "un operando de memoria")),
        }
    }
    /// Valor de 32 bits del argumento de %hi/%lo (etiqueta, número o
    /// constante ya sustituida en pasada 1).
    fn resolve_hilo(&self, i: usize, arg: &HiLoArg) -> Result<u32, AsmError> {
        match arg {
            HiLoArg::Val(v) => Ok(*v as u32),
            HiLoArg::Sym(sym) => self.symbols.get(sym).copied().ok_or_else(|| {
                AsmError::new(
                    self.ins.line,
                    self.ins.ops[i].col,
                    format!("etiqueta no definida: «{sym}»"),
                )
            }),
        }
    }
    /// Offset PC-relativo de `addr` respecto del `auipc` que abre la
    /// expansión (la primera palabra de la pseudo, en `ins.addr`): es el
    /// valor de `%pcrel_hi`/`%pcrel_lo` del estándar RISC-V. `hi20`/`lo12`
    /// lo parten igual que en el par absoluto, así que reconstruyen `addr`
    /// exacto (verificado contra GNU en tests/spike_reubicable.rs).
    fn pcrel(&self, addr: u32) -> u32 {
        addr.wrapping_sub(self.ins.addr)
    }
    /// Resuelve una etiqueta (u offset numérico absoluto) a dirección.
    fn target(&self, i: usize) -> Result<u32, AsmError> {
        match self.ins.ops.get(i).map(|o| &o.kind) {
            Some(OpKind::Sym(s)) => self.symbols.get(s).copied().ok_or_else(|| {
                AsmError::new(
                    self.ins.line,
                    self.ins.ops[i].col,
                    format!("etiqueta no definida: «{s}»"),
                )
            }),
            Some(OpKind::Imm(v)) => Ok(*v as u32),
            _ => Err(self.internal_err(i, "una etiqueta o un offset")),
        }
    }
    /// Número de CSR del operando `i`: nombre conocido o número crudo
    /// 0..=0xFFF, con error didáctico que sugiere los soportados.
    fn csr(&self, i: usize) -> Result<u32, AsmError> {
        match self.ins.ops.get(i).map(|o| &o.kind) {
            Some(OpKind::Sym(nombre)) => super::assembler::csr_number(nombre).ok_or_else(|| {
                AsmError::new(
                    self.ins.line,
                    self.ins.ops[i].col,
                    format!(
                        "CSR desconocido: «{nombre}». Soportados por nombre: mstatus, mie, \
                         mtvec, mscratch, mepc, mcause, mtval, mip (o un número 0x000-0xFFF)"
                    ),
                )
            }),
            Some(OpKind::Imm(v)) if (0..=0xFFF).contains(v) => Ok(*v as u32),
            Some(OpKind::Imm(v)) => Err(AsmError::new(
                self.ins.line,
                self.ins.ops[i].col,
                format!("número de CSR fuera de rango: {v} (valen 0x000 a 0xFFF)"),
            )),
            _ => Err(self.internal_err(i, "un CSR (nombre o número)")),
        }
    }
    /// Inmediato de 5 bits sin signo de las variantes csr*i.
    fn uimm5(&self, i: usize) -> Result<u32, AsmError> {
        let v = self.imm(i)?;
        if !(0..=31).contains(&v) {
            return Err(AsmError::new(
                self.ins.line,
                self.ins.ops[i].col,
                format!("el inmediato de las csr*i va de 0 a 31 (hay {v})"),
            ));
        }
        Ok(v as u32)
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
        // La codificación tipo B descarta el bit 0: un offset impar se
        // ensamblaría mal en silencio, así que se rechaza acá.
        if off & 1 != 0 {
            return Err(AsmError::new(
                self.ins.line,
                self.ins.ops[i].col,
                "destino de salto no alineado a 2 bytes",
            ));
        }
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
        // La codificación tipo J descarta el bit 0: un offset impar se
        // ensamblaría mal en silencio, así que se rechaza acá.
        if off & 1 != 0 {
            return Err(AsmError::new(
                self.ins.line,
                self.ins.ops[i].col,
                "destino de salto no alineado a 2 bytes",
            ));
        }
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
        let v = self.imm(i)?;
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

fn encode_instr(
    ins: &PInstr,
    symbols: &BTreeMap<String, u32>,
    reubicable: bool,
) -> Result<Vec<u32>, AsmError> {
    let c = Ctx {
        ins,
        symbols,
        reubicable,
    };
    let m = ins.mnem.as_str();
    let w = match m {
        // ---- R-type ----
        "add" => enc_r(0x00, c.reg(2)?, c.reg(1)?, 0, c.reg(0)?, 0x33),
        "sub" => enc_r(0x20, c.reg(2)?, c.reg(1)?, 0, c.reg(0)?, 0x33),
        "sll" => enc_r(0x00, c.reg(2)?, c.reg(1)?, 1, c.reg(0)?, 0x33),
        "slt" => enc_r(0x00, c.reg(2)?, c.reg(1)?, 2, c.reg(0)?, 0x33),
        "sltu" => enc_r(0x00, c.reg(2)?, c.reg(1)?, 3, c.reg(0)?, 0x33),
        "xor" => enc_r(0x00, c.reg(2)?, c.reg(1)?, 4, c.reg(0)?, 0x33),
        "srl" => enc_r(0x00, c.reg(2)?, c.reg(1)?, 5, c.reg(0)?, 0x33),
        "sra" => enc_r(0x20, c.reg(2)?, c.reg(1)?, 5, c.reg(0)?, 0x33),
        "or" => enc_r(0x00, c.reg(2)?, c.reg(1)?, 6, c.reg(0)?, 0x33),
        "and" => enc_r(0x00, c.reg(2)?, c.reg(1)?, 7, c.reg(0)?, 0x33),
        // ---- RV32M ----
        "mul" => enc_r(0x01, c.reg(2)?, c.reg(1)?, 0, c.reg(0)?, 0x33),
        "mulh" => enc_r(0x01, c.reg(2)?, c.reg(1)?, 1, c.reg(0)?, 0x33),
        "mulhsu" => enc_r(0x01, c.reg(2)?, c.reg(1)?, 2, c.reg(0)?, 0x33),
        "mulhu" => enc_r(0x01, c.reg(2)?, c.reg(1)?, 3, c.reg(0)?, 0x33),
        "div" => enc_r(0x01, c.reg(2)?, c.reg(1)?, 4, c.reg(0)?, 0x33),
        "divu" => enc_r(0x01, c.reg(2)?, c.reg(1)?, 5, c.reg(0)?, 0x33),
        "rem" => enc_r(0x01, c.reg(2)?, c.reg(1)?, 6, c.reg(0)?, 0x33),
        "remu" => enc_r(0x01, c.reg(2)?, c.reg(1)?, 7, c.reg(0)?, 0x33),
        // ---- I-type aritmética ----
        "addi" => enc_i(c.check_imm12(2, c.imm(2)?)?, c.reg(1)?, 0, c.reg(0)?, 0x13),
        "slti" => enc_i(c.check_imm12(2, c.imm(2)?)?, c.reg(1)?, 2, c.reg(0)?, 0x13),
        "sltiu" => enc_i(c.check_imm12(2, c.imm(2)?)?, c.reg(1)?, 3, c.reg(0)?, 0x13),
        "xori" => enc_i(c.check_imm12(2, c.imm(2)?)?, c.reg(1)?, 4, c.reg(0)?, 0x13),
        "ori" => enc_i(c.check_imm12(2, c.imm(2)?)?, c.reg(1)?, 6, c.reg(0)?, 0x13),
        "andi" => enc_i(c.check_imm12(2, c.imm(2)?)?, c.reg(1)?, 7, c.reg(0)?, 0x13),
        "slli" => enc_i(c.shamt(2)? as i32, c.reg(1)?, 1, c.reg(0)?, 0x13),
        "srli" => enc_i(c.shamt(2)? as i32, c.reg(1)?, 5, c.reg(0)?, 0x13),
        "srai" => enc_i((c.shamt(2)? as i32) | 0x400, c.reg(1)?, 5, c.reg(0)?, 0x13),
        // ---- U / J ----
        "lui" | "auipc" => {
            let v = c.imm(1)?;
            if !(-(1 << 19)..(1 << 20)).contains(&v) {
                return Err(AsmError::new(
                    ins.line,
                    ins.ops[1].col,
                    format!("el inmediato de «{m}» debe caber en 20 bits (hay {v})"),
                ));
            }
            let op = if m == "lui" { 0x37 } else { 0x17 };
            enc_u((v as u32) & 0xFFFFF, c.reg(0)?, op)
        }
        "jal" => {
            let (rd, ti) = if ins.ops.len() == 1 {
                (1u8, 0)
            } else {
                (c.reg(0)?, 1)
            };
            enc_j(c.jal_off(ti)?, rd)
        }
        "j" => enc_j(c.jal_off(0)?, 0),
        "call" => enc_j(c.jal_off(0)?, 1),
        "tail" => enc_j(c.jal_off(0)?, 0),
        "jalr" => match ins.ops.len() {
            1 => enc_i(0, c.reg(0)?, 0, 1, 0x67),
            2 => {
                let (off, base) = c.mem(1)?;
                enc_i(c.check_imm12(1, off)?, base, 0, c.reg(0)?, 0x67)
            }
            _ => enc_i(c.check_imm12(2, c.imm(2)?)?, c.reg(1)?, 0, c.reg(0)?, 0x67),
        },
        "jr" => enc_i(0, c.reg(0)?, 0, 0, 0x67),
        "ret" => enc_i(0, 1, 0, 0, 0x67),
        // ---- saltos condicionales ----
        "beq" => enc_b(c.branch_off(2)?, c.reg(1)?, c.reg(0)?, 0),
        "bne" => enc_b(c.branch_off(2)?, c.reg(1)?, c.reg(0)?, 1),
        "blt" => enc_b(c.branch_off(2)?, c.reg(1)?, c.reg(0)?, 4),
        "bge" => enc_b(c.branch_off(2)?, c.reg(1)?, c.reg(0)?, 5),
        "bltu" => enc_b(c.branch_off(2)?, c.reg(1)?, c.reg(0)?, 6),
        "bgeu" => enc_b(c.branch_off(2)?, c.reg(1)?, c.reg(0)?, 7),
        // pseudo con operandos invertidos
        "bgt" => enc_b(c.branch_off(2)?, c.reg(0)?, c.reg(1)?, 4),
        "ble" => enc_b(c.branch_off(2)?, c.reg(0)?, c.reg(1)?, 5),
        "bgtu" => enc_b(c.branch_off(2)?, c.reg(0)?, c.reg(1)?, 6),
        "bleu" => enc_b(c.branch_off(2)?, c.reg(0)?, c.reg(1)?, 7),
        // comparación con cero
        "beqz" => enc_b(c.branch_off(1)?, 0, c.reg(0)?, 0),
        "bnez" => enc_b(c.branch_off(1)?, 0, c.reg(0)?, 1),
        "bltz" => enc_b(c.branch_off(1)?, 0, c.reg(0)?, 4),
        "bgez" => enc_b(c.branch_off(1)?, 0, c.reg(0)?, 5),
        "blez" => enc_b(c.branch_off(1)?, c.reg(0)?, 0, 5), // 0 >= rs ⇔ rs <= 0
        "bgtz" => enc_b(c.branch_off(1)?, c.reg(0)?, 0, 4), // 0 < rs ⇔ rs > 0
        // ---- pseudo de un solo word ----
        "nop" => enc_i(0, 0, 0, 0, 0x13),
        "mv" => enc_i(0, c.reg(1)?, 0, c.reg(0)?, 0x13),
        "not" => enc_i(-1, c.reg(1)?, 4, c.reg(0)?, 0x13),
        "neg" => enc_r(0x20, c.reg(1)?, 0, 0, c.reg(0)?, 0x33),
        "seqz" => enc_i(1, c.reg(1)?, 3, c.reg(0)?, 0x13),
        "snez" => enc_r(0x00, c.reg(1)?, 0, 3, c.reg(0)?, 0x33),
        "sltz" => enc_r(0x00, 0, c.reg(1)?, 2, c.reg(0)?, 0x33),
        "sgtz" => enc_r(0x00, c.reg(1)?, 0, 2, c.reg(0)?, 0x33),
        // ---- sistema ----
        "ecall" => 0x0000_0073,
        "ebreak" => 0x0010_0073,
        "fence" => 0x0FF0_000F,
        "mret" => 0x3020_0073,
        // ---- Zicsr (RP8): (csr<<20)|(rs1|uimm5)<<15|(f3<<12)|(rd<<7)|0x73
        "csrrw" => enc_csr(c.csr(1)?, c.reg(2)? as u32, 1, c.reg(0)?),
        "csrrs" => enc_csr(c.csr(1)?, c.reg(2)? as u32, 2, c.reg(0)?),
        "csrrc" => enc_csr(c.csr(1)?, c.reg(2)? as u32, 3, c.reg(0)?),
        "csrrwi" => enc_csr(c.csr(1)?, c.uimm5(2)?, 5, c.reg(0)?),
        "csrrsi" => enc_csr(c.csr(1)?, c.uimm5(2)?, 6, c.reg(0)?),
        "csrrci" => enc_csr(c.csr(1)?, c.uimm5(2)?, 7, c.reg(0)?),
        // pseudo: csrr rd, csr = csrrs rd, csr, x0
        "csrr" => enc_csr(c.csr(1)?, 0, 2, c.reg(0)?),
        // pseudo: csrw/csrs/csrc csr, rs = csrr{w,s,c} x0, csr, rs
        "csrw" => enc_csr(c.csr(0)?, c.reg(1)? as u32, 1, 0),
        "csrs" => enc_csr(c.csr(0)?, c.reg(1)? as u32, 2, 0),
        "csrc" => enc_csr(c.csr(0)?, c.reg(1)? as u32, 3, 0),
        // ---- RV32F/D (RP12): opcode OP-FP 0x53; rm=dyn(7) salvo campos
        // fijos; las conversiones EXACTAS (ensanchan) llevan rm=0 como GNU.
        "fadd.s" => enc_r(0x00, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fsub.s" => enc_r(0x04, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fmul.s" => enc_r(0x08, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fdiv.s" => enc_r(0x0C, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fsqrt.s" => enc_r(0x2C, 0, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fsgnj.s" => enc_r(0x10, c.freg(2)?, c.freg(1)?, 0, c.freg(0)?, 0x53),
        "fsgnjn.s" => enc_r(0x10, c.freg(2)?, c.freg(1)?, 1, c.freg(0)?, 0x53),
        "fsgnjx.s" => enc_r(0x10, c.freg(2)?, c.freg(1)?, 2, c.freg(0)?, 0x53),
        "fmin.s" => enc_r(0x14, c.freg(2)?, c.freg(1)?, 0, c.freg(0)?, 0x53),
        "fmax.s" => enc_r(0x14, c.freg(2)?, c.freg(1)?, 1, c.freg(0)?, 0x53),
        "fcvt.w.s" => enc_r(0x60, 0, c.freg(1)?, 7, c.reg(0)?, 0x53),
        "fcvt.wu.s" => enc_r(0x60, 1, c.freg(1)?, 7, c.reg(0)?, 0x53),
        "fcvt.s.w" => enc_r(0x68, 0, c.reg(1)?, 7, c.freg(0)?, 0x53),
        "fcvt.s.wu" => enc_r(0x68, 1, c.reg(1)?, 7, c.freg(0)?, 0x53),
        "fmv.x.w" => enc_r(0x70, 0, c.freg(1)?, 0, c.reg(0)?, 0x53),
        "fmv.w.x" => enc_r(0x78, 0, c.reg(1)?, 0, c.freg(0)?, 0x53),
        "fclass.s" => enc_r(0x70, 0, c.freg(1)?, 1, c.reg(0)?, 0x53),
        "feq.s" => enc_r(0x50, c.freg(2)?, c.freg(1)?, 2, c.reg(0)?, 0x53),
        "flt.s" => enc_r(0x50, c.freg(2)?, c.freg(1)?, 1, c.reg(0)?, 0x53),
        "fle.s" => enc_r(0x50, c.freg(2)?, c.freg(1)?, 0, c.reg(0)?, 0x53),
        "fadd.d" => enc_r(0x01, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fsub.d" => enc_r(0x05, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fmul.d" => enc_r(0x09, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fdiv.d" => enc_r(0x0D, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fsqrt.d" => enc_r(0x2D, 0, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fsgnj.d" => enc_r(0x11, c.freg(2)?, c.freg(1)?, 0, c.freg(0)?, 0x53),
        "fsgnjn.d" => enc_r(0x11, c.freg(2)?, c.freg(1)?, 1, c.freg(0)?, 0x53),
        "fsgnjx.d" => enc_r(0x11, c.freg(2)?, c.freg(1)?, 2, c.freg(0)?, 0x53),
        "fmin.d" => enc_r(0x15, c.freg(2)?, c.freg(1)?, 0, c.freg(0)?, 0x53),
        "fmax.d" => enc_r(0x15, c.freg(2)?, c.freg(1)?, 1, c.freg(0)?, 0x53),
        "fcvt.s.d" => enc_r(0x20, 1, c.freg(1)?, 7, c.freg(0)?, 0x53),
        "fcvt.d.s" => enc_r(0x21, 0, c.freg(1)?, 0, c.freg(0)?, 0x53),
        "fcvt.w.d" => enc_r(0x61, 0, c.freg(1)?, 7, c.reg(0)?, 0x53),
        "fcvt.wu.d" => enc_r(0x61, 1, c.freg(1)?, 7, c.reg(0)?, 0x53),
        "fcvt.d.w" => enc_r(0x69, 0, c.reg(1)?, 0, c.freg(0)?, 0x53),
        "fcvt.d.wu" => enc_r(0x69, 1, c.reg(1)?, 0, c.freg(0)?, 0x53),
        "fclass.d" => enc_r(0x71, 0, c.freg(1)?, 1, c.reg(0)?, 0x53),
        "feq.d" => enc_r(0x51, c.freg(2)?, c.freg(1)?, 2, c.reg(0)?, 0x53),
        "flt.d" => enc_r(0x51, c.freg(2)?, c.freg(1)?, 1, c.reg(0)?, 0x53),
        "fle.d" => enc_r(0x51, c.freg(2)?, c.freg(1)?, 0, c.reg(0)?, 0x53),
        // R4 (fused multiply-add): fmt en funct2 (S=0, D=1), rm=dyn.
        "fmadd.s" => enc_r4(c.freg(3)?, 0, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x43),
        "fmsub.s" => enc_r4(c.freg(3)?, 0, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x47),
        "fnmsub.s" => enc_r4(c.freg(3)?, 0, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x4B),
        "fnmadd.s" => enc_r4(c.freg(3)?, 0, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x4F),
        "fmadd.d" => enc_r4(c.freg(3)?, 1, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x43),
        "fmsub.d" => enc_r4(c.freg(3)?, 1, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x47),
        "fnmsub.d" => enc_r4(c.freg(3)?, 1, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x4B),
        "fnmadd.d" => enc_r4(c.freg(3)?, 1, c.freg(2)?, c.freg(1)?, 7, c.freg(0)?, 0x4F),
        // pseudos sgnj (mismo registro repetido, como GNU)
        "fmv.s" => enc_r(0x10, c.freg(1)?, c.freg(1)?, 0, c.freg(0)?, 0x53),
        "fabs.s" => enc_r(0x10, c.freg(1)?, c.freg(1)?, 2, c.freg(0)?, 0x53),
        "fneg.s" => enc_r(0x10, c.freg(1)?, c.freg(1)?, 1, c.freg(0)?, 0x53),
        "fmv.d" => enc_r(0x11, c.freg(1)?, c.freg(1)?, 0, c.freg(0)?, 0x53),
        "fabs.d" => enc_r(0x11, c.freg(1)?, c.freg(1)?, 2, c.freg(0)?, 0x53),
        "fneg.d" => enc_r(0x11, c.freg(1)?, c.freg(1)?, 1, c.freg(0)?, 0x53),
        // cargas/almacenamientos f (pueden expandir a 2 words con etiqueta)
        "flw" | "fld" => return encode_fload(&c, m),
        "fsw" | "fsd" => return encode_fstore(&c, m),
        // ---- cargas / almacenamientos / li / la: pueden expandir a 2 words
        "lb" | "lh" | "lw" | "lbu" | "lhu" => return encode_load(&c, m),
        "sb" | "sh" | "sw" => return encode_store(&c, m),
        "li" => return encode_li(&c),
        "la" | "lla" => {
            let addr = c.target(1)?;
            let rd = c.reg(0)?;
            return Ok(if c.reubicable {
                // PC-relativo: auipc rd, %pcrel_hi ; addi rd, rd, %pcrel_lo
                let off = c.pcrel(addr);
                vec![
                    enc_u(hi20(off), rd, 0x17),
                    enc_i(lo12(off), rd, 0, rd, 0x13),
                ]
            } else {
                // Absoluto: lui rd, %hi ; addi rd, rd, %lo
                vec![
                    enc_u(hi20(addr), rd, 0x37),
                    enc_i(lo12(addr), rd, 0, rd, 0x13),
                ]
            });
        }
        // ---- RV32A (RP13): opcode AMO 0x2F, funct3=010 (.w) ----
        _ if parse_amo(m).is_some() => {
            let (funct5, aq, rl, es_lr, _es_sc) = parse_amo(m).unwrap();
            // (rs1) es el operando de memoria con offset 0.
            let base = |i: usize| -> Result<u8, AsmError> {
                match c.ins.ops.get(i).map(|o| &o.kind) {
                    Some(OpKind::Mem { off: 0, base }) => Ok(*base),
                    _ => Err(c.internal_err(i, "una dirección «(rsN)» sin desplazamiento")),
                }
            };
            let (rd, rs2, rs1) = if es_lr {
                // lr.w rd, (rs1): rs2 debe ser 0 en el encoding.
                (c.reg(0)?, 0u8, base(1)?)
            } else {
                (c.reg(0)?, c.reg(1)?, base(2)?)
            };
            enc_amo(funct5, aq, rl, rs2, rs1, rd)
        }
        other => {
            return Err(AsmError::new(
                ins.line,
                1,
                format!(
                    "error interno del ensamblador: el mnemónico «{other}» pasó la \
                     validación de la pasada 1 pero no tiene codificación en la \
                     pasada 2 (reportá este bug)"
                ),
            ));
        }
    };
    Ok(vec![w])
}

/// funct3 de cargas/almacenamientos. Devuelve error interno si el mnemónico
/// no es de este grupo (antes era un `unreachable!` que tiraba la app).
fn load_f3(c: &Ctx, m: &str) -> Result<u32, AsmError> {
    match m {
        "lb" => Ok(0),
        "lh" => Ok(1),
        "lw" => Ok(2),
        "lbu" => Ok(4),
        "lhu" => Ok(5),
        "sb" => Ok(0),
        "sh" => Ok(1),
        "sw" => Ok(2),
        _ => Err(AsmError::new(
            c.ins.line,
            1,
            format!(
                "error interno del ensamblador: «{m}» no es una carga ni un \
                 almacenamiento conocido (reportá este bug)"
            ),
        )),
    }
}

fn encode_load(c: &Ctx, m: &str) -> Result<Vec<u32>, AsmError> {
    let rd = c.reg(0)?;
    match c.ins.ops.get(1).map(|o| &o.kind) {
        Some(OpKind::Mem { off, base }) => {
            let imm = c.check_imm12(1, *off)?;
            Ok(vec![enc_i(imm, *base, load_f3(c, m)?, rd, 0x03)])
        }
        Some(OpKind::MemLo { .. }) => {
            // %lo(x)(base): c.mem ya resuelve el %lo a su valor con signo.
            let (off, base) = c.mem(1)?;
            Ok(vec![enc_i(off as i32, base, load_f3(c, m)?, rd, 0x03)])
        }
        Some(OpKind::Sym(_) | OpKind::Imm(_)) => {
            let addr = c.target(1)?;
            if c.reubicable {
                // lw rd, etiqueta → auipc rd, %pcrel_hi ; lw rd, %pcrel_lo(rd)
                let off = c.pcrel(addr);
                Ok(vec![
                    enc_u(hi20(off), rd, 0x17),
                    enc_i(lo12(off), rd, load_f3(c, m)?, rd, 0x03),
                ])
            } else {
                // lw rd, etiqueta → lui rd, %hi ; lw rd, %lo(rd)
                Ok(vec![
                    enc_u(hi20(addr), rd, 0x37),
                    enc_i(lo12(addr), rd, load_f3(c, m)?, rd, 0x03),
                ])
            }
        }
        _ => Err(c.internal_err(1, "un operando de memoria o una etiqueta")),
    }
}

fn encode_store(c: &Ctx, m: &str) -> Result<Vec<u32>, AsmError> {
    let rs2 = c.reg(0)?;
    if c.ins.ops.len() == 2 {
        let (off, base) = c.mem(1)?;
        let imm = c.check_imm12(1, off)?;
        Ok(vec![enc_s(imm, rs2, base, load_f3(c, m)?)])
    } else {
        // sw rs2, etiqueta, rt → (abs) lui rt,%hi;sw rs2,%lo(rt)
        //                        (reub) auipc rt,%pcrel_hi;sw rs2,%pcrel_lo(rt)
        let addr = c.target(1)?;
        let rt = c.reg(2)?;
        if rt == 0 {
            return Err(AsmError::new(
                c.ins.line,
                c.ins.ops[2].col,
                "el registro auxiliar de un almacenamiento con etiqueta no puede ser zero/x0",
            ));
        }
        if c.reubicable {
            let off = c.pcrel(addr);
            Ok(vec![
                enc_u(hi20(off), rt, 0x17),
                enc_s(lo12(off), rs2, rt, load_f3(c, m)?),
            ])
        } else {
            Ok(vec![
                enc_u(hi20(addr), rt, 0x37),
                enc_s(lo12(addr), rs2, rt, load_f3(c, m)?),
            ])
        }
    }
}

/// R4 (fmadd y familia): rs3 en los 5 bits altos, fmt en funct2.
fn enc_r4(rs3: u8, fmt: u32, rs2: u8, rs1: u8, rm: u32, rd: u8, opcode: u32) -> u32 {
    ((rs3 as u32) << 27)
        | (fmt << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (rm << 12)
        | ((rd as u32) << 7)
        | opcode
}

/// flw/fld: directo, %lo(base) o con etiqueta + registro entero auxiliar
/// (el destino f no puede hacer de puntero, a diferencia de lw).
fn encode_fload(c: &Ctx, m: &str) -> Result<Vec<u32>, AsmError> {
    let f3 = if m == "flw" { 2 } else { 3 };
    let rd = c.freg(0)?;
    match c.ins.ops.get(1).map(|o| &o.kind) {
        Some(OpKind::Mem { off, base }) => {
            let imm = c.check_imm12(1, *off)?;
            Ok(vec![enc_i(imm, *base, f3, rd, 0x07)])
        }
        Some(OpKind::MemLo { .. }) => {
            let (off, base) = c.mem(1)?;
            Ok(vec![enc_i(off as i32, base, f3, rd, 0x07)])
        }
        Some(OpKind::Sym(_) | OpKind::Imm(_)) => {
            let addr = c.target(1)?;
            let rt = c.reg(2)?;
            if rt == 0 {
                return Err(AsmError::new(
                    c.ins.line,
                    c.ins.ops[2].col,
                    "el registro auxiliar de una carga f con etiqueta no puede ser zero/x0",
                ));
            }
            if c.reubicable {
                let off = c.pcrel(addr);
                Ok(vec![
                    enc_u(hi20(off), rt, 0x17),
                    enc_i(lo12(off), rt, f3, rd, 0x07),
                ])
            } else {
                Ok(vec![
                    enc_u(hi20(addr), rt, 0x37),
                    enc_i(lo12(addr), rt, f3, rd, 0x07),
                ])
            }
        }
        _ => Err(c.internal_err(1, "un operando de memoria o una etiqueta")),
    }
}

/// fsw/fsd: espejo de encode_fload sobre STORE-FP (0x27).
fn encode_fstore(c: &Ctx, m: &str) -> Result<Vec<u32>, AsmError> {
    let f3 = if m == "fsw" { 2 } else { 3 };
    let rs2 = c.freg(0)?;
    if c.ins.ops.len() == 2 {
        let (off, base) = c.mem(1)?;
        let imm = c.check_imm12(1, off)?;
        Ok(vec![enc_s_fp(imm, rs2, base, f3)])
    } else {
        let addr = c.target(1)?;
        let rt = c.reg(2)?;
        if rt == 0 {
            return Err(AsmError::new(
                c.ins.line,
                c.ins.ops[2].col,
                "el registro auxiliar de un almacenamiento f con etiqueta no puede ser zero/x0",
            ));
        }
        if c.reubicable {
            let off = c.pcrel(addr);
            Ok(vec![
                enc_u(hi20(off), rt, 0x17),
                enc_s_fp(lo12(off), rs2, rt, f3),
            ])
        } else {
            Ok(vec![
                enc_u(hi20(addr), rt, 0x37),
                enc_s_fp(lo12(addr), rs2, rt, f3),
            ])
        }
    }
}

/// Codifica una instrucción comprimida (extensión C, RP13) en 16 bits.
/// Encodings verificados a mano contra `riscv64-elf-as -march=rv32imac`
/// (ver BITACORA RP13-F3). Los inmediatos con rango/alineación inválidos
/// dan error didáctico — el ensamblador no comprime solo, así que un valor
/// fuera de rango es siempre culpa de lo que el usuario escribió.
fn encode_compressed(ins: &PInstr, symbols: &BTreeMap<String, u32>) -> Result<u16, AsmError> {
    // Las comprimidas con salto (c.j/c.beqz/c.bnez) ya son PC-relativas, así
    // que el modo reubicable no las afecta: `reubicable: false` es correcto.
    let c = Ctx {
        ins,
        symbols,
        reubicable: false,
    };
    let m = ins.mnem.as_str();
    let err = |col: u32, msg: String| AsmError::new(ins.line, col, msg);
    let w: u16 = match m {
        "c.nop" => 0x0001,
        "c.addi" | "c.li" => {
            let rd = c.reg(0)?;
            let imm = c.imm(1)?;
            if m == "c.addi" && rd == 0 {
                return Err(err(ins.ops[0].col, "«c.addi» no admite x0/zero como destino".into()));
            }
            if rd == 0 {
                return Err(err(ins.ops[0].col, "«c.li» no admite x0/zero como destino".into()));
            }
            if !(-32..=31).contains(&imm) {
                return Err(err(
                    ins.ops[1].col,
                    format!("el inmediato de «{m}» debe caber en 6 bits con signo (-32..31); hay {imm}"),
                ));
            }
            let funct3: u16 = if m == "c.addi" { 0b000 } else { 0b010 };
            let imm = imm as u16 & 0x3F;
            (funct3 << 13) | ((imm >> 5) << 12) | ((rd as u16) << 7) | ((imm & 0x1F) << 2) | 0b01
        }
        "c.mv" => {
            let (rd, rs2) = (c.reg(0)?, c.reg(1)?);
            if rd == 0 || rs2 == 0 {
                return Err(err(ins.ops[0].col, "«c.mv» no admite x0/zero".into()));
            }
            (0b1000 << 12) | ((rd as u16) << 7) | ((rs2 as u16) << 2) | 0b10
        }
        "c.jr" => {
            let rs1 = c.reg(0)?;
            if rs1 == 0 {
                return Err(err(ins.ops[0].col, "«c.jr» no admite x0/zero".into()));
            }
            (0b1000 << 12) | ((rs1 as u16) << 7) | 0b10
        }
        "c.j" => {
            let off = c.branch_off(0)?;
            if !(-2048..=2046).contains(&off) || off % 2 != 0 {
                return Err(err(ins.ops[0].col, format!("«c.j» fuera de alcance (±2 KB, par); offset {off}")));
            }
            let o = off as u16 & 0xFFF;
            let bit = |b: u16| (o >> b) & 1;
            let imm: u16 = (bit(11) << 10)
                | (bit(4) << 9)
                | (bit(9) << 8)
                | (bit(8) << 7)
                | (bit(10) << 6)
                | (bit(6) << 5)
                | (bit(7) << 4)
                | (bit(3) << 3)
                | (bit(2) << 2)
                | (bit(1) << 1)
                | bit(5);
            (0b101 << 13) | (imm << 2) | 0b01
        }
        "c.lw" | "c.sw" => {
            let r = creg(c.reg(0)?).ok_or_else(|| {
                err(ins.ops[0].col, format!("«{m}» solo alcanza x8–x15"))
            })?;
            let (off, base) = c.mem(1)?;
            let base = creg(base).ok_or_else(|| {
                err(ins.ops[1].col, format!("«{m}»: la base solo puede ser x8–x15"))
            })?;
            if !(0..=124).contains(&off) || off % 4 != 0 {
                return Err(err(
                    ins.ops[1].col,
                    format!("«{m}»: el desplazamiento debe ser múltiplo de 4 entre 0 y 124; hay {off}"),
                ));
            }
            let o = off as u16;
            let funct3: u16 = if m == "c.lw" { 0b010 } else { 0b110 };
            // opcode de C de loads/stores = 0b00 (omitido: el OR sería no-op)
            (funct3 << 13)
                | (((o >> 3) & 0x7) << 10)
                | ((base as u16) << 7)
                | (((o >> 2) & 1) << 6)
                | (((o >> 6) & 1) << 5)
                | ((r as u16) << 2)
        }
        "c.beqz" | "c.bnez" => {
            let rs1 = creg(c.reg(0)?).ok_or_else(|| {
                err(ins.ops[0].col, format!("«{m}» solo alcanza x8–x15"))
            })?;
            let off = c.branch_off(1)?;
            if !(-256..=254).contains(&off) || off % 2 != 0 {
                return Err(err(ins.ops[1].col, format!("«{m}» fuera de alcance (±256 B, par); offset {off}")));
            }
            let o = off as u16 & 0x1FF;
            let bit = |b: u16| (o >> b) & 1;
            let funct3: u16 = if m == "c.beqz" { 0b110 } else { 0b111 };
            let imm_hi = (bit(8) << 2) | (bit(4) << 1) | bit(3);
            let imm_lo = (bit(7) << 4) | (bit(6) << 3) | (bit(2) << 2) | (bit(1) << 1) | bit(5);
            (funct3 << 13) | (imm_hi << 10) | ((rs1 as u16) << 7) | (imm_lo << 2) | 0b01
        }
        otro => {
            return Err(err(1, format!("error interno: «{otro}» no tiene encoding comprimido (bug)")));
        }
    };
    Ok(w)
}

fn encode_li(c: &Ctx) -> Result<Vec<u32>, AsmError> {
    let rd = c.reg(0)?;
    let v = c.imm(1)?;
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
    fn salto_condicional_a_destino_impar_es_error() {
        // 0x400009 está a 9 bytes de TEXT_BASE: offset impar que la
        // codificación tipo B descartaría en silencio (bit 0).
        let e = first_error("beq x1, x2, 0x400009");
        assert_eq!(e.line, 1);
        assert!(e.msg.contains("no alineado a 2 bytes"), "{}", e.msg);
    }

    #[test]
    fn jal_a_destino_impar_es_error() {
        // Mismo caso para la codificación tipo J (jal).
        let e = first_error("jal x1, 0x400009");
        assert_eq!(e.line, 1);
        assert!(e.msg.contains("no alineado a 2 bytes"), "{}", e.msg);
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

    // ---- RP5 Fase 1: constantes .eqv/.equ y operadores %hi/%lo ----

    #[test]
    fn constantes_eqv_equ_encadenadas() {
        // .eqv estilo RARS (sin coma), .equ estilo GNU (con coma), y una
        // constante definida desde otra.
        let src = ".eqv BASE 0x100\n.equ DOBLE, 0x200\n.eqv ALIAS BASE\nli t0, BASE\nli t1, DOBLE\nli t2, ALIAS";
        assert_eq!(
            asm_words(src),
            asm_words("li t0, 0x100\nli t1, 0x200\nli t2, 0x100")
        );
    }

    #[test]
    fn constante_redefinida_es_error() {
        let e = first_error(".eqv N 1\n.eqv N 2\nnop");
        assert!(e.msg.contains("redefinida"), "{}", e.msg);
        assert_eq!(e.line, 2);
    }

    #[test]
    fn constante_con_nombre_de_registro_es_error() {
        let e = first_error(".eqv t0 5\nnop");
        assert!(e.msg.contains("registro"), "{}", e.msg);
    }

    #[test]
    fn constante_y_etiqueta_chocan_en_ambos_sentidos() {
        let e = first_error(".eqv x 1\nx: nop");
        assert!(e.msg.contains("constante"), "{}", e.msg);
        let e = first_error(".text\nx: nop\n.eqv x 1");
        assert!(e.msg.contains("etiqueta"), "{}", e.msg);
    }

    #[test]
    fn constante_usable_en_word_y_la() {
        let p = assemble(".eqv N 42\n.data\nv: .word N\n.text\nnop").expect("ensambla");
        assert_eq!(&p.data[0..4], &42u32.to_le_bytes());
        // la CONSTANTE == li de 8 bytes del mismo valor (ambos lui+addi).
        let a = asm_words(".eqv DIR 0x12345\nla t0, DIR");
        let b = asm_words("li t0, 0x12345");
        assert_eq!(a, b);
    }

    #[test]
    fn hi_lo_equivalen_a_la_con_ajuste_de_signo() {
        // dato queda en DATA_BASE + 2052: el %lo (2052 > 2047) es NEGATIVO
        // y el %hi compensa (+0x800 antes del shift) — el par lui+addi debe
        // reconstruir la dirección EXACTA, byte a byte igual que «la».
        let con_la = asm_words(".data\n.space 2052\ndato: .word 7\n.text\nla t0, dato");
        let con_hilo = asm_words(
            ".data\n.space 2052\ndato: .word 7\n.text\nlui t0, %hi(dato)\naddi t0, t0, %lo(dato)",
        );
        assert_eq!(con_la, con_hilo);
    }

    #[test]
    fn hi_lo_con_referencia_hacia_adelante_y_lo_en_memoria() {
        // La etiqueta se define DESPUÉS de usarse (pasada 2, como siempre)
        // y %lo(dato)(t0) funciona como desplazamiento de carga.
        let w = asm_words(".text\nlui t0, %hi(dato)\nlw t1, %lo(dato)(t0)\n.data\ndato: .word 7");
        let w2 = asm_words(".text\nlw t1, dato\n.data\ndato: .word 7");
        // lw rd, etiqueta expande a lui rd + lw — mismos words salvo el
        // registro intermedio (acá t0 vs t1): comparar el lw final ajustado
        // no vale; alcanza con que la dirección reconstruida coincida:
        // lui t0,%hi + lw t1,%lo(t0) carga lo mismo que la expansión.
        assert_eq!(w.len(), 2);
        assert_eq!(w2.len(), 2);
        // El offset inmediato del lw (bits 31:20) debe ser el mismo.
        assert_eq!(w[1] >> 20, w2[1] >> 20);
    }

    #[test]
    fn hi_con_auipc_es_error_didactico() {
        let e = first_error("auipc t0, %hi(x)\nx: nop");
        assert!(e.msg.contains("pcrel"), "{}", e.msg);
    }

    #[test]
    fn lo_con_valor_negativo_ajusta_signo() {
        // 0x12345FFF tiene low12 = 0xFFF (negativo como i12): lui+addi con
        // %hi/%lo debe dar exactamente el li de 32 bits.
        let a = asm_words("lui t0, %hi(0x12345FFF)\naddi t0, t0, %lo(0x12345FFF)");
        let b = asm_words("li t0, 0x12345FFF");
        assert_eq!(a, b);
    }

    // ---- RP8 Fase 2: Zicsr y mret ----

    #[test]
    fn zicsr_encodings_de_referencia() {
        // Valores calculados a mano contra la espec privilegiada (formato I
        // sobre SYSTEM 0x73) y CONSISTENTES con tests/spike_csr.rs, que los
        // ejecutó nativos en Unicorn.
        assert_eq!(asm_words("csrrw x0, mscratch, t0")[0], 0x3402_9073);
        assert_eq!(asm_words("csrr t1, mscratch")[0], 0x3400_2373);
        assert_eq!(asm_words("csrrwi x0, mscratch, 21")[0], 0x340A_D073);
        assert_eq!(asm_words("csrw mtvec, t0")[0], 0x3052_9073);
        assert_eq!(asm_words("csrs mstatus, t1")[0], 0x3003_2073);
        assert_eq!(asm_words("csrrci t2, mip, 1")[0], 0x3440_F3F3);
        assert_eq!(asm_words("mret")[0], 0x3020_0073);
        // CSR por número crudo.
        assert_eq!(asm_words("csrr t0, 0x340")[0], 0x3400_22F3);
    }

    #[test]
    fn zicsr_errores_didacticos() {
        let e = first_error("csrr t0, mfoo");
        assert!(
            e.msg.contains("mfoo") && e.msg.contains("mstatus"),
            "{}",
            e.msg
        );
        let e = first_error("csrrwi t0, mscratch, 32");
        assert!(e.msg.contains("0 a 31"), "{}", e.msg);
        let e = first_error("csrr t0, 0x1000");
        assert!(e.msg.contains("fuera de rango"), "{}", e.msg);
        let e = first_error("csrrw t0, mscratch");
        assert!(e.msg.contains("espera 3 operandos"), "{}", e.msg);
    }
}
