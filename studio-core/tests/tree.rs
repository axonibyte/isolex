//! The Tree tab's rows (#71): over the graph's fixture, which makes every
//! kind of reference, kept as a golden (`tests/fixtures/graph.tree.golden
//! .json`; UPDATE_GOLDENS=1 writes it again), and read as an outline here.

use isolex_studio_core::tree::{tree, Tree, TreeRow};

fn files(name: &str, text: &str) -> Vec<(String, String)> {
    vec![(name.to_string(), text.to_string())]
}

/// Each row as "kind name", nested rows indented, with what a click opens
/// when it is not the row's own.
fn listed(rows: &[TreeRow], depth: usize, out: &mut Vec<String>) {
    for r in rows {
        let mut line = format!("{}{} {}", "  ".repeat(depth), r.kind, r.name);
        if let (Some(a), Some(v)) = (&r.axis, &r.variant) {
            line += &format!(" (variant {v} of axis {a})");
        }
        if r.icon.is_none() {
            line += " · no thumbnail";
        }
        out.push(line);
        listed(&r.children, depth + 1, out);
    }
}

fn outline(t: &Tree) -> Vec<String> {
    let mut out = Vec::new();
    listed(&t.icons, 0, &mut out);
    out.push("--".into());
    listed(&t.fonts, 0, &mut out);
    out
}

#[test]
fn the_graph_fixture_as_the_tree_lists_it() {
    let t = tree(&files(
        "graph.ilx",
        include_str!("../../tests/fixtures/graph.ilx"),
    ));
    assert_eq!(
        outline(&t),
        [
            "base chassis · no thumbnail",
            "  icon server",
            "    variant server-down (variant down of axis state)",
            "  icon switch",
            "icon down",
            "--",
            "font mono · no thumbnail",
        ]
    );
    // a variant opens its axis, drawing the variant
    let v = &t.icons[0].children[0].children[0];
    assert_eq!(v.definition, "state");
    assert_eq!(v.icon.as_deref(), Some("server-down"));

    let json = serde_json::to_string_pretty(&t).unwrap() + "\n";
    let golden = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/graph.tree.golden.json"
    );
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(golden, &json).unwrap();
    }
    let want = std::fs::read_to_string(golden)
        .expect("golden missing: run with UPDATE_GOLDENS=1 once, then review the diff");
    assert_eq!(
        json,
        want.replace("\r\n", "\n"),
        "the tree drifted from its golden"
    );
}

#[test]
fn the_sample_nests_children_under_their_bases_and_variants_under_their_icons() {
    let t = tree(&files(
        "sample.ilx",
        include_str!("../../tests/fixtures/sample.ilx"),
    ));
    assert_eq!(
        outline(&t),
        [
            "base rack-unit · no thumbnail",
            "  base switch · no thumbnail",
            "    icon switch-l2",
            "      icon switch-l3",
            "      icon switch-poe",
            "        variant switch-poe-down (variant down of axis state)",
            "        variant switch-poe-planned (variant planned of axis state)",
            "icon state-down",
            "icon state-planned",
            "--",
        ]
    );
}

#[test]
fn a_cycle_or_a_missing_base_still_lists_every_definition_once() {
    let text = r#"
base "a" extends="b" { category "x"; footprint 1 1 1 }
base "b" extends="a" { category "x"; footprint 1 1 1 }
icon "c" extends="nowhere" { category "x"; footprint 1 1 1 }
icon "d" extends="c" { category "x"; footprint 1 1 1 }
"#;
    let t = tree(&files("cycle.ilx", text));
    let lines = outline(&t);
    // c's base is not there, so it is a root; a and b extend each other,
    // so neither is, and they follow, from the first
    assert_eq!(
        lines,
        [
            "icon c · no thumbnail",
            "  icon d · no thumbnail",
            "base a · no thumbnail",
            "  base b · no thumbnail",
            "--"
        ]
    );
}
