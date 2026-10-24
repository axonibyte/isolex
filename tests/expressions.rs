//! The expression language without recursion (#18): held, by differential
//! testing, to the recursive implementation it replaced, and shown to
//! take nesting of any depth.

#[path = "reference/expr.rs"]
#[allow(dead_code)]
mod reference;

use std::collections::BTreeMap;

use isolex::expr::{eval, parse_bool, parse_num, Expr, ExprError};

/// xorshift64*, as the battery uses: deterministic, no dependencies.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len())]
    }
}

const NUMS: &[&str] = &["0", "1", "2.5", "0.5", "3", "10", "1.25"];
// zz is never bound, so its errors are compared too
const NAMES: &[&str] = &["a", "b", "i", "d", "u", "zz"];
const UNARY: &[&str] = &[
    "abs", "round", "floor", "ceil", "cos", "sin", "cosr", "sinr",
];
const ARITH: &[&str] = &["+", "-", "*", "/", "%"];
const CMP: &[&str] = &["<", "<=", ">", ">=", "==", "!="];

/// A numeric expression as tokens separated by spaces, mostly well
/// typed, sometimes not.
fn num(rng: &mut Rng, depth: usize, out: &mut Vec<String>) {
    let leaf = depth == 0 || rng.below(4) == 0;
    match if leaf { rng.below(2) } else { 2 + rng.below(7) } {
        0 => out.push(rng.pick(NUMS).to_string()),
        1 => out.push(rng.pick(NAMES).to_string()),
        2 | 3 => {
            num(rng, depth - 1, out);
            out.push(rng.pick(ARITH).to_string());
            num(rng, depth - 1, out);
        }
        4 => {
            out.push("-".into());
            num(rng, depth - 1, out);
        }
        5 => {
            out.push("(".into());
            num(rng, depth - 1, out);
            out.push(")".into());
        }
        6 => {
            out.push(rng.pick(UNARY).into());
            out.push("(".into());
            num(rng, depth - 1, out);
            out.push(")".into());
        }
        7 => {
            out.push(rng.pick(&["min", "max"]).into());
            out.push("(".into());
            num(rng, depth - 1, out);
            out.push(",".into());
            num(rng, depth - 1, out);
            out.push(")".into());
        }
        _ => {
            out.push("if".into());
            out.push("(".into());
            boolean(rng, depth - 1, out);
            out.push(",".into());
            num(rng, depth - 1, out);
            out.push(",".into());
            num(rng, depth - 1, out);
            out.push(")".into());
        }
    }
}

fn boolean(rng: &mut Rng, depth: usize, out: &mut Vec<String>) {
    match if depth == 0 { 0 } else { rng.below(6) } {
        0 | 1 => {
            num(rng, depth.saturating_sub(1), out);
            out.push(rng.pick(CMP).into());
            num(rng, depth.saturating_sub(1), out);
        }
        2 | 3 => {
            boolean(rng, depth - 1, out);
            out.push(rng.pick(&["and", "or"]).into());
            boolean(rng, depth - 1, out);
        }
        4 => {
            out.push("not".into());
            boolean(rng, depth - 1, out);
        }
        _ => {
            // a type error now and then: a number where a boolean belongs
            num(rng, depth - 1, out);
        }
    }
}

/// Mangle a token list: delete, duplicate, insert or swap.
fn mangle(rng: &mut Rng, toks: &mut Vec<String>) {
    const ANY: &[&str] = &[
        "(", ")", ",", "-", "+", "*", "not", "and", "or", "if", "min", "abs", "a", "1", "==", "<",
        "foo", "=", "!",
    ];
    for _ in 0..1 + rng.below(3) {
        let n = toks.len();
        match rng.below(4) {
            0 if n > 1 => {
                toks.remove(rng.below(n));
            }
            1 if n > 0 => {
                let t = toks[rng.below(n)].clone();
                toks.insert(rng.below(n + 1), t);
            }
            2 => {
                let t = rng.pick(ANY).to_string();
                toks.insert(rng.below(n + 1), t);
            }
            _ if n > 1 => {
                let k = rng.below(n - 1);
                toks.swap(k, k + 1);
            }
            _ => {}
        }
    }
}

