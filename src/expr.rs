//! The Isolex expression language: tokenizer, parser, type check, and
//! evaluator. Self-contained on purpose — SPEC.md §5 defines this
//! grammar without reference to any host language.
//!
//! Two types exist: `Num` (f64) and `Bool`. Comparisons and boolean
//! operators produce `Bool`, which is legal only as the condition of
//! `if(cond, a, b)` or as the root of a `when` guard. Everything else is
//! `Num`, and a numeric position requires a `Num` root. Evaluation is
//! total and pure: no user functions, no recursion, no assignment, no
//! I/O, no randomness.
//!
//! Nothing here recurses (#18). An expression is a flat array of nodes,
//! each after its operands, so cloning, dropping, comparing or
//! serializing one is a loop; the parser, type check and evaluator keep
//! their work on explicit stacks in the heap. How deeply an expression
//! nests costs memory, never the call stack, which leaves
//! `expression-depth` (SPEC §9) a limit an author can raise or switch
//! off rather than a guard against a crash.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

/// An expression: its nodes, each after its operands, the root last.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Expr {
    nodes: Vec<ExprNode>,
    /// How deeply it nests, worked out once when it is parsed.
    #[serde(skip)]
    depth: usize,
}

/// One node of an [`Expr`]; its operands are indices of earlier nodes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ExprNode {
    Num(f64),
    Var(String),
    Neg(usize),
    Bin(BinOp, usize, usize),
    Cmp(CmpOp, usize, usize),
    And(usize, usize),
    Or(usize, usize),
    Not(usize),
    Call(Func, Vec<usize>),
    If(usize, usize, usize),
}

impl ExprNode {
    fn operands(&self) -> Vec<usize> {
        match self {
            ExprNode::Num(_) | ExprNode::Var(_) => Vec::new(),
            ExprNode::Neg(a) | ExprNode::Not(a) => vec![*a],
            ExprNode::Bin(_, a, b)
            | ExprNode::Cmp(_, a, b)
            | ExprNode::And(a, b)
            | ExprNode::Or(a, b) => vec![*a, *b],
            ExprNode::Call(_, args) => args.clone(),
            ExprNode::If(c, t, e) => vec![*c, *t, *e],
        }
    }
}

impl Expr {
    fn push(&mut self, node: ExprNode) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn root(&self) -> usize {
        self.nodes.len() - 1
    }

    /// How deeply it nests (SPEC §9, `expression-depth`): a number or a
    /// name is 1, and an operator or call is one more than its deepest
    /// operand. Parentheses add nothing.
    pub fn depth(&self) -> usize {
        self.depth
    }

