//! The expression language as it was before #18: a recursive Pratt parser,
//! type check and evaluator over a tree of boxes, kept as the oracle the
//! flat, loop-based implementation in src/expr.rs is held to. It is
//! src/expr.rs at f462785 but for its module docs, its Serialize derives,
//! and cos/sin, taken from the crate (the same libm functions).

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Call(Func, Vec<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Min,
    Max,
    Abs,
    Round,
    Floor,
    Ceil,
    /// cosine of an angle in degrees — radial glyphs (gear teeth, wheel
    /// spokes, rosettes) are the demonstrated need
    Cos,
    /// sine of an angle in degrees
    Sin,
    /// cosine of an angle in radians — the radian-native glyph helpers
    /// (gear teeth, star points, rosettes) use these
    Cosr,
    /// sine of an angle in radians
    Sinr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Num,
    Bool,
}

/// An error with a byte offset into the expression source. The caller
/// maps offsets onto the KDL value's span for file:line diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at offset {})", self.message, self.offset)
    }
}

fn err<T>(offset: usize, message: impl Into<String>) -> Result<T, ExprError> {
    Err(ExprError {
        offset,
        message: message.into(),
    })
}

// --- tokenizer -----------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
    Comma,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Ne,
}

fn tokenize(src: &str) -> Result<Vec<(usize, Tok)>, ExprError> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            ' ' | '\t' => i += 1,
            '+' => {
                out.push((i, Tok::Plus));
                i += 1;
            }
            '-' => {
                out.push((i, Tok::Minus));
                i += 1;
            }
            '*' => {
                out.push((i, Tok::Star));
                i += 1;
            }
            '/' => {
                out.push((i, Tok::Slash));
                i += 1;
            }
            '%' => {
                out.push((i, Tok::Percent));
                i += 1;
            }
            '(' => {
                out.push((i, Tok::LParen));
                i += 1;
            }
            ')' => {
                out.push((i, Tok::RParen));
                i += 1;
            }
            ',' => {
                out.push((i, Tok::Comma));
                i += 1;
            }
            '<' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push((i, Tok::Le));
                    i += 2;
                } else {
                    out.push((i, Tok::Lt));
                    i += 1;
                }
            }
            '>' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push((i, Tok::Ge));
                    i += 2;
                } else {
                    out.push((i, Tok::Gt));
                    i += 1;
                }
            }
            '=' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push((i, Tok::EqEq));
                    i += 2;
                } else {
                    return err(i, "single '=' is not an operator; use '==' to compare (assignment does not exist in Isolex)");
                }
            }
            '!' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push((i, Tok::Ne));
                    i += 2;
                } else {
                    return err(i, "'!' is not an operator; use 'not' for boolean negation");
                }
            }
            '0'..='9' | '.' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                let text = &src[start..i];
                match text.parse::<f64>() {
                    Ok(n) => out.push((start, Tok::Num(n))),
                    Err(_) => return err(start, format!("malformed number '{text}'")),
                }
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                out.push((start, Tok::Ident(src[start..i].to_string())));
            }
            _ => {
                return err(
                    i,
                    format!("character '{c}' has no meaning in an Isolex expression"),
                )
            }
        }
    }
    Ok(out)
}

// --- Pratt parser --------------------------------------------------------

