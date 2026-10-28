//! A library as a graph (#57): every definition, and every reference
//! from one to another with where its name is written.

use isolex::graph::EdgeKind;

const FIXTURE: &str = include_str!("fixtures/graph.ilx");

fn graph() -> isolex::graph::Graph {
    isolex::graph(&[("graph.ilx".to_string(), FIXTURE.to_string())])
}

#[test]
fn the_fixture_matches_its_golden() {
    let got = serde_json::to_string_pretty(&graph()).unwrap();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/graph.golden.json"
    );
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(path, &got).unwrap();
    }
    let want = std::fs::read_to_string(path)
        .expect("golden missing: run with UPDATE_GOLDENS=1 once, then review the diff");
    assert_eq!(got, want, "the graph drifted from its golden");
}

#[test]
fn every_kind_of_edge_is_there_and_each_points_at_the_name_written() {
    let g = graph();
    for kind in [
        EdgeKind::Extends,
        EdgeKind::Insert,
        EdgeKind::Badge,
        EdgeKind::Axis,
        EdgeKind::Font,
    ] {
        assert!(g.edges.iter().any(|e| e.kind == kind), "no {kind:?} edge");
    }
    for e in &g.edges {
        let written = &FIXTURE[e.at.start..e.at.start + e.at.len];
        assert_eq!(written, format!("\"{}\"", e.to), "{e:?}");
    }
    // extends both ways, an insert from a fragment, a badge from a variant
    let has = |kind, from: &str, to: &str| {
        g.edges
            .iter()
            .any(|e| e.kind == kind && e.from == from && e.to == to)
    };
    assert!(has(EdgeKind::Extends, "server", "chassis"));
    assert!(has(EdgeKind::Extends, "switch", "chassis"));
    assert!(has(EdgeKind::Insert, "led-row", "led"));
    assert!(has(EdgeKind::Axis, "mono", "weight"));
    let badge = g.edges.iter().find(|e| e.kind == EdgeKind::Badge).unwrap();
    assert_eq!(
        (badge.from.as_str(), badge.variant.as_deref()),
        ("state", Some("down"))
    );
    // an icon's emitted names, its variants' included
    let server = g.definitions.iter().find(|d| d.name == "server").unwrap();
    assert_eq!(server.emits, ["server", "server-down"]);
}

#[test]
fn a_reference_to_nothing_is_still_an_edge() {
    let src = "icon \"a\" extends=\"missing\" {\n  category \"c\"\n  canvas 9 9\n}\n";
    let g = isolex::graph(&[("a.ilx".to_string(), src.to_string())]);
    assert_eq!(g.edges.len(), 1);
    assert_eq!(g.edges[0].to, "missing");
    assert_eq!(&src[g.edges[0].at.start..][..9], "\"missing\"");
}
