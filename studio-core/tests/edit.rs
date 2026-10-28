//! The source-edit engine (#64): seeded random intents over the
//! compiler's reference fixtures, each changing only the bytes it means
//! to and undone exactly; and the choices for expressions and inherited
//! nodes.

use std::collections::BTreeMap;

use isolex::kdl::{Document, Value};
use isolex_studio_core::edit::{plan, Choice, Intent, NodeRef, Plan, Refusal, Request, Slot};
use isolex_studio_core::Library;

const FIXTURES: &[(&str, &str)] = &[
    (
        "sample.ilx",
        include_str!("../../tests/fixtures/sample.ilx"),
    ),
    (
        "transforms.ilx",
        include_str!("../../tests/fixtures/transforms.ilx"),
    ),
    ("text.ilx", include_str!("../../tests/fixtures/text.ilx")),
    ("graph.ilx", include_str!("../../tests/fixtures/graph.ilx")),
    (
        "palette.ilx",
        include_str!("../../tests/fixtures/palette.ilx"),
    ),
];

/// xorshift64*: deterministic, no dependencies.
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
        (self.next() % n.max(1) as u64) as usize
    }
}

fn library(file: &str, text: &str) -> Library {
    let files: BTreeMap<String, String> =
        [(file.to_string(), text.to_string())].into_iter().collect();
    Library::new("/nowhere", files)
}

/// A node as compared: its depth, name and entries (floats by bits).
#[derive(Debug, Clone, PartialEq)]
struct Flat {
    depth: usize,
    name: String,
    entries: Vec<(Option<String>, String)>,
    start: usize,
    len: usize,
    end: usize,
    children: bool,
    /// Its parent's position in the list.
    parent: Option<usize>,
}

fn show(v: &Value) -> String {
    match v {
        Value::Float(f) => format!("f{}", f.to_bits()),
        Value::Integer(i) => format!("f{}", (*i as f64).to_bits()),
        other => format!("{other:?}"),
    }
}

fn flatten(src: &str) -> Vec<Flat> {
    let doc = Document::parse(src).expect("the source parses");
    let mut out: Vec<Flat> = Vec::new();
    let mut todo: Vec<(isolex::kdl::NodeRef, usize, Option<usize>)> =
        doc.nodes().map(|n| (n, 0, None)).collect();
    todo.reverse();
    while let Some((n, depth, parent)) = todo.pop() {
        let s = n.span();
        let idx = out.len();
        out.push(Flat {
            depth,
            name: n.name().value().to_string(),
            entries: n
                .entries()
                .iter()
                .map(|e| (e.name().map(|k| k.value().to_string()), show(e.value())))
                .collect(),
            start: s.offset(),
            len: s.len(),
            end: s.offset() + s.len(),
            children: n.children().is_some(),
            parent,
        });
        if let Some(c) = n.children() {
            let kids: Vec<_> = c.nodes().collect();
            for k in kids.into_iter().rev() {
                todo.push((k, depth + 1, Some(idx)));
            }
        }
    }
    out
}

/// The shape of the document: every node but where it is.
/// A node's entries: each key (none for an argument) and value.
type Entries = Vec<(Option<String>, String)>;

fn shape(nodes: &[Flat]) -> Vec<(usize, String, Entries)> {
    nodes
        .iter()
        .map(|n| (n.depth, n.name.clone(), n.entries.clone()))
        .collect()
}

fn at(file: &str, n: &Flat) -> NodeRef {
    NodeRef {
        file: file.to_string(),
        start: n.start,
        len: n.len,
    }
}

/// Plans and applies `intent`, which must come to patches; returns the
/// file's text after, having checked undo restores it exactly.
fn run(lib: &mut Library, file: &str, intent: Intent) -> String {
    let before = lib.files()[file].clone();
    let req = Request::new(lib, None, "edit", intent.clone());
    let Plan::Apply(tx) = plan(lib, &req).unwrap_or_else(|e| panic!("{intent:?}: {e}")) else {
        panic!("{intent:?} asked for a choice")
    };
    lib.apply(tx).unwrap();
    let after = lib.files()[file].clone();
    lib.undo();
    assert_eq!(lib.files()[file], before, "undo is exact");
    lib.redo();
    after
}