struct Parser<'a> {
    toks: &'a [(usize, Tok)],
    pos: usize,
    src_len: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&(usize, Tok)> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<(usize, Tok)> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, want: &Tok, what: &str) -> Result<(), ExprError> {
        match self.next() {
            Some((_, ref t)) if t == want => Ok(()),
            Some((o, t)) => err(o, format!("expected {what}, found {t:?}")),
            None => err(
                self.src_len,
                format!("expected {what}, found end of expression"),
            ),
        }
    }

    /// Precedence climbing over: or(1) < and(2) < not(3) < cmp(4) <
    /// add/sub(5) < mul/div/rem(6) < unary minus(7) < atoms.
    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ExprError> {
        let mut lhs = self.parse_prefix()?;
        loop {
            let (op_bp, build): (u8, fn(Expr, Expr) -> Expr) = match self.peek() {
                Some((_, Tok::Ident(w))) if w == "or" => (1, |a, b| Expr::Or(a.into(), b.into())),
                Some((_, Tok::Ident(w))) if w == "and" => (2, |a, b| Expr::And(a.into(), b.into())),
                Some((_, Tok::Lt)) => (4, |a, b| Expr::Cmp(CmpOp::Lt, a.into(), b.into())),
                Some((_, Tok::Le)) => (4, |a, b| Expr::Cmp(CmpOp::Le, a.into(), b.into())),
                Some((_, Tok::Gt)) => (4, |a, b| Expr::Cmp(CmpOp::Gt, a.into(), b.into())),
                Some((_, Tok::Ge)) => (4, |a, b| Expr::Cmp(CmpOp::Ge, a.into(), b.into())),
                Some((_, Tok::EqEq)) => (4, |a, b| Expr::Cmp(CmpOp::Eq, a.into(), b.into())),
                Some((_, Tok::Ne)) => (4, |a, b| Expr::Cmp(CmpOp::Ne, a.into(), b.into())),
                Some((_, Tok::Plus)) => (5, |a, b| Expr::Bin(BinOp::Add, a.into(), b.into())),
                Some((_, Tok::Minus)) => (5, |a, b| Expr::Bin(BinOp::Sub, a.into(), b.into())),
                Some((_, Tok::Star)) => (6, |a, b| Expr::Bin(BinOp::Mul, a.into(), b.into())),
                Some((_, Tok::Slash)) => (6, |a, b| Expr::Bin(BinOp::Div, a.into(), b.into())),
                Some((_, Tok::Percent)) => (6, |a, b| Expr::Bin(BinOp::Rem, a.into(), b.into())),
                _ => break,
            };
            if op_bp < min_bp {
                break;
            }
            self.pos += 1;
            let rhs = self.parse_expr(op_bp + 1)?;
            lhs = build(lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ExprError> {
        match self.next() {
            Some((_, Tok::Num(n))) => Ok(Expr::Num(n)),
            Some((_, Tok::Minus)) => {
                let inner = self.parse_expr(7)?;
                Ok(Expr::Neg(inner.into()))
            }
            Some((_, Tok::LParen)) => {
                let inner = self.parse_expr(0)?;
                self.expect(&Tok::RParen, "')'")?;
                Ok(inner)
            }
            Some((o, Tok::Ident(name))) => match name.as_str() {
                "not" => {
                    let inner = self.parse_expr(3)?;
                    Ok(Expr::Not(inner.into()))
                }
                "if" => {
                    self.expect(&Tok::LParen, "'(' after if")?;
                    let cond = self.parse_expr(0)?;
                    self.expect(&Tok::Comma, "',' after if condition")?;
                    let then = self.parse_expr(0)?;
                    self.expect(&Tok::Comma, "',' after if branch")?;
                    let alt = self.parse_expr(0)?;
                    self.expect(&Tok::RParen, "')' closing if")?;
                    Ok(Expr::If(cond.into(), then.into(), alt.into()))
                }
                "min" | "max" | "abs" | "round" | "floor" | "ceil" | "cos" | "sin" | "cosr"
                | "sinr" => {
                    let func = match name.as_str() {
                        "min" => Func::Min,
                        "max" => Func::Max,
                        "abs" => Func::Abs,
                        "round" => Func::Round,
                        "floor" => Func::Floor,
                        "ceil" => Func::Ceil,
                        "cos" => Func::Cos,
                        "sin" => Func::Sin,
                        "cosr" => Func::Cosr,
                        _ => Func::Sinr,
                    };
                    self.expect(&Tok::LParen, &format!("'(' after {name}"))?;
                    let mut args = vec![self.parse_expr(0)?];
                    while matches!(self.peek(), Some((_, Tok::Comma))) {
                        self.pos += 1;
                        args.push(self.parse_expr(0)?);
                    }
                    self.expect(&Tok::RParen, &format!("')' closing {name}"))?;
                    let want = match func {
                        Func::Min | Func::Max => 2,
                        _ => 1,
                    };
                    if args.len() != want {
                        return err(
                            o,
                            format!("{name} takes {want} argument(s), found {}", args.len()),
                        );
                    }
                    Ok(Expr::Call(func, args))
                }
                _ => {
                    if matches!(self.peek(), Some((_, Tok::LParen))) {
                        return err(
                            o,
                            format!(
                                "unknown function '{name}' (Isolex has min, max, abs, \
                                 round, floor, ceil, cos, sin, cosr, sinr, if — \
                                 user-defined functions do not exist)"
                            ),
                        );
                    }
                    Ok(Expr::Var(name))
                }
            },
            Some((o, t)) => err(o, format!("expected a value, found {t:?}")),
            None => err(self.src_len, "expected a value, found end of expression"),
        }
    }
}

// --- type check ----------------------------------------------------------

fn type_of(e: &Expr, src_len: usize) -> Result<Ty, ExprError> {
    let need = |e: &Expr, want: Ty, what: &str| -> Result<(), ExprError> {
        let got = type_of(e, src_len)?;
        if got != want {
            return err(
                src_len,
                format!("{what} must be {want:?}, but this sub-expression is {got:?}"),
            );
        }
        Ok(())
    };
    match e {
        Expr::Num(_) | Expr::Var(_) => Ok(Ty::Num),
        Expr::Neg(a) => {
            need(a, Ty::Num, "the operand of unary minus")?;
            Ok(Ty::Num)
        }
        Expr::Bin(_, a, b) => {
            need(a, Ty::Num, "an arithmetic operand")?;
            need(b, Ty::Num, "an arithmetic operand")?;
            Ok(Ty::Num)
        }
        Expr::Cmp(_, a, b) => {
            need(a, Ty::Num, "a comparison operand")?;
            need(b, Ty::Num, "a comparison operand")?;
            Ok(Ty::Bool)
        }
        Expr::And(a, b) | Expr::Or(a, b) => {
            need(a, Ty::Bool, "a boolean operand")?;
            need(b, Ty::Bool, "a boolean operand")?;
            Ok(Ty::Bool)
        }
        Expr::Not(a) => {
            need(a, Ty::Bool, "the operand of 'not'")?;
            Ok(Ty::Bool)
        }
        Expr::Call(_, args) => {
            for a in args {
                need(a, Ty::Num, "a function argument")?;
            }
            Ok(Ty::Num)
        }
        Expr::If(c, t, a) => {
            need(c, Ty::Bool, "the if condition")?;
            need(t, Ty::Num, "an if branch")?;
            need(a, Ty::Num, "an if branch")?;
            Ok(Ty::Num)
        }
    }
}

/// Parse a numeric expression (the common case: any numeric attribute).
pub fn parse_num(src: &str) -> Result<Expr, ExprError> {
    parse_typed(src, Ty::Num)
}

/// Parse a boolean expression (the root of a `when` guard).
pub fn parse_bool(src: &str) -> Result<Expr, ExprError> {
    parse_typed(src, Ty::Bool)
}

fn parse_typed(src: &str, want: Ty) -> Result<Expr, ExprError> {
    let toks = tokenize(src)?;
    if toks.is_empty() {
        return err(0, "empty expression");
    }
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        src_len: src.len(),
    };
    let e = p.parse_expr(0)?;
    if let Some((o, t)) = p.peek() {
        return err(*o, format!("unexpected {t:?} after a complete expression"));
    }
    let got = type_of(&e, src.len())?;
    if got != want {
        return err(
            0,
            format!(
                "this position requires a {want:?} expression, but the whole \
                 expression is {got:?}{}",
                if want == Ty::Num && got == Ty::Bool {
                    " (comparisons are only valid inside if(cond, a, b) or a when guard)"
                } else {
                    ""
                }
            ),
        );
    }
    Ok(e)
}

/// Evaluate with the given variable environment. BTreeMap so that any
/// iteration a future change introduces is deterministic by construction.
///
/// The value must be finite (SPEC §5): JSON has no infinity or NaN, so
/// one would reach the IR as null. Only the value counts; an overflow
/// along the way that comes back to a finite result (`1 / (a * a)`) is
/// not an error.
pub fn eval(e: &Expr, env: &BTreeMap<String, f64>) -> Result<f64, ExprError> {
    let v = value(e, env)?;
    if !v.is_finite() {
        return err(0, format!("the result is {v}, not a finite number"));
    }
    Ok(v)
}

fn value(e: &Expr, env: &BTreeMap<String, f64>) -> Result<f64, ExprError> {
    Ok(match e {
        Expr::Num(n) => *n,
        Expr::Var(name) => match env.get(name) {
            Some(v) => *v,
            None => {
                let mut names: Vec<&str> = env.keys().map(|s| s.as_str()).collect();
                names.sort_unstable();
                return err(
                    0,
                    format!(
                        "unknown name '{name}' (in scope here: {})",
                        if names.is_empty() {
                            "nothing".to_string()
                        } else {
                            names.join(", ")
                        }
                    ),
                );
            }
        },
        Expr::Neg(a) => -value(a, env)?,
        Expr::Bin(op, a, b) => {
            let (a, b) = (value(a, env)?, value(b, env)?);
            match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => {
                    if b == 0.0 {
                        return err(0, "division by zero");
                    }
                    a / b
                }
                BinOp::Rem => {
                    if b == 0.0 {
                        return err(0, "remainder by zero");
                    }
                    a % b
                }
            }
        }
        Expr::Cmp(op, a, b) => {
            let (a, b) = (value(a, env)?, value(b, env)?);
            let r = match op {
                CmpOp::Lt => a < b,
                CmpOp::Le => a <= b,
                CmpOp::Gt => a > b,
                CmpOp::Ge => a >= b,
                CmpOp::Eq => a == b,
                CmpOp::Ne => a != b,
            };
            bool_num(r)
        }
        Expr::And(a, b) => bool_num(num_bool(value(a, env)?) && num_bool(value(b, env)?)),
        Expr::Or(a, b) => bool_num(num_bool(value(a, env)?) || num_bool(value(b, env)?)),
        Expr::Not(a) => bool_num(!num_bool(value(a, env)?)),
        Expr::Call(f, args) => {
            let v: Vec<f64> = args
                .iter()
                .map(|a| value(a, env))
                .collect::<Result<_, _>>()?;
            match f {
                Func::Min => v[0].min(v[1]),
                Func::Max => v[0].max(v[1]),
                Func::Abs => v[0].abs(),
                Func::Round => v[0].round(),
                Func::Floor => v[0].floor(),
                Func::Ceil => v[0].ceil(),
                // cos/sin are the same bits everywhere (see below), then
                // quantized to 1e-9, which snaps results like cosr(pi/2)'s
                // 6e-17 to an exact 0; 1e-9 is far below the two-decimal
                // render resolution, so nothing visible changes.
                Func::Cos => quantize(cos(v[0].to_radians())),
                Func::Sin => quantize(sin(v[0].to_radians())),
                Func::Cosr => quantize(cos(v[0])),
                Func::Sinr => quantize(sin(v[0])),
            }
        }
        Expr::If(c, t, a) => {
            if num_bool(value(c, env)?) {
                value(t, env)?
            } else {
                value(a, env)?
            }
        }
    })
}

