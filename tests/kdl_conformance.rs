//! isolex's KDL reader (#28) against the official KDL 2.0 test suite,
//! vendored in tests/kdl-suite (CC BY-SA 4.0, see its README): every
//! input parses exactly when the suite says it should, and then to the
//! same document as the suite's canonical printing of it.

use std::path::Path;

use isolex::kdl::{Document, NodeRef, Value};

/// A node as the suite's canonical form keeps it: properties down to the
/// rightmost of each name and sorted, an empty children block dropped.
#[derive(Debug, PartialEq)]
struct Canon {
    name: String,
    ty: Option<String>,
    args: Vec<(Option<String>, Val)>,
    props: Vec<(String, Option<String>, Val)>,
    children: Vec<Canon>,
}

/// A value, floats by their bits so NaN compares equal to itself.
#[derive(Debug, PartialEq)]
enum Val {
    S(String),
    I(i128),
    F(u64),
    B(bool),
    N,
}

fn val(v: &Value) -> Val {
    match v {
        Value::String(s) => Val::S(s.clone()),
        Value::Integer(i) => Val::I(*i),
        Value::Float(f) if f.is_nan() => Val::F(f64::NAN.to_bits()),
        Value::Float(f) => Val::F(f.to_bits()),
        Value::Bool(b) => Val::B(*b),
        Value::Null => Val::N,
    }
}

/// The suite's documents are shallow, so this one may recurse.
fn canon<'a>(nodes: impl Iterator<Item = NodeRef<'a>>) -> Vec<Canon> {
    nodes
        .map(|n| {
            let mut args = Vec::new();
            let mut props: Vec<(String, Option<String>, Val)> = Vec::new();
            for e in n.entries() {
                let ty = e.ty().map(|t| t.value().to_string());
                match e.name() {
                    None => args.push((ty, val(e.value()))),
                    Some(k) => {
                        props.retain(|(name, ..)| name != k.value());
                        props.push((k.value().to_string(), ty, val(e.value())));
                    }
                }
            }
            props.sort_by(|a, b| a.0.cmp(&b.0));
            Canon {
                name: n.name().value().to_string(),
                ty: n.ty().map(|t| t.value().to_string()),
                args,
                props,
                children: n.children().map(|c| canon(c.nodes())).unwrap_or_default(),
            }
        })
        .collect()
}

#[test]
fn passes_the_official_kdl_test_suite() {
    let suite = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/kdl-suite");
    let mut names: Vec<String> = std::fs::read_dir(suite.join("input"))
        .expect("the vendored suite")
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names.len(), 338, "the suite as vendored");
    let mut failures = Vec::new();
    for name in &names {
        let input = std::fs::read_to_string(suite.join("input").join(name)).unwrap();
        let expected = std::fs::read_to_string(suite.join("expected_kdl").join(name)).ok();
        match (Document::parse(&input), expected) {
            (Ok(_), None) => {
                failures.push(format!("{name}: accepted, but the suite says it fails"))
            }
            (Err(e), Some(_)) => failures.push(format!(
                "{name}: rejected at {}: {}, but the suite says it parses",
                e.offset, e.message
            )),
            (Err(_), None) => {}
            (Ok(doc), Some(expected)) => {
                let want = Document::parse(&expected)
                    .unwrap_or_else(|e| panic!("{name}: the expected form does not parse: {e}"));
                let (got, want) = (canon(doc.nodes()), canon(want.nodes()));
                if got != want {
                    failures.push(format!("{name}:\n  got  {got:?}\n  want {want:?}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} disagree with the suite:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}