/// The bytes before `a` and after `b` (from the end) are the same.
fn same_outside(old: &str, new: &str, from: usize, old_to: usize) {
    assert_eq!(&old[..from], &new[..from], "the bytes before the change");
    assert_eq!(
        &old[old_to..],
        &new[new.len() - (old.len() - old_to)..],
        "the bytes after the change"
    );
}

#[test]
fn a_value_set_changes_that_value_and_no_other_byte() {
    let mut runs = 0;
    for (file, text) in FIXTURES {
        for seed in 1..=60u64 {
            let mut rng = Rng(0x5EED_0064 ^ (seed << 13) ^ file.len() as u64);
            let mut lib = library(file, text);
            let old = lib.files()[*file].clone();
            let nodes = flatten(&old);
            // a node with a number among its arguments
            let candidates: Vec<(usize, usize)> = nodes
                .iter()
                .enumerate()
                .flat_map(|(i, n)| {
                    n.entries
                        .iter()
                        .filter(|e| e.0.is_none())
                        .enumerate()
                        .filter(|(_, e)| e.1.starts_with('f'))
                        .map(move |(a, _)| (i, a))
                })
                .collect();
            let (i, arg) = candidates[rng.below(candidates.len())];
            let value = (rng.below(4000) as f64 - 2000.0) / 100.0;
            let new = run(
                &mut lib,
                file,
                Intent::SetValue {
                    node: at(file, &nodes[i]),
                    slot: Slot::Arg(arg),
                    value: Value::Float(value),
                },
            );
            // one splice, inside the node: everything around it is as it was
            let from = old
                .bytes()
                .zip(new.bytes())
                .take_while(|(a, b)| a == b)
                .count()
                .min(nodes[i].start + nodes[i].len);
            assert!(
                from >= nodes[i].start,
                "seed {seed:#x} {file}: the change starts inside the node"
            );
            let after = flatten(&new);
            let mut want = shape(&nodes);
            let args: Vec<usize> = nodes[i]
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.0.is_none())
                .map(|(k, _)| k)
                .collect();
            want[i].2[args[arg]].1 = format!("f{}", value.to_bits());
            assert_eq!(
                shape(&after),
                want,
                "seed {seed:#x} {file}: only that value changed"
            );
            same_outside(&old, &new, nodes[i].start, nodes[i].end);
            runs += 1;
        }
    }
    assert_eq!(runs, 300);
}

#[test]
fn an_insert_adds_one_node_where_asked_and_keeps_the_rest() {
    for (file, text) in FIXTURES {
        for seed in 1..=40u64 {
            let mut rng = Rng(0x5EED_1064 ^ (seed << 11) ^ file.len() as u64);
            let mut lib = library(file, text);
            let old = lib.files()[*file].clone();
            let nodes = flatten(&old);
            let parents: Vec<usize> = (0..nodes.len()).filter(|i| nodes[*i].children).collect();
            let p = parents[rng.below(parents.len())];
            let kids: Vec<usize> = (0..nodes.len())
                .filter(|i| nodes[*i].parent == Some(p))
                .collect();
            let before = if !kids.is_empty() && rng.below(2) == 0 {
                Some(kids[rng.below(kids.len())])
            } else {
                None
            };
            let new = run(
                &mut lib,
                file,
                Intent::Insert {
                    parent: at(file, &nodes[p]),
                    text: "inserted 1 2 \"three\"".into(),
                    before: before.map(|b| at(file, &nodes[b])),
                },
            );
            let after = flatten(&new);
            // the new node is a child of the parent, at the place asked
            let new_idx = after
                .iter()
                .position(|n| n.name == "inserted")
                .expect("inserted");
            assert_eq!(
                after[new_idx].depth,
                nodes[p].depth + 1,
                "seed {seed} {file}"
            );
            let mut without: Vec<Flat> = after.clone();
            without.remove(new_idx);
            assert_eq!(
                shape(&without),
                shape(&nodes),
                "seed {seed} {file}: nothing else changed"
            );
            // an insert leaves every node before it where it was in the
            // list, the parent among them
            assert_eq!(
                after[new_idx].parent,
                Some(p),
                "seed {seed} {file}: a child of the parent"
            );
            let siblings: Vec<&Flat> = after.iter().filter(|n| n.parent == Some(p)).collect();
            let place = siblings.iter().position(|n| n.name == "inserted").unwrap();
            let want = match before {
                Some(b) => kids.iter().position(|k| *k == b).unwrap(),
                None => kids.len(),
            };
            assert_eq!(
                place, want,
                "seed {seed} {file}: before the sibling asked, or last"
            );
            // comments are all still there
            assert_eq!(new.matches("//").count(), old.matches("//").count());
        }
    }
}