/// Collect the free variable names of an expression.
pub fn free_vars(e: &Expr, out: &mut std::collections::BTreeSet<String>) {
    match e {
        Expr::Num(_) => {}
        Expr::Var(name) => {
            out.insert(name.clone());
        }
        Expr::Neg(a) | Expr::Not(a) => free_vars(a, out),
        Expr::Bin(_, a, b) | Expr::Cmp(_, a, b) | Expr::And(a, b) | Expr::Or(a, b) => {
            free_vars(a, out);
            free_vars(b, out);
        }
        Expr::Call(_, args) => {
            for a in args {
                free_vars(a, out);
            }
        }
        Expr::If(c, t, alt) => {
            free_vars(c, out);
            free_vars(t, out);
            free_vars(alt, out);
        }
    }
}

/// Round a cos/sin result to 1e-9 (SPEC §7), so values that are zero or
/// one in exact arithmetic come out exactly zero or one.
pub fn quantize(v: f64) -> f64 {
    (v * 1e9).round() / 1e9
}

/// Cosine of radians, from libm's pure-Rust implementation rather than
/// the platform's maths library: platforms' own cos and sin may differ in
/// the last bit, and rounding afterwards cannot hide that when a result
/// lies right at a rounding boundary. The same code computes the same
/// bits everywhere (SPEC §7). Every other operation the compiler performs
/// is basic IEEE-754 arithmetic, which is already exact and portable.
pub fn cos(radians: f64) -> f64 {
    isolex::expr::cos(radians)
}

/// Sine of radians; see [`cos`].
pub fn sin(radians: f64) -> f64 {
    isolex::expr::sin(radians)
}

fn bool_num(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

fn num_bool(n: f64) -> bool {
    n != 0.0
}