    fn measure_depth(&self) -> usize {
        let mut depth: Vec<usize> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let deepest = node.operands().iter().map(|&o| depth[o]).max();
            depth.push(1 + deepest.unwrap_or(0));
        }
        depth.last().copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

// --- parser --------------------------------------------------------------

/// A binary operator and how tightly it binds: or(1) < and(2) < cmp(4) <
/// add/sub(5) < mul/div/rem(6). Unary minus binds tighter than all of them
/// (7); `not` takes everything from comparison up (3).
#[derive(Clone, Copy)]
enum Infix {
    Or,
    And,
    Cmp(CmpOp),
    Bin(BinOp),
}

fn infix(tok: &Tok) -> Option<(Infix, u8)> {
    Some(match tok {
        Tok::Ident(w) if w == "or" => (Infix::Or, 1),
        Tok::Ident(w) if w == "and" => (Infix::And, 2),
        Tok::Lt => (Infix::Cmp(CmpOp::Lt), 4),
        Tok::Le => (Infix::Cmp(CmpOp::Le), 4),
        Tok::Gt => (Infix::Cmp(CmpOp::Gt), 4),
        Tok::Ge => (Infix::Cmp(CmpOp::Ge), 4),
        Tok::EqEq => (Infix::Cmp(CmpOp::Eq), 4),
        Tok::Ne => (Infix::Cmp(CmpOp::Ne), 4),
        Tok::Plus => (Infix::Bin(BinOp::Add), 5),
        Tok::Minus => (Infix::Bin(BinOp::Sub), 5),
        Tok::Star => (Infix::Bin(BinOp::Mul), 6),
        Tok::Slash => (Infix::Bin(BinOp::Div), 6),
        Tok::Percent => (Infix::Bin(BinOp::Rem), 6),
        _ => return None,
    })
}

fn func(name: &str) -> Option<Func> {
    Some(match name {
        "min" => Func::Min,
        "max" => Func::Max,
        "abs" => Func::Abs,
        "round" => Func::Round,
        "floor" => Func::Floor,
        "ceil" => Func::Ceil,
        "cos" => Func::Cos,
        "sin" => Func::Sin,
        "cosr" => Func::Cosr,
        "sinr" => Func::Sinr,
        _ => return None,
    })
}

/// An operator or parenthesis waiting for what comes after it.
enum Pending {
    /// unary minus or `not`: its operand is everything binding at least
    /// `rhs_min`
    Prefix { neg: bool, rhs_min: u8 },
    /// a binary operator, its left operand already parsed
    Infix { op: Infix, rhs_min: u8 },
    /// an open parenthesis, with the commas seen inside it
    Paren { kind: Paren, commas: usize },
}

enum Paren {
    Group,
    /// a function's argument list; `at` is where its name is
    Call {
        func: Func,
        name: String,
        at: usize,
    },
    If,
}

impl Paren {
    /// What a parenthesis expects next after a complete operand, for the
    /// error when something else comes.
    fn expecting(&self, commas: usize) -> String {
        match self {
            Paren::Group => "')'".to_string(),
            Paren::Call { name, .. } => format!("')' closing {name}"),
            Paren::If => match commas {
                0 => "',' after if condition".to_string(),
                1 => "',' after if branch".to_string(),
                _ => "')' closing if".to_string(),
            },
        }
    }
}

/// Build every pending operator that binds at least as tightly as `bp`
/// asks: an operator whose operand is complete once something binding
/// less than its `rhs_min` arrives. Stops at a parenthesis.
fn reduce(e: &mut Expr, operands: &mut Vec<usize>, pending: &mut Vec<Pending>, bp: u8) {
    while let Some(top) = pending.last() {
        let node = match *top {
            Pending::Prefix { neg, rhs_min } if bp < rhs_min => {
                let a = operands.pop().expect("a prefix has its operand");
                if neg {
                    ExprNode::Neg(a)
                } else {
                    ExprNode::Not(a)
                }
            }
            Pending::Infix { op, rhs_min } if bp < rhs_min => {
                let b = operands.pop().expect("an infix has its right operand");
                let a = operands.pop().expect("an infix has its left operand");
                match op {
                    Infix::Or => ExprNode::Or(a, b),
                    Infix::And => ExprNode::And(a, b),
                    Infix::Cmp(c) => ExprNode::Cmp(c, a, b),
                    Infix::Bin(o) => ExprNode::Bin(o, a, b),
                }
            }
            _ => break,
        };
        pending.pop();
        operands.push(e.push(node));
    }
}

/// Parse an expression with operator and operand stacks instead of
/// recursion (shunting-yard): the same grammar, precedence,
/// associativity and errors as SPEC §5's recursive description, at any
/// depth.
fn parse(src: &str, toks: &[(usize, Tok)]) -> Result<Expr, ExprError> {
    let end = src.len();
    let mut e = Expr {
        nodes: Vec::new(),
        depth: 0,
    };
    let mut operands: Vec<usize> = Vec::new();
    let mut pending: Vec<Pending> = Vec::new();
    // an operand is wanted next, rather than an operator
    let mut want_operand = true;
    let found = |i: usize| match toks.get(i) {
        Some((_, t)) => format!("{t:?}"),
        None => "end of expression".to_string(),
    };
    let found_at = |i: usize| toks.get(i).map(|(o, _)| *o).unwrap_or(end);

    let mut i = 0;
    while i < toks.len() {
        let (o, tok) = (&toks[i].0, &toks[i].1);
        let o = *o;
        if want_operand {
            match tok {
                Tok::Num(n) => {
                    operands.push(e.push(ExprNode::Num(*n)));
                    want_operand = false;
                }
                Tok::Minus => pending.push(Pending::Prefix {
                    neg: true,
                    rhs_min: 7,
                }),
                Tok::LParen => pending.push(Pending::Paren {
                    kind: Paren::Group,
                    commas: 0,
                }),
                Tok::Ident(w) if w == "not" => pending.push(Pending::Prefix {
                    neg: false,
                    rhs_min: 3,
                }),
                Tok::Ident(w) if w == "if" || func(w).is_some() => {
                    if !matches!(toks.get(i + 1), Some((_, Tok::LParen))) {
                        return err(
                            found_at(i + 1),
                            format!("expected '(' after {w}, found {}", found(i + 1)),
                        );
                    }
                    i += 1;
                    let kind = match func(w) {
                        Some(f) => Paren::Call {
                            func: f,
                            name: w.clone(),
                            at: o,
                        },
                        None => Paren::If,
                    };
                    pending.push(Pending::Paren { kind, commas: 0 });
                }
                Tok::Ident(w) => {
                    if matches!(toks.get(i + 1), Some((_, Tok::LParen))) {
                        return err(
                            o,
                            format!(
                                "unknown function '{w}' (Isolex has min, max, abs, \
                                 round, floor, ceil, cos, sin, cosr, sinr, if — \
                                 user-defined functions do not exist)"
                            ),
                        );
                    }
                    operands.push(e.push(ExprNode::Var(w.clone())));
                    want_operand = false;
                }
                t => return err(o, format!("expected a value, found {t:?}")),
            }
        } else if let Some((op, bp)) = infix(tok) {
            reduce(&mut e, &mut operands, &mut pending, bp);
            pending.push(Pending::Infix {
                op,
                rhs_min: bp + 1,
            });
            want_operand = true;
        } else {
            reduce(&mut e, &mut operands, &mut pending, 0);
            match (tok, pending.last_mut()) {
                // a comma separates a call's or an if's arguments; an if
                // takes exactly three
                (
                    Tok::Comma,
                    Some(Pending::Paren {
                        kind: kind @ (Paren::Call { .. } | Paren::If),
                        commas,
                    }),
                ) if !(matches!(kind, Paren::If) && *commas == 2) => {
                    *commas += 1;
                    want_operand = true;
                }
                (Tok::RParen, Some(Pending::Paren { .. })) => {
                    let Some(Pending::Paren { kind, commas }) = pending.pop() else {
                        unreachable!("just matched")
                    };
                    match kind {
                        Paren::Group => {}
                        Paren::Call { func, name, at } => {
                            let n = commas + 1;
                            let want = match func {
                                Func::Min | Func::Max => 2,
                                _ => 1,
                            };
                            if n != want {
                                return err(
                                    at,
                                    format!("{name} takes {want} argument(s), found {n}"),
                                );
                            }
                            let args = operands.split_off(operands.len() - n);
                            operands.push(e.push(ExprNode::Call(func, args)));
                        }
                        Paren::If => {
                            if commas < 2 {
                                return err(
                                    o,
                                    format!(
                                        "expected {}, found {}",
                                        Paren::If.expecting(commas),
                                        found(i)
                                    ),
                                );
                            }
                            let alt = operands.pop().expect("if has three");
                            let then = operands.pop().expect("if has three");
                            let cond = operands.pop().expect("if has three");
                            operands.push(e.push(ExprNode::If(cond, then, alt)));
                        }
                    }
                }
                (_, Some(Pending::Paren { kind, commas })) => {
                    return err(
                        o,
                        format!("expected {}, found {}", kind.expecting(*commas), found(i)),
                    );
                }
                _ => {
                    return err(o, format!("unexpected {tok:?} after a complete expression"));
                }
            }
        }
        i += 1;
    }

    if want_operand {
        return err(end, "expected a value, found end of expression");
    }
    reduce(&mut e, &mut operands, &mut pending, 0);
    if let Some(Pending::Paren { kind, commas }) = pending.last() {
        return err(
            end,
            format!(
                "expected {}, found end of expression",
                kind.expecting(*commas)
            ),
        );
    }
    debug_assert_eq!(operands, vec![e.root()], "one expression, its root last");
    e.depth = e.measure_depth();
    Ok(e)
}

// --- type check ----------------------------------------------------------

/// What operand `k` of `node` must be, and what to call it if it is not.
fn operand_must_be(node: &ExprNode, k: usize) -> (Ty, &'static str) {
    match node {
        ExprNode::Neg(_) => (Ty::Num, "the operand of unary minus"),
        ExprNode::Bin(..) => (Ty::Num, "an arithmetic operand"),
        ExprNode::Cmp(..) => (Ty::Num, "a comparison operand"),
        ExprNode::And(..) | ExprNode::Or(..) => (Ty::Bool, "a boolean operand"),
        ExprNode::Not(_) => (Ty::Bool, "the operand of 'not'"),
        ExprNode::Call(..) => (Ty::Num, "a function argument"),
        ExprNode::If(..) if k == 0 => (Ty::Bool, "the if condition"),
        ExprNode::If(..) => (Ty::Num, "an if branch"),
        ExprNode::Num(_) | ExprNode::Var(_) => unreachable!("no operands"),
    }
}

fn type_of_node(node: &ExprNode) -> Ty {
    match node {
        ExprNode::Cmp(..) | ExprNode::And(..) | ExprNode::Or(..) | ExprNode::Not(_) => Ty::Bool,
        _ => Ty::Num,
    }
}

/// The root's type, checking every operand on the way, depth first and
/// left to right with an explicit stack, so the first error reported is
/// the one a recursive check would find first.
fn type_of(e: &Expr, src_len: usize) -> Result<Ty, ExprError> {
    let mut checked: Vec<bool> = vec![false; e.nodes.len()];
    // (node, its next operand to check)
    let mut stack: Vec<(usize, usize)> = vec![(e.root(), 0)];
    while let Some(&(n, k)) = stack.last() {
        let operands = e.nodes[n].operands();
        match operands.get(k) {
            Some(&o) if !checked[o] => stack.push((o, 0)),
            Some(&o) => {
                let (want, what) = operand_must_be(&e.nodes[n], k);
                let got = type_of_node(&e.nodes[o]);
                if got != want {
                    return err(
                        src_len,
                        format!("{what} must be {want:?}, but this sub-expression is {got:?}"),
                    );
                }
                stack.last_mut().expect("on the stack").1 += 1;
            }
            None => {
                checked[n] = true;
                stack.pop();
            }
        }
    }
    Ok(type_of_node(&e.nodes[e.root()]))
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
    let e = parse(src, &toks)?;
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

// --- evaluation ----------------------------------------------------------

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

/// A step of evaluation: a node to evaluate, or what to do with the
/// values its operands left on the value stack.
#[derive(Clone, Copy)]
enum Step {
    Eval(usize),
    Apply(usize),
    /// `and`/`or`: the left operand is on the value stack; the right one
    /// is evaluated only if it can still change the answer
    And(usize),
    Or(usize),
    /// turn the value on top into a boolean 0 or 1
    Truth,
    /// an if's condition is on the value stack: evaluate one branch
    Choose(usize, usize),
}

/// Evaluate with a stack machine. Operands go left to right, so the
/// first error is the one a recursive evaluation would hit first; `if`
/// evaluates only the branch it takes, and `and`/`or` stop once the
/// answer is known, so `if(d == 0, 0, 1 / d)` never divides by zero.
fn value(e: &Expr, env: &BTreeMap<String, f64>) -> Result<f64, ExprError> {
    let mut steps = vec![Step::Eval(e.root())];
    let mut values: Vec<f64> = Vec::new();
    while let Some(step) = steps.pop() {
        match step {
            Step::Eval(n) => match &e.nodes[n] {
                ExprNode::Num(v) => values.push(*v),
                ExprNode::Var(name) => match env.get(name) {
                    Some(v) => values.push(*v),
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
                ExprNode::And(a, b) => {
                    steps.push(Step::And(*b));
                    steps.push(Step::Eval(*a));
                }
                ExprNode::Or(a, b) => {
                    steps.push(Step::Or(*b));
                    steps.push(Step::Eval(*a));
                }
                ExprNode::If(c, t, alt) => {
                    steps.push(Step::Choose(*t, *alt));
                    steps.push(Step::Eval(*c));
                }
                node => {
                    steps.push(Step::Apply(n));
                    for o in node.operands().into_iter().rev() {
                        steps.push(Step::Eval(o));
                    }
                }
            },
            Step::Apply(n) => {
                let node = &e.nodes[n];
                let args = values.split_off(values.len() - node.operands().len());
                values.push(match node {
                    ExprNode::Neg(_) => -args[0],
                    ExprNode::Not(_) => bool_num(!num_bool(args[0])),
                    ExprNode::Bin(op, ..) => {
                        let (a, b) = (args[0], args[1]);
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
                    ExprNode::Cmp(op, ..) => {
                        let (a, b) = (args[0], args[1]);
                        bool_num(match op {
                            CmpOp::Lt => a < b,
                            CmpOp::Le => a <= b,
                            CmpOp::Gt => a > b,
                            CmpOp::Ge => a >= b,
                            CmpOp::Eq => a == b,
                            CmpOp::Ne => a != b,
                        })
                    }
                    ExprNode::Call(f, _) => match f {
                        Func::Min => args[0].min(args[1]),
                        Func::Max => args[0].max(args[1]),
                        Func::Abs => args[0].abs(),
                        Func::Round => args[0].round(),
                        Func::Floor => args[0].floor(),
                        Func::Ceil => args[0].ceil(),
                        // cos/sin are the same bits everywhere (see below),
                        // then quantized to 1e-9, which snaps results like
                        // cosr(pi/2)'s 6e-17 to an exact 0; 1e-9 is far
                        // below the two-decimal render resolution, so
                        // nothing visible changes.
                        Func::Cos => quantize(cos(args[0].to_radians())),
                        Func::Sin => quantize(sin(args[0].to_radians())),
                        Func::Cosr => quantize(cos(args[0])),
                        Func::Sinr => quantize(sin(args[0])),
                    },
                    _ => unreachable!("evaluated directly, or by And, Or or Choose"),
                });
            }
            Step::And(b) | Step::Or(b) => {
                let a = num_bool(values.pop().expect("the left operand"));
                // `and` with a false left, or `or` with a true one, is decided
                if a == matches!(step, Step::Or(_)) {
                    values.push(bool_num(a));
                } else {
                    steps.push(Step::Truth);
                    steps.push(Step::Eval(b));
                }
            }
            Step::Truth => {
                let v = values.pop().expect("a value");
                values.push(bool_num(num_bool(v)));
            }
            Step::Choose(t, alt) => {
                let c = num_bool(values.pop().expect("the condition"));
                steps.push(Step::Eval(if c { t } else { alt }));
            }
        }
    }
    Ok(values.pop().expect("one value"))
}

/// Collect the free variable names of an expression.
pub fn free_vars(e: &Expr, out: &mut std::collections::BTreeSet<String>) {
    for node in &e.nodes {
        if let ExprNode::Var(name) = node {
            out.insert(name.clone());
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
    libm::cos(radians)
}

/// Sine of radians; see [`cos`].
pub fn sin(radians: f64) -> f64 {
    libm::sin(radians)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn num(src: &str, e: &[(&str, f64)]) -> f64 {
        eval(&parse_num(src).unwrap(), &env(e)).unwrap()
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(num("1 + 2 * 3", &[]), 7.0);
        assert_eq!(num("(1 + 2) * 3", &[]), 9.0);
        assert_eq!(num("10 / 4", &[]), 2.5);
        assert_eq!(num("7 % 3", &[]), 1.0);
        assert_eq!(num("-2 * 3", &[]), -6.0);
        assert_eq!(num("2 - -3", &[]), 5.0);
    }

    #[test]
    fn variables_and_the_port_row_shape() {
        // the bread-and-butter expression from the existing pack
        assert_eq!(num("0.1 + i * 0.155", &[("i", 3.0)]), 0.565);
        assert_eq!(num("u * 0.28", &[("u", 2.0)]), 0.56);
    }

    #[test]
    fn functions() {
        assert_eq!(num("min(2, 3)", &[]), 2.0);
        assert_eq!(num("max(2, 3)", &[]), 3.0);
        assert_eq!(num("abs(-4)", &[]), 4.0);
        assert_eq!(num("round(2.5)", &[]), 3.0); // f64::round: half away from zero
        assert_eq!(num("floor(2.9)", &[]), 2.0);
        assert_eq!(num("ceil(2.1)", &[]), 3.0);
        assert_eq!(num("cos(0)", &[]), 1.0);
        assert!((num("sin(90)", &[]) - 1.0).abs() < 1e-12);
        assert!((num("cos(60)", &[]) - 0.5).abs() < 1e-12);
        // cosr/sinr are radian-native: cosr(pi) == -1, where the degree
        // cos(pi) would be ~+1.
        assert_eq!(num("cosr(0)", &[]), 1.0);
        assert!((num("sinr(1.5707963267948966)", &[]) - 1.0).abs() < 1e-12);
        assert!((num("cosr(3.141592653589793)", &[]) + 1.0).abs() < 1e-12);
        // Transcendentals are quantized to 1e-9 (SPEC §7), so a value that
        // is ~6e-17 in raw IEEE lands on exactly 0.0 — the property that
        // makes the shape-IR byte-identical across platforms. If quantize
        // were dropped these would be tiny nonzero and this would fail.
        assert_eq!(num("cosr(1.5707963267948966)", &[]), 0.0);
        assert_eq!(num("sinr(3.141592653589793)", &[]), 0.0);
    }

    #[test]
    fn if_with_boolean_condition() {
        assert_eq!(num("if(i % 2 == 0, 0.35, 0.12)", &[("i", 4.0)]), 0.35);
        assert_eq!(num("if(i % 2 == 0, 0.35, 0.12)", &[("i", 5.0)]), 0.12);
        assert_eq!(
            num("if(a < 1 and b > 1, 10, 20)", &[("a", 0.0), ("b", 2.0)]),
            10.0
        );
        assert_eq!(num("if(not (a == 0), 1, 2)", &[("a", 0.0)]), 2.0);
    }

    #[test]
    fn when_guards_require_bool_roots() {
        assert!(parse_bool("i < 4").is_ok());
        assert!(parse_bool("i < 4 or i == 9").is_ok());
        let e = parse_bool("i + 1").unwrap_err();
        assert!(e.message.contains("requires a Bool"), "{e:?}");
    }

    #[test]
    fn numeric_positions_reject_bare_booleans() {
        let e = parse_num("i < 4").unwrap_err();
        assert!(e.message.contains("only valid inside if"), "{e:?}");
        let e = parse_num("if(1 + 1, 2, 3)").unwrap_err();
        assert!(e.message.contains("if condition"), "{e:?}");
    }

    #[test]
    fn rejections_carry_offsets_and_name_the_sin() {
        let e = parse_num("x = 4").unwrap_err();
        assert_eq!(e.offset, 2);
        assert!(e.message.contains("assignment does not exist"), "{e:?}");

        let e = parse_num("foo(3)").unwrap_err();
        assert!(e.message.contains("unknown function 'foo'"), "{e:?}");

        let e = parse_num("2 ** 3").unwrap_err();
        assert!(e.message.contains("expected a value"), "{e:?}");

        let e = parse_num("x @ 2").unwrap_err();
        assert!(e.message.contains("no meaning"), "{e:?}");

        let e = parse_num("min(1)").unwrap_err();
        assert!(e.message.contains("takes 2 argument(s)"), "{e:?}");

        let e = parse_num("1 2").unwrap_err();
        assert!(e.message.contains("after a complete expression"), "{e:?}");

        let e = parse_num("").unwrap_err();
        assert!(e.message.contains("empty expression"), "{e:?}");
    }

    #[test]
    fn eval_failures_are_diagnostic() {
        let e = eval(&parse_num("1 / z").unwrap(), &env(&[("z", 0.0)])).unwrap_err();
        assert!(e.message.contains("division by zero"), "{e:?}");

        let e = eval(&parse_num("ghost + 1").unwrap(), &env(&[("i", 1.0)])).unwrap_err();
        assert!(e.message.contains("unknown name 'ghost'"), "{e:?}");
        assert!(e.message.contains("in scope here: i"), "{e:?}");
    }

    #[test]
    fn a_value_that_is_not_finite_is_an_error() {
        let big = env(&[("a", 1e300)]);
        let e = eval(&parse_num("a * a").unwrap(), &big).unwrap_err();
        assert_eq!(e.message, "the result is inf, not a finite number");
        let e = eval(&parse_num("0 - a * a").unwrap(), &big).unwrap_err();
        assert_eq!(e.message, "the result is -inf, not a finite number");
        let e = eval(&parse_num("a * a - a * a").unwrap(), &big).unwrap_err();
        assert_eq!(e.message, "the result is NaN, not a finite number");
        // Only the value counts: an overflow along the way that comes back
        // to a finite result is not one.
        assert_eq!(eval(&parse_num("1 / (a * a)").unwrap(), &big), Ok(0.0));
        assert_eq!(eval(&parse_num("min(a * a, 2)").unwrap(), &big), Ok(2.0));
    }

    #[test]
    fn cos_and_sin_are_the_same_bits_everywhere() {
        // Sweeping 20 million arguments, the platforms' own cos and sin
        // differed from libm's in 1 083 956 of 40 million results with
        // glibc and 580 547 with Windows' UCRT, and from each other: glibc
        // and UCRT disagree on the sine of `a`. Both differ from libm on
        // the cosine of `b`. These are libm's values, on every platform.
        let a = f64::from_bits(4_686_651_910_829_642_665); // 215714.62251296386
        assert_eq!(sin(a).to_bits(), 0.299_860_931_826_934_63_f64.to_bits());
        let b = f64::from_bits(13_840_824_672_179_564_243); // -5.121784682949243
        assert_eq!(cos(b).to_bits(), 0.398_055_041_475_774_3_f64.to_bits());

        // And the hard cases for argument reduction (K. C. Ng, "Argument
        // Reduction for Huge Arguments"): correctly rounded.
        assert_eq!(cos(1e22).to_bits(), 0.523_214_785_395_139_f64.to_bits());
        assert_eq!(
            sin(1e22).to_bits(),
            (-0.852_200_849_767_188_8_f64).to_bits()
        );
        // and through the language, where the 1e-9 rounding follows
        let e = env(&[("big", 1e22)]);
        assert_eq!(num("cosr(big)", &[("big", 1e22)]), 0.523214785);
        assert_eq!(eval(&parse_num("sinr(big)").unwrap(), &e), Ok(-0.85220085));
    }

    #[test]
    fn round_takes_halves_away_from_zero() {
        // SPEC §5: the opposite of the output formatting's half-to-even
        assert_eq!(num("round(2.5)", &[]), 3.0);
        assert_eq!(num("round(-2.5)", &[]), -3.0);
        assert_eq!(num("round(0.5)", &[]), 1.0);
        assert_eq!(num("round(-0.5)", &[]), -1.0);
    }
}
