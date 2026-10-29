//! What the studio's core says of the compiler's sample library (its
//! graph, each definition's outline, each icon's sources), kept as a
//! golden (`tests/fixtures/sample.studio.golden.json`): the data the
//! interface's tests select and list parts by (`studio/ui/src/lib/
//! select.test.ts`, `App.parts.test.ts`), checked here against the core.
//! UPDATE_GOLDENS=1 writes it again.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

use isolex_studio_core::compile::{Compiler, Isolex, Origin, Outcome};
use isolex_studio_core::outline::{outline, Outline};
use serde::Serialize;

#[derive(Serialize)]
struct Sample {
    graph: isolex::graph::Graph,
    outlines: BTreeMap<String, Outline>,
    sources: BTreeMap<String, Vec<Origin>>,
}

#[test]
fn the_sample_as_the_interface_is_told_it() {
    let files = vec![(
        "sample.ilx".to_string(),
        include_str!("../../tests/fixtures/sample.ilx").to_string(),
    )];
    let (ready, _, icons) = Isolex.prepare(&files);
    let sources = icons
        .iter()
        .map(
            |name| match Isolex.icon(&ready, name, &AtomicBool::new(false)) {
                Some(Outcome::Drawn { drawing, .. }) => (name.clone(), drawing.sources.clone()),
                other => panic!("{name}: {other:?}"),
            },
        )
        .collect();
    let graph = isolex::graph(&files);
    let outlines = graph
        .definitions
        .iter()
        .filter_map(|d| outline(&files, &d.name).map(|o| (d.name.clone(), o)))
        .collect();
    let json = serde_json::to_string_pretty(&Sample {
        graph,
        outlines,
        sources,
    })
    .unwrap()
        + "\n";
    let golden = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/sample.studio.golden.json"
    );
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(golden, &json).unwrap();
    }
    let want = std::fs::read_to_string(golden)
        .expect("golden missing: run with UPDATE_GOLDENS=1 once, then review the diff");
    assert_eq!(
        json,
        want.replace("\r\n", "\n"),
        "the studio's sample drifted from its golden"
    );
}
