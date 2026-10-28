//! A definition's parts, as the Parts panel lists them (#68).

use isolex_studio_core::outline::{outline, Inherited, Outline, Row};

const SAMPLE: &str = include_str!("../../tests/fixtures/sample.ilx");

fn files() -> Vec<(String, String)> {
    vec![("sample.ilx".to_string(), SAMPLE.to_string())]
}

fn of(name: &str) -> Outline {
    outline(&files(), name).unwrap_or_else(|| panic!("no {name}"))
}

/// Each row as "kind note", nested rows indented.
fn rows(rows: &[Row], depth: usize, out: &mut Vec<String>) {
    for r in rows {
        out.push(
            format!("{}{} {}", "  ".repeat(depth), r.kind, r.note)
                .trim_end()
                .to_string(),
        );
        rows_of(r, depth, out);
    }
}

fn rows_of(r: &Row, depth: usize, out: &mut Vec<String>) {
    rows(&r.children, depth + 1, out);
}

fn listed(o: &Outline) -> Vec<String> {
    let mut out = Vec::new();
    for p in &o.parts {
        let variant = p
            .variant
            .as_deref()
            .map(|v| format!(" in {v}"))
            .unwrap_or_default();
        out.push(format!("{} {}{variant}", p.op, p.name));
        rows(&p.shapes, 1, &mut out);
    }
    out
}

#[test]
fn a_definition_lists_its_parts_and_their_shapes() {
    let o = of("switch-poe");
    assert_eq!(
        (o.kind.as_str(), o.extends.as_deref()),
        ("icon", Some("switch-l2"))
    );
    assert_eq!(
        listed(&o),
        [
            "replace ports",
            "  repeat ×8",
            "    facerect front",
            "remove chassis",
            "part everything-else",
            "  tile 2×1",
            "  cylinder r 0.4 · h 0.12",
            "  poly3d 3 points",
            "  shadow 0.48×0.48",
            "  seg",
            "  at at 0.5 0.5 0.6",
            "    line",
            "    circle r 8",
            "    ellipse 4.5×8",
            "    rect 18×12",
            "    arc",
            "    poly points by repeat",
            "    insert mini-glyph",
            "  insert disk-stack",
            "  badge state-down",
        ]
    );
    // it replaces one part and removes the other it would inherit
    assert_eq!(o.inherited, []);
}

#[test]
fn every_place_is_the_node_it_names() {
    for name in [
        "switch-poe",
        "switch-l3",
        "state",
        "state-down",
        "disk-stack",
    ] {
        let o = of(name);
        let text = |p: &isolex_studio_core::compile::Place| {
            assert_eq!(p.file, "sample.ilx");
            &SAMPLE[p.start..p.start + p.len]
        };
        assert!(text(&o.node).starts_with(&format!("{} \"{name}\"", o.kind)));
        fn each(r: &Row, text: &dyn Fn(&isolex_studio_core::compile::Place) -> &'static str) {
            assert!(text(&r.node).starts_with(&r.kind), "{}", text(&r.node));
            r.children.iter().for_each(|c| each(c, text));
        }
        for p in &o.parts {
            assert!(text(&p.node).starts_with(&format!("{} \"{}\"", p.op, p.name)));
            p.shapes.iter().for_each(|r| each(r, &text));
        }
    }
}

#[test]
fn inherited_parts_name_where_they_come_from() {
    let o = of("switch-l3");
    assert_eq!(
        listed(&o),
        ["part routing-badge", "  at at 1 0.5 0.28", "    path"]
    );
    let inherited = |name: &str, from: &str| Inherited {
        name: name.into(),
        from: from.into(),
        shapes: 1,
    };
    assert_eq!(
        o.inherited,
        [
            inherited("chassis", "rack-unit"),
            inherited("ports", "switch")
        ]
    );
    // a child with no parts of its own draws its parent's
    assert_eq!(of("switch-l2").parts, []);
    assert_eq!(of("switch-l2").inherited, o.inherited);
}

#[test]
fn an_axis_lists_its_variants_parts() {
    assert_eq!(
        listed(&of("state")),
        [
            "part state-badge in down",
            "  badge state-down",
            "part state-badge in planned",
            "  badge state-planned",
        ]
    );
}

#[test]
fn a_flat_icon_and_a_fragment() {
    assert_eq!(
        listed(&of("state-down")),
        [
            "part octagon",
            "  at at 0 0 0",
            "    poly 8 points",
            "    path"
        ]
    );
    assert_eq!(
        listed(&of("disk-stack")),
        ["part stack", "  repeat ×n", "    cylinder r 0.26 · h 0.2"]
    );
}

#[test]
fn there_is_none_of_what_is_not_there() {
    assert!(outline(&files(), "nope").is_none());
    // a parent that is not there leaves nothing inherited, and no panic
    let broken = vec![(
        "a.ilx".to_string(),
        "icon \"a\" {\n  extends \"gone\"\n  canvas 4 4\n}\n".to_string(),
    )];
    assert_eq!(outline(&broken, "a").unwrap().inherited, []);
}

#[test]
fn a_cycle_of_extends_ends() {
    let files = vec![(
        "a.ilx".to_string(),
        "base \"a\" {\n  extends \"b\"\n  part \"x\" {\n    box 0 0 0 1 1 1\n  }\n}\n\
         base \"b\" {\n  extends \"a\"\n  part \"y\" {\n    box 0 0 0 1 1 1\n  }\n}\n"
            .to_string(),
    )];
    let o = outline(&files, "a").unwrap();
    assert_eq!(
        o.inherited
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>(),
        ["y"]
    );
}
