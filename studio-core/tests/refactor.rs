//! Renaming definitions, and file and definition operations (#65).

use std::collections::{BTreeMap, BTreeSet};

use isolex_studio_core::refactor::{
    create_file, move_definition, move_file, new_definition, rename_definition, RefactorError,
};
use isolex_studio_core::Library;

const FIXTURES: &[(&str, &str)] = &[
    (
        "sample.ilx",
        include_str!("../../tests/fixtures/sample.ilx"),
    ),
    ("graph.ilx", include_str!("../../tests/fixtures/graph.ilx")),
    ("text.ilx", include_str!("../../tests/fixtures/text.ilx")),
];

fn library(files: &[(&str, &str)]) -> Library {
    let files: BTreeMap<String, String> = files
        .iter()
        .map(|(p, t)| (p.to_string(), t.to_string()))
        .collect();
    Library::new("/nowhere", files)
}

/// Each emitted icon's IR as JSON, by name.
fn drawn(lib: &Library) -> BTreeMap<String, String> {
    let c = isolex::compile(&lib.sources());
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    c.doc
        .icons
        .iter()
        .map(|i| {
            let mut i = i.clone();
            let name = std::mem::take(&mut i.name);
            (name, serde_json::to_string(&i).unwrap())
        })
        .collect()
}

/// The graph's edges, without where they are written.
fn edges(lib: &Library) -> BTreeSet<String> {
    isolex::graph(&lib.sources())
        .edges
        .iter()
        .map(|e| format!("{:?} {} -> {} {:?}", e.kind, e.from, e.to, e.variant))
        .collect()
}

#[test]
fn every_definition_renamed_draws_the_same_and_one_undo_restores_it() {
    let mut renamed = 0;
    for (file, text) in FIXTURES {
        let names: Vec<String> = isolex::graph(&[(file.to_string(), text.to_string())])
            .definitions
            .iter()
            .map(|d| d.name.clone())
            .collect();
        for from in &names {
            let to = format!("{from}-renamed");
            let mut lib = library(&[(file, text)]);
            let before = lib.files().clone();
            let drawn_before = drawn(&lib);
            let graph_before = isolex::graph(&lib.sources());
            let edges_before = edges(&lib);
            let tx = rename_definition(&lib, from, &to).unwrap();
            lib.apply(tx).unwrap();
            // the icons it emits, renamed in order; every other the same
            let graph_after = isolex::graph(&lib.sources());
            let emits = |g: &isolex::graph::Graph, n: &str| {
                g.definitions
                    .iter()
                    .find(|d| d.name == n)
                    .unwrap()
                    .emits
                    .clone()
            };
            let mapping: BTreeMap<String, String> = emits(&graph_before, from)
                .into_iter()
                .zip(emits(&graph_after, &to))
                .collect();
            let want: BTreeMap<String, String> = drawn_before
                .into_iter()
                .map(|(n, ir)| (mapping.get(&n).cloned().unwrap_or(n), ir))
                .collect();
            assert_eq!(drawn(&lib), want, "{file}: {from} renamed draws the same");
            // the graph, with the name changed
            let swap = |s: &str| {
                s.replace(&format!(" {from} "), &format!(" {to} "))
                    .replace(&format!("-> {from} "), &format!("-> {to} "))
            };
            let want_edges: BTreeSet<String> = edges_before
                .iter()
                .map(|e| swap(&format!("{e} ")))
                .map(|e| e.trim_end().to_string())
                .collect();
            assert_eq!(edges(&lib), want_edges, "{file}: {from}'s edges");
            assert!(graph_after.definitions.iter().any(|d| d.name == to));
            assert!(
                lib.files()[*file].contains(&format!("\"{to}\"")),
                "{file}: written quoted"
            );
            assert!(!graph_after.definitions.iter().any(|d| &d.name == from));
            // one undo restores every file
            assert_eq!(lib.history().len(), 1);
            lib.undo();
            assert_eq!(lib.files(), &before, "{file}: {from}");
            renamed += 1;
        }
    }
    assert!(renamed > 25, "{renamed}");
}

