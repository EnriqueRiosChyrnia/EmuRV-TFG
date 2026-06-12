//! Condiciones de breakpoint (RP7): gramática mínima `REG OP VALOR`.
//!
//! - REG: nombre ABI (t0, a1, sp, …) o xN.
//! - OP: == != < <= > >= — comparación SIGNED (i32), la natural para el
//!   estudiante que cuenta iteraciones o compara con -1.
//! - VALOR: inmediato decimal o hex 0x…, o un segundo registro.
//!
//! El parseo ocurre AL DEFINIR la condición, con errores didácticos en
//! español; el hook de código solo evalúa un AST ya validado (costo por
//! instrucción mínimo). Nada de && ni aritmética: esto es la versión
//! didáctica — para más, está el stub gdb (RP-6).

use super::assembler::reg_number;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondVal {
    Reg(u8),
    Imm(i32),
}

/// Condición validada: `reg OP val`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CondAst {
    pub reg: u8,
    pub op: CondOp,
    pub val: CondVal,
}

impl CondAst {
    /// Evalúa con los valores actuales (`leer` devuelve el registro N).
    /// Comparación con signo (i32).
    pub fn eval(&self, leer: impl Fn(u8) -> u32) -> bool {
        let a = leer(self.reg) as i32;
        let b = match self.val {
            CondVal::Reg(r) => leer(r) as i32,
            CondVal::Imm(v) => v,
        };
        match self.op {
            CondOp::Eq => a == b,
            CondOp::Ne => a != b,
            CondOp::Lt => a < b,
            CondOp::Le => a <= b,
            CondOp::Gt => a > b,
            CondOp::Ge => a >= b,
        }
    }
}

/// Parsea `REG OP VALOR` con espacios libres. Errores didácticos.
pub fn parse_cond(texto: &str) -> Result<CondAst, String> {
    let t = texto.trim();
    if t.is_empty() {
        return Err("la condición está vacía; el formato es REGISTRO OPERADOR VALOR (ej.: t0 == 5)".into());
    }
    // Separar en tres partes: reg, op, valor (el op puede venir pegado).
    let pos_op = t
        .find(|c| ['=', '!', '<', '>'].contains(&c))
        .ok_or_else(|| {
            format!(
                "falta el operador en «{t}»; valen == != < <= > >= (ej.: t0 == 5)"
            )
        })?;
    let (parte_reg, resto) = t.split_at(pos_op);
    let parte_reg = parte_reg.trim();
    let reg = reg_number(parte_reg).ok_or_else(|| {
        if parte_reg.is_empty() {
            "la condición empieza con el registro (ej.: t0 == 5)".to_string()
        } else {
            format!(
                "«{parte_reg}» no es un registro; valen los nombres ABI \
                 (t0–t6, s0–s11, a0–a7, sp, ra…) o x0–x31"
            )
        }
    })?;

    let (op, despues) = if let Some(r) = resto.strip_prefix("==") {
        (CondOp::Eq, r)
    } else if let Some(r) = resto.strip_prefix("!=") {
        (CondOp::Ne, r)
    } else if let Some(r) = resto.strip_prefix("<=") {
        (CondOp::Le, r)
    } else if let Some(r) = resto.strip_prefix(">=") {
        (CondOp::Ge, r)
    } else if let Some(r) = resto.strip_prefix('<') {
        if r.trim_start().starts_with(['<', '>', '=']) {
            return Err(format!(
                "operador inválido en «{resto}»; valen == != < <= > >="
            ));
        }
        (CondOp::Lt, r)
    } else if let Some(r) = resto.strip_prefix('>') {
        if r.trim_start().starts_with(['<', '>', '=']) {
            return Err(format!(
                "operador inválido en «{resto}»; valen == != < <= > >="
            ));
        }
        (CondOp::Gt, r)
    } else if resto.starts_with('=') {
        return Err("«=» solo no compara: usá «==» para igualdad".into());
    } else {
        return Err(format!(
            "operador inválido en «{resto}»; valen == != < <= > >="
        ));
    };

    let parte_val = despues.trim();
    if parte_val.is_empty() {
        return Err("falta el valor después del operador (número o registro)".into());
    }
    // ¿Basura al final? El valor es UNA palabra.
    if parte_val.split_whitespace().count() > 1 {
        return Err(format!(
            "sobra texto al final: «{parte_val}» — la condición es REGISTRO OPERADOR VALOR, sin más"
        ));
    }
    let val = if let Some(r) = reg_number(parte_val) {
        CondVal::Reg(r)
    } else {
        let v = parse_valor(parte_val).ok_or_else(|| {
            format!(
                "«{parte_val}» no es un número ni un registro; valen decimales \
                 (con signo), hex 0x… o un registro"
            )
        })?;
        CondVal::Imm(v)
    };
    Ok(CondAst { reg, op, val })
}

/// Decimal con signo o hex 0x… (los 32 bits del hex se toman como i32:
/// 0xFFFFFFFF es -1, coherente con la comparación signed).
fn parse_valor(t: &str) -> Option<i32> {
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(&h.replace('_', ""), 16)
            .ok()
            .map(|v| v as i32);
    }
    if let Some(h) = t.strip_prefix("-0x").or_else(|| t.strip_prefix("-0X")) {
        return u32::from_str_radix(&h.replace('_', ""), 16)
            .ok()
            .map(|v| (v as i32).wrapping_neg());
    }
    t.parse::<i32>().ok()
}