fn envs() -> Vec<BTreeMap<String, f64>> {
    let bind = |pairs: &[(&str, f64)]| -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    };
    vec![
        bind(&[("a", 1.0), ("b", 0.0), ("i", 3.0), ("d", 0.0), ("u", 2.5)]),
        bind(&[("a", -2.0), ("b", 7.5), ("i", 0.0), ("d", 4.0), ("u", 0.25)]),
        bind(&[
            ("a", 1e300),
            ("b", 1e-300),
            ("i", 2.0),
            ("d", -1.0),
            ("u", 1.0),
        ]),
        bind(&[]),
    ]
}

/// What a parse and its evaluations come to, comparably: errors as
/// (offset, message), values as bits.
fn outcome(
    parsed: Result<Vec<Result<f64, String>>, ExprError>,
) -> Result<Vec<Result<u64, String>>, (usize, String)> {
    match parsed {
        Err(e) => Err((e.offset, e.message)),
        Ok(vs) => Ok(vs.into_iter().map(|v| v.map(|x| x.to_bits())).collect()),
    }
}

fn new_outcome(src: &str, boolean: bool) -> Result<Vec<Result<u64, String>>, (usize, String)> {
    let parsed: Result<Expr, ExprError> = if boolean {
        parse_bool(src)
    } else {
        parse_num(src)
    };
    outcome(parsed.map(|e| {
        envs()
            .iter()
            .map(|env| eval(&e, env).map_err(|x| x.message))
            .collect()
    }))
}

fn old_outcome(src: &str, boolean: bool) -> Result<Vec<Result<u64, String>>, (usize, String)> {
    let parsed = if boolean {
        reference::parse_bool(src)
    } else {
        reference::parse_num(src)
    };
    let parsed = parsed.map_err(|e| ExprError {
        offset: e.offset,
        message: e.message,
    });
    outcome(parsed.map(|e| {
        envs()
            .iter()
            .map(|env| reference::eval(&e, env).map_err(|x| x.message))
            .collect()
    }))
}

#[test]
fn agrees_with_the_recursive_implementation() {
    let mut rng = Rng(0x1505_1E5C_0FFE_E001);
    let (mut sources, mut valid, mut invalid) = (0, 0, 0);
    for case in 0..40_000 {
        let mut toks = Vec::new();
        let depth = 1 + rng.below(5);
        if case % 2 == 0 {
            num(&mut rng, depth, &mut toks);
        } else {
            boolean(&mut rng, depth, &mut toks);
        }
        if rng.below(3) == 0 {
            mangle(&mut rng, &mut toks);
        }
        let src = toks.join(if rng.below(4) == 0 { "" } else { " " });
        for boolean in [false, true] {
            let new = new_outcome(&src, boolean);
            let old = old_outcome(&src, boolean);
            assert_eq!(
                new,
                old,
                "parse_{} of {src:?}: new (left) and recursive (right) disagree",
                if boolean { "bool" } else { "num" }
            );
            if new.is_ok() {
                valid += 1;
            } else {
                invalid += 1;
            }
        }
        sources += 1;
    }
    // the comparison means something only if both kinds were plentiful
    assert!(
        valid > 10_000 && invalid > 10_000,
        "{sources} sources: {valid} valid, {invalid} not"
    );
}

