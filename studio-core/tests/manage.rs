//! Managing a library (#72): its lexicon and palette blocks as Lexicon
//! Properties lists them, its files and definitions made, renamed and
//! moved through a session, each one undo step, a change on disk loaded
//! or flagged (in memory as on disk), and a new library made.

use std::sync::{Arc, Mutex};

use isolex_studio_core::lexicon::lexicon;
use isolex_studio_core::library::{create, Start};
use isolex_studio_core::session::{Answer, LibraryOp, Notice, Session, Sink};
use isolex_studio_core::watch::DiskChange;
use isolex_studio_core::Library;

const PALETTE: &str = include_str!("../../tests/fixtures/palette.ilx");
const SAMPLE: &str = include_str!("../../tests/fixtures/sample.ilx");

#[derive(Default)]
struct Window(Mutex<Vec<Notice>>);

impl Sink for Window {
    fn notify(&self, n: Notice) {
        self.0.lock().unwrap().push(n);
    }
}

impl Window {
    fn kinds(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|n| {
                serde_json::to_value(n).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }
}

fn session(files: &[(&str, &str)]) -> (Session, Arc<Window>) {
    let window = Arc::new(Window::default());
    let lib = Library::new(
        "/nowhere",
        files
            .iter()
            .map(|(p, t)| (p.to_string(), t.to_string()))
            .collect(),
    );
    (Session::in_memory(lib, window.clone()), window)
}

#[test]
fn the_lexicon_and_palette_blocks_as_written() {
    let rows = lexicon(&[("palette.ilx".to_string(), PALETTE.to_string())]);
    let lex = rows.lexicon.unwrap();
    assert_eq!(&PALETTE[lex.node.start..lex.node.start + 9], "lexicon {");
    let tokens = &lex.rows[0];
    assert_eq!(tokens.name, "tokens");
    assert_eq!(
        tokens.args,
        [
            "ink",
            "outline",
            "body-top",
            "body-left",
            "body-right",
            "accent"
        ]
    );
    assert_eq!(lex.rows[1].args, ["1.2", "1.5"]);
    let palette = rows.palette.unwrap();
    let ink = &palette.rows[0];
    assert_eq!(ink.name, "ink");
    assert_eq!(ink.props["light"], "#102a43");
    assert_eq!(ink.props["dark"], "#f0f4f8");
    assert_eq!(
        &PALETTE[ink.node.start..ink.node.start + ink.node.len],
        "ink light=\"#102a43\" dark=\"#f0f4f8\""
    );
    assert_eq!(rows.file, "palette.ilx");
    // the sample has no palette: one would go where its lexicon is
    let sample = lexicon(&[
        ("a.ilx".to_string(), "icon \"x\" {}".to_string()),
        ("sample.ilx".to_string(), SAMPLE.to_string()),
    ]);
    assert!(sample.palette.is_none());
    assert_eq!(sample.file, "sample.ilx");
    // a library with neither: lexicon.ilx when there is one
    let none = lexicon(&[
        ("a.ilx".to_string(), String::new()),
        ("lexicon.ilx".to_string(), String::new()),
    ]);
    assert_eq!((none.lexicon, none.file.as_str()), (None, "lexicon.ilx"));
}

#[test]
fn files_and_definitions_are_made_renamed_and_moved_each_one_undo_step() {
    let (mut s, _) = session(&[("sample.ilx", SAMPLE)]);
    let ok = |a: Answer| assert!(matches!(a, Answer::Applied { .. }), "{a:?}");
    ok(s.library_op(&LibraryOp::RenameDefinition {
        from: "switch-l2".into(),
        to: "switch-access".into(),
    }));
    let text = &s.library().files()["sample.ilx"];
    assert!(text.contains("icon \"switch-access\" {"));
    assert!(text.contains("extends \"switch-access\""));
    assert!(!text.contains("switch-l2\""));
    ok(s.library_op(&LibraryOp::NewFile {
        path: "racks.ilx".into(),
    }));
    ok(s.library_op(&LibraryOp::NewDefinition {
        file: "racks.ilx".into(),
        kind: "icon".into(),
        name: "rack".into(),
    }));
    assert!(s.library().files()["racks.ilx"].contains("icon \"rack\""));
    ok(s.library_op(&LibraryOp::MoveDefinition {
        name: "state-planned".into(),
        to: "racks.ilx".into(),
    }));
    assert!(s.library().files()["racks.ilx"].contains("icon \"state-planned\""));
    ok(s.library_op(&LibraryOp::MoveFile {
        from: "racks.ilx".into(),
        to: "racks/racks.ilx".into(),
    }));
    assert!(s.library().files().contains_key("racks/racks.ilx"));
    // a name taken is refused, offering one that is free
    let Answer::Refused { reason } = s.library_op(&LibraryOp::RenameDefinition {
        from: "switch-l3".into(),
        to: "switch-poe".into(),
    }) else {
        panic!()
    };
    assert!(reason.contains("switch-poe-2"), "{reason}");
    // each was one step: five undos and the sample is as it was
    for _ in 0..5 {
        s.undo().unwrap();
    }
    assert_eq!(s.library().files().len(), 1);
    assert_eq!(s.library().files()["sample.ilx"], SAMPLE);
}

#[test]
fn a_change_on_disk_loads_when_nothing_is_unsaved_and_is_flagged_when_something_is() {
    let (mut s, window) = session(&[("sample.ilx", SAMPLE)]);
    let changed = SAMPLE.replace("param ports 12", "param ports 16");
    s.outside(vec![DiskChange::Written(
        "sample.ilx".into(),
        changed.clone(),
    )]);
    // loaded, as one undoable step
    assert_eq!(s.library().files()["sample.ilx"], changed);
    assert!(window.kinds().contains(&"reloaded".to_string()));
    assert!(!s.library().is_modified());
    assert_eq!(
        s.library().history().last().map(|s| s.to_string()),
        Some("Changed on disk: sample.ilx".into())
    );

    // with an edit unsaved, the disk's change is flagged, not loaded
    let mine = changed.replace("param ports 16", "param ports 20");
    let at = changed.find("param ports 16").unwrap();
    s.ask(
        &isolex_studio_core::session::Ask::Type {
            file: "sample.ilx".into(),
            at,
            len: "param ports 16".len(),
            text: "param ports 20".into(),
        },
        "Type",
    );
    assert_eq!(s.library().files()["sample.ilx"], mine);
    let theirs = changed.replace("param ports 16", "param ports 24");
    s.outside(vec![DiskChange::Written(
        "sample.ilx".into(),
        theirs.clone(),
    )]);
    assert!(window.kinds().contains(&"out-of-sync".to_string()));
    assert_eq!(
        s.library().files()["sample.ilx"],
        mine,
        "the studio's copy kept"
    );

    // reloading discards the edit for what is on disk, undoably
    s.reload().unwrap();
    assert_eq!(s.library().files()["sample.ilx"], theirs);
    s.undo().unwrap();
    assert_eq!(s.library().files()["sample.ilx"], mine);
    // saving over it: the studio's copy is what is on disk now
    s.save().unwrap();
    assert!(!s.library().is_modified());
    s.reload().unwrap();
    assert_eq!(s.library().files()["sample.ilx"], mine);
}

#[test]
fn a_new_library_starts_empty_or_as_a_copy_and_never_over_one() {
    let root = std::env::temp_dir().join(format!("isolex-studio-new-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let empty = root.join("empty");
    create(&empty, &Start::Empty).unwrap();
    let lib = Library::open(&empty).unwrap();
    assert_eq!(
        lib.files().keys().collect::<Vec<_>>(),
        ["icons.ilx", "lexicon.ilx"]
    );
    let c = isolex::compile(&lib.sources());
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    assert_eq!(c.doc.icons.len(), 1);
    // a copy of it
    let copy = root.join("copy");
    create(&copy, &Start::Copy { of: empty.clone() }).unwrap();
    assert_eq!(Library::open(&copy).unwrap().files(), lib.files());
    // never over a library
    let err = create(&empty, &Start::Empty).unwrap_err();
    assert!(err.to_string().contains("holds a library already"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}