#[test]
fn a_rename_across_files_is_one_step() {
    let mut lib = library(&[
        ("base.ilx", "// the chassis\nbase \"chassis\" {\n  category \"c\"\n  footprint 1 1 1\n  part \"b\" { box 0 0 0 1 1 1 }\n}\n"),
        ("icons/a.ilx", "icon \"a\" extends=\"chassis\" {\n}\n"),
        ("icons/b.ilx", "icon \"b\" {\n  extends \"chassis\"\n}\n"),
    ]);
    let before = lib.files().clone();
    lib.apply(rename_definition(&lib, "chassis", "rack unit").unwrap())
        .unwrap();
    assert_eq!(lib.files()["base.ilx"], "// the chassis\nbase \"rack unit\" {\n  category \"c\"\n  footprint 1 1 1\n  part \"b\" { box 0 0 0 1 1 1 }\n}\n");
    assert_eq!(
        lib.files()["icons/a.ilx"],
        "icon \"a\" extends=\"rack unit\" {\n}\n"
    );
    assert_eq!(
        lib.files()["icons/b.ilx"],
        "icon \"b\" {\n  extends \"rack unit\"\n}\n"
    );
    assert_eq!(lib.history(), ["Rename chassis to rack unit"]);
    lib.undo();
    assert_eq!(lib.files(), &before);
}

#[test]
fn a_name_taken_is_refused_with_one_that_is_free() {
    let lib = library(&[FIXTURES[1]]);
    // a definition's name, and an icon a variant emits
    assert_eq!(
        rename_definition(&lib, "switch", "server"),
        Err(RefactorError::NameTaken {
            name: "server".into(),
            free: "server-2".into()
        })
    );
    assert_eq!(
        rename_definition(&lib, "switch", "server-down"),
        Err(RefactorError::NameTaken {
            name: "server-down".into(),
            free: "server-down-2".into()
        })
    );
    assert_eq!(
        rename_definition(&lib, "nope", "x"),
        Err(RefactorError::NoSuchDefinition("nope".into()))
    );
    assert_eq!(
        new_definition(&lib, "graph.ilx", "icon", "led"),
        Err(RefactorError::NameTaken {
            name: "led".into(),
            free: "led-2".into()
        })
    );
}

#[test]
fn files_and_definitions_are_made_and_moved_undoably() {
    let mut lib = library(&[FIXTURES[1]]);
    let before = drawn(&lib);
    // a new file, then a new icon in it, which compiles
    lib.apply(create_file(&lib, "more/new.ilx", "// more\n").unwrap())
        .unwrap();
    lib.apply(new_definition(&lib, "more/new.ilx", "icon", "crate").unwrap())
        .unwrap();
    assert!(
        lib.files()["more/new.ilx"].starts_with("// more\n\nicon \"crate\" {"),
        "{:?}",
        lib.files()["more/new.ilx"]
    );
    let mut want = before.clone();
    want.insert("crate".into(), drawn(&lib)["crate"].clone());
    assert_eq!(drawn(&lib), want);
    // a definition moved to another file, with its comment, draws the same
    lib.apply(move_definition(&lib, "led", "more/new.ilx").unwrap())
        .unwrap();
    assert!(lib.files()["more/new.ilx"].ends_with("fragment \"led\" {\n  part \"dot\" {\n    facerect \"front\" 0.1 1 0.1 0.1 0.1 fill=\"accent\"\n  }\n}\n"));
    assert!(!lib.files()["graph.ilx"].contains("fragment \"led\""));
    assert_eq!(drawn(&lib), want);
    // a file moved: nothing refers to files, so nothing else changes
    lib.apply(move_file(&lib, "more/new.ilx", "lib/new.ilx").unwrap())
        .unwrap();
    assert_eq!(drawn(&lib), want);
    assert_eq!(
        move_file(&lib, "graph.ilx", "lib/new.ilx"),
        Err(RefactorError::FileExists {
            path: "lib/new.ilx".into(),
            free: "lib/new-2.ilx".into()
        })
    );
    assert_eq!(
        create_file(&lib, "../out.ilx", ""),
        Err(RefactorError::BadPath("../out.ilx".into()))
    );
    // each one a step
    assert_eq!(lib.history().len(), 4);
    while lib.undo().is_some() {}
    assert_eq!(lib.files().len(), 1);
    assert_eq!(drawn(&lib), before);
}