#[test]
fn a_delete_removes_its_node_and_keeps_comments_on_their_own_lines() {
    for (file, text) in FIXTURES {
        for seed in 1..=40u64 {
            let mut rng = Rng(0x5EED_2064 ^ (seed << 9) ^ file.len() as u64);
            let mut lib = library(file, text);
            let old = lib.files()[*file].clone();
            let nodes = flatten(&old);
            let i = 1 + rng.below(nodes.len() - 1);
            let n = &nodes[i];
            let new = run(&mut lib, file, Intent::Delete { node: at(file, n) });
            let gone: Vec<usize> = (0..nodes.len())
                .filter(|k| nodes[*k].start >= n.start && nodes[*k].end <= n.end)
                .collect();
            let want: Vec<Flat> = nodes
                .iter()
                .enumerate()
                .filter(|(k, _)| !gone.contains(k))
                .map(|(_, f)| f.clone())
                .collect();
            assert_eq!(shape(&flatten(&new)), shape(&want), "seed {seed} {file}");
            // a comment on a line of its own, outside the node, stays
            let own_line = |s: &str, skip: Option<(usize, usize)>| {
                let mut off = 0;
                let mut count = 0;
                for line in s.split_inclusive('\n') {
                    let inside = skip.is_some_and(|(a, b)| off >= a && off < b);
                    if line.trim_start().starts_with("//") && !inside {
                        count += 1;
                    }
                    off += line.len();
                }
                count
            };
            assert_eq!(
                own_line(&new, None),
                own_line(&old, Some((n.start, n.end))),
                "seed {seed} {file}"
            );
            // and its lines go with it: no blank line is left in their place
            let blank = |s: &str| s.lines().filter(|l| l.trim().is_empty()).count();
            assert!(
                blank(&new) <= blank(&old),
                "seed {seed} {file}: a blank line was left"
            );
        }
    }
}

#[test]
fn a_move_reorders_siblings_and_nothing_else() {
    let mut moves = 0;
    for (file, text) in FIXTURES {
        for seed in 1..=40u64 {
            let mut rng = Rng(0x5EED_3064 ^ (seed << 7) ^ file.len() as u64);
            let mut lib = library(file, text);
            let old = lib.files()[*file].clone();
            let nodes = flatten(&old);
            // a node with a sibling, each on lines of its own
            let alone = |n: &Flat| {
                let line = old[..n.start].rfind('\n').map_or(0, |i| i + 1);
                let eol = old[n.end..].find('\n').map_or(old.len(), |i| n.end + i);
                old[line..n.start].trim().is_empty() && old[n.end..eol].trim().is_empty()
            };
            let pairs: Vec<(usize, Option<usize>)> = (0..nodes.len())
                .filter(|i| alone(&nodes[*i]))
                .flat_map(|i| {
                    let sibs: Vec<usize> = (0..nodes.len())
                        .filter(|k| {
                            *k != i && nodes[*k].parent == nodes[i].parent && alone(&nodes[*k])
                        })
                        .collect();
                    sibs.into_iter()
                        .map(Some)
                        .chain(std::iter::once(None))
                        .map(move |s| (i, s))
                })
                .collect();
            let (i, before) = pairs[rng.below(pairs.len())];
            let new = run(
                &mut lib,
                file,
                Intent::MoveBefore {
                    node: at(file, &nodes[i]),
                    before: before.map(|b| at(file, &nodes[b])),
                },
            );
            let mut a = shape(&flatten(&new));
            let mut b = shape(&nodes);
            a.sort_by(|x, y| format!("{x:?}").cmp(&format!("{y:?}")));
            b.sort_by(|x, y| format!("{x:?}").cmp(&format!("{y:?}")));
            assert_eq!(a, b, "seed {seed} {file}: the same nodes");
            assert_eq!(new.len(), old.len(), "seed {seed} {file}: only moved");
            moves += 1;
        }
    }
    assert_eq!(moves, 200);
}