/// The hand-written cases a generator might not reach, each an error the
/// recursive parser produced for a reason worth keeping.
#[test]
fn agrees_on_every_kind_of_malformed_input() {
    for src in [
        "",
        " ",
        "(",
        ")",
        "()",
        "(1",
        "1)",
        "(1 2)",
        "1 2",
        "min",
        "min(",
        "min(1",
        "min(1,",
        "min(1 2)",
        "min(1)",
        "min(1,2,3)",
        "abs(1,2)",
        "abs()",
        "if",
        "if(",
        "if(1 < 2",
        "if(1 < 2,",
        "if(1 < 2, 3",
        "if(1 < 2, 3,",
        "if(1 < 2, 3, 4",
        "if(1 < 2, 3, 4, 5)",
        "if(1 < 2)",
        "if(1 < 2, 3)",
        "if 1",
        "min 1",
        "foo(1)",
        "or(1)",
        "a b",
        "a not",
        "not",
        "-",
        "1 +",
        "1 + * 2",
        "(1,2)",
        "1, 2",
        "x = 1",
        "x ! 1",
        "1 @ 2",
        "1..2",
        "not 1",
        "1 < 2",
        "-(1 < 2)",
        "(1 < 2) + not 3",
        "if(1, 2, 3)",
        "if(1 < 2, 1 < 2, 3)",
        "min(1 < 2, 3)",
        "a < b < c",
        "not a == b and c",
        "and",
        "or",
        "a and",
        "a or b",
        // exponents (#27) changed only a number directly followed by e or
        // E, which was always an error; these must mean what they did
        "1 e5",
        "e5",
        "x1e5",
        "1 and 2",
        "1and 2 > 1",
        "2 * e",
    ] {
        for boolean in [false, true] {
            assert_eq!(
                new_outcome(src, boolean),
                old_outcome(src, boolean),
                "{src:?} as {}",
                if boolean { "bool" } else { "num" }
            );
        }
    }
}

/// Nesting far past anything the call stack could take, in every shape
/// that nests: prefix operators, binary operators each way, calls, if,
/// and parentheses alone. The recursive implementation overflowed its
/// stack on the last at about 100 000 (#18).
#[test]
fn any_depth_is_parsed_evaluated_cloned_and_dropped() {
    const N: usize = 200_000;
    let env: BTreeMap<String, f64> = [("a".to_string(), 1.0)].into_iter().collect();
    let cases: Vec<(String, f64, usize)> = vec![
        // minus, N times, around 1
        (format!("{}1{}", "-(".repeat(N), ")".repeat(N)), 1.0, N + 1),
        // right-nested sums: 1 + (1 + (… + a))
        (
            format!("{}a{}", "1 + (".repeat(N), ")".repeat(N)),
            (N + 1) as f64,
            N + 1,
        ),
        // left-nested, no parentheses needed: 1 - 1 - … - 1
        (format!("1{}", " - 1".repeat(N)), 1.0 - N as f64, N + 1),
        // calls and ifs
        (
            format!("{}a{}", "abs(".repeat(N), ")".repeat(N)),
            1.0,
            N + 1,
        ),
        (
            // each if is one deeper than its condition, `a > 0`, which is two
            format!("{}a{}", "if(a > 0, ".repeat(N), ", 0)".repeat(N)),
            1.0,
            N + 2,
        ),
        // parentheses alone add no depth at all
        (format!("{}a{}", "(".repeat(N), ")".repeat(N)), 1.0, 1),
    ];
    for (src, want, depth) in cases {
        let e = parse_num(&src).unwrap();
        assert_eq!(e.depth(), depth, "{}…", &src[..40]);
        assert_eq!(eval(&e, &env), Ok(want), "{}…", &src[..40]);
        let copy = e.clone();
        assert_eq!(copy, e);
        drop(copy);
        drop(e);
    }
}

#[test]
fn if_and_and_or_evaluate_only_what_they_need() {
    let zero: BTreeMap<String, f64> = [("d".to_string(), 0.0)].into_iter().collect();
    for (src, want) in [
        ("if(d == 0, 0, 1 / d)", 0.0),
        ("if(d != 0, 1 / d, 7)", 7.0),
        ("if(d == 0 or 1 / d > 0, 1, 2)", 1.0),
        ("if(d != 0 and 1 / d > 0, 1, 2)", 2.0),
    ] {
        assert_eq!(eval(&parse_num(src).unwrap(), &zero), Ok(want), "{src}");
    }
}
