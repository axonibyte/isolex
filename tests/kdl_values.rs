//! Each value's own bytes (#52): the span of a value's literal, and the
//! writer that makes one. The studio edits source by replacing a value's
//! literal with a written one, so both must give back the same value.

use std::path::Path;

use isolex::kdl::{write_value, Document, NodeRef, Value};

/// Floats by their bits, so NaN equals itself.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => {
            x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())
        }
        _ => a == b,
    }
}

/// The value of the only entry of `n <literal>`.
fn read_back(literal: &str) -> Result<Value, String> {
    let src = format!("n {literal}");
    let doc = Document::parse(&src).map_err(|e| format!("{src:?}: {}", e.message))?;
    let node = doc.nodes().next().ok_or("no node")?;
    match node.entries() {
        [entry] if entry.name().is_some() || entry.ty().is_some() => {
            Err(format!("{src:?}: more than a value"))
        }
        [entry] => Ok(entry.value().clone()),
        other => Err(format!("{src:?}: {} entries", other.len())),
    }
}

fn check_spans<'a>(
    src: &str,
    nodes: impl Iterator<Item = NodeRef<'a>>,
    failures: &mut Vec<String>,
) {
    for node in nodes {
        for entry in node.entries() {
            let (outer, inner) = (entry.span(), entry.value_span());
            if inner.offset() < outer.offset()
                || inner.offset() + inner.len() != outer.offset() + outer.len()
            {
                failures.push(format!("value span {inner:?} is not the end of {outer:?}"));
            }
            let raw = entry.raw(src);
            match read_back(raw) {
                Ok(v) if same(&v, entry.value()) => {}
                Ok(v) => failures.push(format!("{raw:?} reads as {v:?}, not {:?}", entry.value())),
                Err(e) => failures.push(e),
            }
        }
        if let Some(children) = node.children() {
            check_spans(src, children.nodes(), failures);
        }
    }
}

#[test]
fn every_value_span_in_the_kdl_suite_reads_back_as_its_value() {
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/kdl-suite/input");
    let mut checked = 0;
    let mut failures = Vec::new();
    for file in std::fs::read_dir(input).expect("the vendored suite") {
        let path = file.unwrap().path();
        let src = std::fs::read_to_string(&path).unwrap();
        let Ok(doc) = Document::parse(&src) else {
            continue;
        };
        let before = failures.len();
        check_spans(&src, doc.nodes(), &mut failures);
        for f in &mut failures[before..] {
            *f = format!("{}: {f}", path.file_name().unwrap().to_string_lossy());
        }
        checked += 1;
    }
    assert!(checked > 150, "only {checked} documents parsed");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_value_span_leaves_out_the_key_and_the_type() {
    let src = "n (u8)12 key=(f)\"two\" #\"raw\"# 1.5e3 bare";
    let doc = Document::parse(src).unwrap();
    let raws: Vec<&str> = doc
        .nodes()
        .next()
        .unwrap()
        .entries()
        .iter()
        .map(|e| e.raw(src))
        .collect();
    assert_eq!(raws, ["12", "\"two\"", "#\"raw\"#", "1.5e3", "bare"]);
}

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
}

const SEED: u64 = 0x5EED_0052;

fn round_trip(value: &Value) -> Result<(), String> {
    let written = write_value(value);
    match read_back(&written) {
        Ok(v) if same(&v, value) => {}
        Ok(v) => return Err(format!("{value:?} written {written:?} reads as {v:?}")),
        Err(e) => return Err(format!("{value:?} written {written:?}: {e}")),
    }
    // and as a property's value
    let src = format!("n k={written}");
    match Document::parse(&src) {
        Ok(doc) => match doc.nodes().next().map(|n| n.entries()) {
            Some([e]) if same(e.value(), value) => Ok(()),
            other => Err(format!("{src:?} reads as {other:?}")),
        },
        Err(e) => Err(format!("{src:?}: {}", e.message)),
    }
}

#[test]
fn written_floats_read_back_the_same() {
    let mut rng = Rng(SEED);
    let mut values: Vec<f64> = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.1,
        0.28,
        1e300,
        1e-300,
        5e-324,
        f64::MAX,
        f64::MIN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        12.0,
        1e21,
        123456789012345680.0,
    ];
    for _ in 0..20_000 {
        values.push(f64::from_bits(rng.next()));
        // and numbers of the size a lexicon writes
        values.push((rng.next() % 2_000_001) as f64 / 1000.0 - 1000.0);
    }
    let failures: Vec<String> = values
        .iter()
        .filter_map(|&f| round_trip(&Value::Float(f)).err())
        .take(10)
        .collect();
    assert!(
        failures.is_empty(),
        "seed {SEED:#x}:\n{}",
        failures.join("\n")
    );
    assert_eq!(
        write_value(&Value::Float(2.0)),
        "2.0",
        "a whole float stays a float"
    );
    assert_eq!(write_value(&Value::Float(0.28)), "0.28");
}

#[test]
fn written_integers_read_back_the_same() {
    let mut rng = Rng(SEED ^ 1);
    let mut values: Vec<i128> = vec![0, 1, -1, i128::MAX, i128::MIN + 1, i64::MIN as i128];
    for _ in 0..5_000 {
        values.push(((rng.next() as i128) << 64 | rng.next() as i128) >> rng.below(127));
    }
    let failures: Vec<String> = values
        .iter()
        .filter_map(|&n| round_trip(&Value::Integer(n)).err())
        .take(10)
        .collect();
    assert!(
        failures.is_empty(),
        "seed {SEED:#x}:\n{}",
        failures.join("\n")
    );
}

#[test]
fn written_strings_read_back_the_same() {
    // pieces that are each legal somewhere and wrong somewhere else
    const PIECES: &[&str] = &[
        "a", "Z", "box", "-", "+", ".", "_", "1", "0x", "e", "#", "\"", "\\", "\"#", "=", "(", ")",
        "{", "}", ";", "[", "/", "//", " ", "\t", "\n", "\r\n", "\u{85}", "\u{2028}", "\u{7}",
        "\u{7F}", "\u{FEFF}", "\u{200E}", "é", "日本", "🙂", "true", "null", "inf", "-inf", "nan",
    ];
    let mut rng = Rng(SEED ^ 2);
    let mut values: Vec<String> = PIECES.iter().map(|p| p.to_string()).collect();
    values.push(String::new());
    for _ in 0..20_000 {
        let n = 1 + rng.below(6);
        values.push((0..n).map(|_| PIECES[rng.below(PIECES.len())]).collect());
    }
    let failures: Vec<String> = values
        .into_iter()
        .filter_map(|s| round_trip(&Value::String(s)).err())
        .take(10)
        .collect();
    assert!(
        failures.is_empty(),
        "seed {SEED:#x}:\n{}",
        failures.join("\n")
    );
    assert_eq!(write_value(&Value::String("body-top".into())), "body-top");
    assert_eq!(
        write_value(&Value::String("u * 0.28".into())),
        "\"u * 0.28\""
    );
    assert_eq!(write_value(&Value::String("true".into())), "\"true\"");
    assert_eq!(write_value(&Value::String("a\"#b".into())), "##\"a\"#b\"##");
}

#[test]
fn written_keywords_read_back_the_same() {
    for value in [Value::Bool(true), Value::Bool(false), Value::Null] {
        round_trip(&value).unwrap();
    }
}