fn sample() -> Library {
    library("sample.ilx", FIXTURES[0].1)
}

/// The node in `src` whose source starts with `prefix`.
fn node(src: &str, prefix: &str) -> NodeRef {
    let n = flatten(src)
        .into_iter()
        .find(|n| src[n.start..].starts_with(prefix))
        .unwrap_or_else(|| panic!("no node {prefix}"));
    at("sample.ilx", &n)
}

#[test]
fn an_expression_offers_its_param_or_a_number() {
    let mut lib = sample();
    let src = lib.files()["sample.ilx"].clone();
    let boxed = node(&src, "box 0 0 0 2 1 \"u * 0.28\"");
    // 0.56 is u * 0.28 with u = 2
    let req = Request::new(
        &lib,
        Some("rack-unit"),
        "Resize box",
        Intent::SetValue {
            node: boxed.clone(),
            slot: Slot::Arg(5),
            value: Value::Float(0.56),
        },
    );
    let Ok(Plan::Choose(choices)) = plan(&lib, &req) else {
        panic!("an expression asks")
    };
    let Choice::EditParam {
        param,
        definition,
        value,
        then,
    } = &choices[0]
    else {
        panic!("{choices:?}")
    };
    assert_eq!((param.as_str(), definition.as_str()), ("u", "rack-unit"));
    assert!((value - 2.0).abs() < 1e-9);
    let Ok(Plan::Apply(tx)) = plan(&lib, then) else {
        panic!()
    };
    lib.apply(tx).unwrap();
    assert!(
        lib.files()["sample.ilx"].contains("  param u 2\n"),
        "the param's default, where it is declared"
    );
    assert!(
        lib.files()["sample.ilx"].contains("\"u * 0.28\""),
        "the expression is kept"
    );
    lib.undo();
    // the undo is a change too: what was planned before it is stale now
    assert!(matches!(plan(&lib, &req), Err(Refusal::Stale { .. })));
    let req = Request::new(
        &lib,
        Some("rack-unit"),
        "Resize box",
        Intent::SetValue {
            node: boxed.clone(),
            slot: Slot::Arg(5),
            value: Value::Float(0.56),
        },
    );
    let Ok(Plan::Choose(choices)) = plan(&lib, &req) else {
        panic!("an expression asks")
    };
    let Choice::ReplaceWithNumber { then } = &choices[1] else {
        panic!("{choices:?}")
    };
    let Ok(Plan::Apply(tx)) = plan(&lib, then) else {
        panic!()
    };
    lib.apply(tx).unwrap();
    assert!(lib.files()["sample.ilx"].contains("box 0 0 0 2 1 0.56\n"));

    // one that reads a repeat's variable too cannot be solved for a param
    let src = lib.files()["sample.ilx"].clone();
    let port = node(&src, "facerect \"front\" \"0.1 + i * (1.8 / ports)\"");
    let req = Request::new(
        &lib,
        Some("switch"),
        "Move port",
        Intent::SetValue {
            node: port,
            slot: Slot::Arg(1),
            value: Value::Float(0.5),
        },
    );
    let Ok(Plan::Choose(choices)) = plan(&lib, &req) else {
        panic!()
    };
    assert!(
        matches!(choices.as_slice(), [Choice::ReplaceWithNumber { .. }]),
        "{choices:?}"
    );
}

