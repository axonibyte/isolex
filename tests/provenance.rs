//! Which source made each IR shape (#56): Compilation::provenance, one
//! entry per shape of each icon, a group's before its own, with the node
//! that drew it, the definition and part it is written in, the sites it
//! came through, and the map from its node's coordinates to the viewBox.

use isolex::ast::Span;
use isolex::expand::SiteKind;
use isolex::ir::IrShape;
use isolex::parser::parse_lexicon;
use isolex::{compile, Compilation, Map3, ShapeSource};

const FIXTURES: &[(&str, &str)] = &[
    ("sample.ilx", include_str!("fixtures/sample.ilx")),
    ("transforms.ilx", include_str!("fixtures/transforms.ilx")),
    ("text.ilx", include_str!("fixtures/text.ilx")),
    ("palette.ilx", include_str!("fixtures/palette.ilx")),
];

fn compiled(name: &str, src: &str) -> Compilation {
    let c = compile(&[(name.to_string(), src.to_string())]);
    assert!(c.errors.is_empty(), "{name}: {:?}", c.errors);
    c
}

/// The shapes in the order provenance lists them: each group, then its own.
fn preorder(shapes: &[IrShape]) -> Vec<&IrShape> {
    let mut out = Vec::new();
    for s in shapes {
        out.push(s);
        if let IrShape::Group { shapes, .. } = s {
            out.extend(shapes.iter());
        }
    }
    out
}

fn kind(shape: &IrShape) -> &'static str {
    match shape {
        IrShape::Polygon { .. } => "polygon",
        IrShape::Path { .. } => "path",
        IrShape::Ellipse { .. } => "ellipse",
        IrShape::Circle { .. } => "circle",
        IrShape::Rect { .. } => "rect",
        IrShape::Group { .. } => "group",
    }
}

/// The nodes that draw shapes of each kind.
fn drawn_by(kind: &str) -> &'static [&'static str] {
    match kind {
        "polygon" => &["box", "tile", "facerect", "poly3d", "poly", "cylinder"],
        "path" => &["seg", "line", "arc", "path", "cylinder"],
        "ellipse" => &["shadow", "ellipse", "cylinder"],
        "circle" => &["circle"],
        "rect" => &["rect"],
        "group" => &["transform", "text", "badge"],
        _ => &[],
    }
}