#[test]
fn an_inherited_node_offers_its_base_or_an_override() {
    let mut lib = sample();
    let src = lib.files()["sample.ilx"].clone();
    let before = isolex::compile(&lib.sources());
    let port = node(&src, "facerect \"front\" \"0.1 + i * (1.8 / ports)\"");
    let req = Request::new(
        &lib,
        Some("switch-l2"),
        "Recolor port",
        Intent::SetValue {
            node: port,
            slot: Slot::Prop("opacity".into()),
            value: Value::Float(0.5),
        },
    );
    let Ok(Plan::Choose(choices)) = plan(&lib, &req) else {
        panic!("an inherited node asks")
    };
    let Choice::EditInBase { base, affected, .. } = &choices[0] else {
        panic!("{choices:?}")
    };
    assert_eq!(base, "switch");
    for icon in ["switch-l3", "switch-poe", "switch-poe-down"] {
        assert!(
            affected.iter().any(|a| a == icon),
            "{icon} follows: {affected:?}"
        );
    }
    assert!(
        !affected.iter().any(|a| a == "switch-l2"),
        "not the one being edited"
    );
    // overriding copies the part into the one being edited, unchanged
    let Choice::OverrideHere { part, then } = &choices[1] else {
        panic!("{choices:?}")
    };
    assert_eq!(part, "ports");
    let Ok(Plan::Apply(tx)) = plan(&lib, then) else {
        panic!()
    };
    lib.apply(tx).unwrap();
    let after = isolex::compile(&lib.sources());
    assert!(after.errors.is_empty(), "{:?}", after.errors);
    assert!(
        lib.files()["sample.ilx"].contains("  replace \"ports\" {"),
        "{}",
        lib.files()["sample.ilx"]
    );
    let ir = |c: &isolex::Compilation, name: &str| {
        serde_json::to_string(&c.doc.icons.iter().find(|i| i.name == name).unwrap().shapes).unwrap()
    };
    assert_eq!(
        ir(&after, "switch-l2"),
        ir(&before, "switch-l2"),
        "drawn as before"
    );
}

#[test]
fn values_and_properties_kept_set_added_and_removed() {
    let mut lib = sample();
    let src = lib.files()["sample.ilx"].clone();
    let seg = node(&src, "seg 0 0 0 2 0 0");
    // the same value keeps its spelling: no patch at all
    let req = Request::new(
        &lib,
        None,
        "x",
        Intent::SetValue {
            node: seg.clone(),
            slot: Slot::Prop("width".into()),
            value: Value::Float(1.2),
        },
    );
    assert_eq!(
        plan(&lib, &req),
        Ok(Plan::Apply(isolex_studio_core::Transaction::new(
            "x",
            vec![]
        )))
    );
    // a whole number is written as one
    let new = run(
        &mut lib,
        "sample.ilx",
        Intent::SetValue {
            node: seg.clone(),
            slot: Slot::Arg(3),
            value: Value::Float(3.0),
        },
    );
    assert!(new.contains("seg 0 0 0 3 0 0 stroke"), "{new}");
    let src = lib.files()["sample.ilx"].clone();
    let seg = node(&src, "seg 0 0 0 3 0 0");
    // a property that is not there is added after the last entry
    let new = run(
        &mut lib,
        "sample.ilx",
        Intent::SetValue {
            node: seg.clone(),
            slot: Slot::Prop("dash".into()),
            value: Value::String("2 1".into()),
        },
    );
    assert!(
        new.contains("seg 0 0 0 3 0 0 stroke=\"outline\" width=1.2 dash=\"2 1\"\n"),
        "{new}"
    );
    let src = lib.files()["sample.ilx"].clone();
    let seg = node(&src, "seg 0 0 0 3 0 0");
    let new = run(
        &mut lib,
        "sample.ilx",
        Intent::RemoveProp {
            node: seg,
            key: "stroke".into(),
        },
    );
    assert!(
        new.contains("seg 0 0 0 3 0 0 width=1.2 dash=\"2 1\"\n"),
        "{new}"
    );
    // an argument that is not there is refused
    let src = lib.files()["sample.ilx"].clone();
    let seg = node(&src, "seg 0 0 0 3 0 0");
    let req = Request::new(
        &lib,
        None,
        "x",
        Intent::SetValue {
            node: seg.clone(),
            slot: Slot::Arg(9),
            value: Value::Float(1.0),
        },
    );
    assert_eq!(
        plan(&lib, &req),
        Err(Refusal::NoSuchSlot(seg, Slot::Arg(9)))
    );
}

#[test]
fn a_request_from_an_older_revision_is_refused() {
    let mut lib = sample();
    let src = lib.files()["sample.ilx"].clone();
    let seg = node(&src, "seg 0 0 0 2 0 0");
    let req = Request::new(
        &lib,
        None,
        "x",
        Intent::SetValue {
            node: seg,
            slot: Slot::Arg(3),
            value: Value::Float(3.0),
        },
    );
    let Ok(Plan::Apply(tx)) = plan(&lib, &req) else {
        panic!()
    };
    lib.apply(tx).unwrap();
    assert!(matches!(plan(&lib, &req), Err(Refusal::Stale { .. })));
}

#[test]
fn inserts_open_blocks_follow_indentation_and_line_endings() {
    // a one-line block is opened up; a node with no block gets one; CRLF stays CRLF
    let text = "icon \"a\" {\r\n\tpart \"p\" { box 0 0 0 1 1 1 }\r\n\tpart \"q\"\r\n}\r\n";
    let mut lib = library("a.ilx", text);
    let nodes = flatten(text);
    let p = nodes
        .iter()
        .find(|n| n.name == "part" && text[n.start..].starts_with("part \"p\""))
        .unwrap();
    let new = run(
        &mut lib,
        "a.ilx",
        Intent::Insert {
            parent: at("a.ilx", p),
            text: "tile 0 0 1 1 1".into(),
            before: None,
        },
    );
    assert_eq!(new, "icon \"a\" {\r\n\tpart \"p\" {\r\n\t\tbox 0 0 0 1 1 1\r\n\t\ttile 0 0 1 1 1\r\n\t}\r\n\tpart \"q\"\r\n}\r\n");
    let nodes = flatten(&new);
    let q = nodes
        .iter()
        .find(|n| n.name == "part" && new[n.start..].starts_with("part \"q\""))
        .unwrap();
    let newer = run(
        &mut lib,
        "a.ilx",
        Intent::Insert {
            parent: at("a.ilx", q),
            text: "box 0 0 0 2 2 2".into(),
            before: None,
        },
    );
    assert_eq!(newer, "icon \"a\" {\r\n\tpart \"p\" {\r\n\t\tbox 0 0 0 1 1 1\r\n\t\ttile 0 0 1 1 1\r\n\t}\r\n\tpart \"q\" {\r\n\t\tbox 0 0 0 2 2 2\r\n\t}\r\n}\r\n");
}

#[test]
fn typed_text_is_one_splice_and_a_range_that_is_not_there_is_refused() {
    let mut lib = sample();
    let new = run(
        &mut lib,
        "sample.ilx",
        Intent::Type {
            file: "sample.ilx".into(),
            at: 0,
            len: 2,
            text: "##".into(),
        },
    );
    assert!(new.starts_with("##"));
    let req = Request::new(
        &lib,
        None,
        "type",
        Intent::Type {
            file: "sample.ilx".into(),
            at: new.len(),
            len: 1,
            text: String::new(),
        },
    );
    assert_eq!(plan(&lib, &req), Err(Refusal::BadRange));
}