#[test]
fn every_shape_has_one_source_and_it_is_a_node_that_draws_its_kind() {
    let mut checked = 0;
    for (name, src) in FIXTURES {
        let c = compiled(name, src);
        let sources = parse_lexicon(&[(name.to_string(), src.to_string())])
            .lexicon
            .sources;
        assert_eq!(c.provenance.len(), c.doc.icons.len(), "{name}");
        for (icon, prov) in c.doc.icons.iter().zip(&c.provenance) {
            let shapes = preorder(&icon.shapes);
            assert_eq!(shapes.len(), prov.len(), "{name}: {}", icon.name);
            for (shape, p) in shapes.iter().zip(prov) {
                let node = sources
                    .get(p.span)
                    .unwrap_or_else(|| panic!("{name}: {}: {:?} is no node", icon.name, p.span));
                let k = kind(shape);
                assert!(
                    drawn_by(k).contains(&node.name.as_str()),
                    "{name}: {}: a {k} drawn by '{}'",
                    icon.name,
                    node.name
                );
                assert!(
                    !p.definition.is_empty(),
                    "{name}: {}: no definition",
                    icon.name
                );
                for site in &p.sites {
                    let at = sources.get(site.span).expect("a site is a node");
                    let want = match site.kind {
                        SiteKind::Insert => "insert",
                        SiteKind::Repeat => "repeat",
                        SiteKind::Badge => "badge",
                        SiteKind::Text => "text",
                    };
                    assert_eq!(at.name, want, "{name}: {}", icon.name);
                    assert_eq!(site.index.is_some(), want == "repeat" || want == "text");
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 300, "only {checked} shapes");
}

fn slice(src: &str, span: Span) -> &str {
    let text = &src[span.start..span.start + span.len];
    text.lines().next().unwrap().trim()
}

#[test]
fn inserted_repeated_and_badged_shapes_carry_their_sites() {
    let src = include_str!("fixtures/sample.ilx");
    let c = compiled("sample.ilx", src);
    let poe = c
        .doc
        .icons
        .iter()
        .position(|i| i.name == "switch-poe")
        .unwrap();
    let prov = &c.provenance[poe];

    // the ports a repeat replaced, one an iteration, in switch-poe's part
    let ports: Vec<&ShapeSource> = prov
        .iter()
        .filter(|p| p.part.as_deref() == Some("ports"))
        .collect();
    assert_eq!(ports.len(), 7, "when=\"i < 7\" leaves seven");
    for (i, p) in ports.iter().enumerate() {
        assert_eq!(p.definition, "switch-poe");
        assert_eq!(p.sites.len(), 1);
        assert_eq!(
            (p.sites[0].kind, p.sites[0].index),
            (SiteKind::Repeat, Some(i))
        );
        assert_eq!(slice(src, p.sites[0].span), "repeat 8 \"i\" {");
    }

    // a fragment's shapes, in the fragment, through the insert and then its repeat
    let disks: Vec<&ShapeSource> = prov
        .iter()
        .filter(|p| p.definition == "disk-stack")
        .collect();
    assert_eq!(disks.len(), 4, "two disks, each a side and a top");
    for p in &disks {
        assert_eq!(p.part.as_deref(), Some("stack"));
        assert_eq!(p.sites[0].kind, SiteKind::Insert);
        assert!(slice(src, p.sites[0].span).starts_with("insert \"disk-stack\""));
        assert_eq!(p.sites[1].kind, SiteKind::Repeat);
    }
    assert_eq!(disks[0].sites[1].index, Some(0));
    assert_eq!(disks[3].sites[1].index, Some(1));

    // a badge's group is the badge, and its shapes are the target's, reached through it
    let badge = prov
        .iter()
        .position(|p| slice(src, p.span).starts_with("badge"))
        .unwrap();
    assert_eq!(prov[badge].definition, "switch-poe");
    for p in &prov[badge + 1..] {
        assert_eq!(p.definition, "state-down");
        assert_eq!(p.part.as_deref(), Some("octagon"));
        assert_eq!(p.sites.len(), 1);
        assert_eq!(p.sites[0].kind, SiteKind::Badge);
        assert_eq!(p.sites[0].span, prov[badge].span);
    }
    assert_eq!(prov.len() - badge - 1, 2, "the octagon and its cross");

    // an axis variant's badge is written in the axis
    let down = c
        .doc
        .icons
        .iter()
        .position(|i| i.name == "switch-poe-down")
        .unwrap();
    let variant = c.provenance[down]
        .iter()
        .find(|p| p.part.as_deref() == Some("state-badge"))
        .unwrap();
    assert_eq!(variant.definition, "state");
}

fn apply(m: &Map3, [x, y, z]: [f64; 3]) -> [f64; 2] {
    [
        m[0] * x + m[2] * y + m[4] * z + m[6],
        m[1] * x + m[3] * y + m[5] * z + m[7],
    ]
}

fn group_apply(m: &[f64; 6], [x, y]: [f64; 2]) -> [f64; 2] {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

/// Every point of `shape` (in its group's frame) through `matrix`.
fn points(shape: &IrShape, matrix: &[f64; 6]) -> Vec<[f64; 2]> {
    match shape {
        IrShape::Polygon { points, .. } => points.iter().map(|p| group_apply(matrix, *p)).collect(),
        IrShape::Circle { cx, cy, .. } => vec![group_apply(matrix, [*cx, *cy])],
        other => panic!("not a shape this test draws: {other:?}"),
    }
}

#[test]
fn a_nodes_own_coordinates_map_to_where_its_shape_is_drawn() {
    // a box turned a quarter, inside a transform that scales and moves it,
    // and a circle on an anchor inside one that moves it
    let src = r#"icon "t" {
  category "c"
  footprint 2 2 1
  transform-bounds "grow"
  part "p" {
    transform scale=2 translate="3 -4" {
      turn 90 origin="1 1" {
        box 0 0 0 2 1 0.5
      }
    }
    at 0 0 0 {
      transform translate="5 6" {
        circle 1 2 3
      }
    }
  }
}
"#;
    let c = compiled("t.ilx", src);
    let icon = &c.doc.icons[0];
    let prov = &c.provenance[0];
    let shapes = preorder(&icon.shapes);
    let mut seen = 0;
    let mut matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    for (shape, p) in shapes.iter().zip(prov) {
        if let IrShape::Group { matrix: m, .. } = shape {
            matrix = *m;
            // a group's own map is its matrix
            assert!(near(
                apply(&p.map, [1.0, 2.0, 0.0]),
                group_apply(m, [1.0, 2.0])
            ));
            continue;
        }
        let drawn = points(shape, &matrix);
        let corners: Vec<[f64; 3]> = match slice(src, p.span) {
            "box 0 0 0 2 1 0.5" => {
                let mut c = Vec::new();
                for x in [0.0, 2.0] {
                    for y in [0.0, 1.0] {
                        for z in [0.0, 0.5] {
                            c.push([x, y, z]);
                        }
                    }
                }
                c
            }
            "circle 1 2 3" => vec![[1.0, 2.0, 0.0]],
            other => panic!("drawn by {other}"),
        };
        let mapped: Vec<[f64; 2]> = corners.iter().map(|c| apply(&p.map, *c)).collect();
        // each point drawn is a corner of the node, as written, mapped
        for d in &drawn {
            assert!(
                mapped.iter().any(|m| near(*m, *d)),
                "{d:?} is no corner in {mapped:?}"
            );
        }
        seen += 1;
    }
    assert_eq!(seen, 4, "three faces of the box and the circle");
}

#[test]
fn a_badged_shape_maps_through_the_badge() {
    let src = r#"icon "dot" {
  category "c"
  canvas 10 10
  part "p" {
    at 0 0 0 {
      circle 5 5 4
    }
  }
}
icon "host" {
  category "c"
  canvas 40 40
  part "p" {
    badge "dot" corner="se" scale=0.5
  }
}
"#;
    let c = compiled("b.ilx", src);
    let host = c.doc.icons.iter().position(|i| i.name == "host").unwrap();
    let shapes = preorder(&c.doc.icons[host].shapes);
    let IrShape::Group { matrix, .. } = shapes[0] else {
        panic!("a badge is a group")
    };
    let p = &c.provenance[host][1];
    assert_eq!(p.definition, "dot");
    assert!(near(
        apply(&p.map, [5.0, 5.0, 0.0]),
        points(shapes[1], matrix)[0]
    ));
}

#[test]
fn compile_one_and_compile_many_carry_it_for_their_icon_alone() {
    let src = include_str!("fixtures/sample.ilx");
    let files = [("sample.ilx".to_string(), src.to_string())];
    let all = compile(&files);
    let poe = all
        .doc
        .icons
        .iter()
        .position(|i| i.name == "switch-poe")
        .unwrap();
    let one = isolex::compile_one(&files, "switch-poe", &Default::default()).unwrap();
    assert_eq!(one.provenance.len(), 1);
    assert_eq!(one.provenance[0], all.provenance[poe]);
}

#[test]
fn a_3d_shape_gives_the_numbers_its_node_came_to() {
    let (_, sample) = FIXTURES[0];
    let c = compiled("sample.ilx", sample);
    let values = |icon: &str, prefix: &str| -> Vec<Vec<f64>> {
        let at = c.doc.icons.iter().position(|i| i.name == icon).unwrap();
        let mut out: Vec<Vec<f64>> = Vec::new();
        for p in &c.provenance[at] {
            if slice(sample, p.span).starts_with(prefix) && out.last() != Some(&p.values) {
                out.push(p.values.clone());
            }
        }
        out
    };
    // `u * 0.28`, with u at its default of 1
    assert_eq!(
        values("switch-l3", "box"),
        [vec![0.0, 0.0, 0.0, 2.0, 1.0, 0.28]]
    );
    // each copy of a repeat its own: twelve ports, a stepping along
    let ports = values("switch-l3", "facerect");
    assert_eq!(ports.len(), 12);
    for (i, p) in ports.iter().enumerate() {
        let a = 0.1 + i as f64 * (1.8 / 12.0);
        assert!((p[0] - a).abs() < 1e-9, "{p:?}");
        assert_eq!(p[1..], [1.0, 0.09, 0.11, 0.11]);
    }
    assert_eq!(
        values("switch-poe", "tile"),
        [vec![0.0, 0.0, 0.0, 2.0, 1.0]]
    );
    assert_eq!(
        values("switch-poe", "cylinder 0.5"),
        [vec![0.5, 0.5, 0.0, 0.4, 0.12]]
    );
    assert_eq!(values("switch-poe", "shadow"), [vec![0.5, 0.5, 0.48, 0.48]]);
    assert_eq!(
        values("switch-poe", "seg"),
        [vec![0.0, 0.0, 0.0, 2.0, 0.0, 0.0]]
    );
    // a flat shape gives none
    assert_eq!(values("switch-l3", "path"), [Vec::<f64>::new()]);
}
