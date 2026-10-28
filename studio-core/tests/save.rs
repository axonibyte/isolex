//! Saving a library at once, and recovering a save that was interrupted
//! (#61): the folder is always all old or all new.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use isolex_studio_core::save::{recover, Faults, JOURNAL, TEMP_SUFFIX};
use isolex_studio_core::{Library, Patch, Transaction};

fn folder(tag: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("isolex-studio-save-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(
        root.join("a.ilx"),
        "icon \"a\" { category \"c\"; canvas 9 9 }\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/b.ilx"), "// b\n").unwrap();
    std::fs::write(root.join("old.ilx"), "// to be removed\n").unwrap();
    std::fs::write(root.join("move.ilx"), "// to be moved\n").unwrap();
    std::fs::write(root.join("notes.txt"), "not part of the library").unwrap();
    root
}

/// Every file under `root`, by path with `/`, and its text.
fn disk(root: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                todo.push(p);
            } else {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read_to_string(&p).unwrap());
            }
        }
    }
    out
}

/// Edits of every kind: a splice, a new file in a new folder, a removal
/// and a move.
fn edit(lib: &mut Library) {
    lib.apply(Transaction::new(
        "everything",
        vec![
            Patch::Splice {
                file: "a.ilx".into(),
                at: 0,
                len: 0,
                text: "// edited\n".into(),
            },
            Patch::Create {
                file: "new/c.ilx".into(),
                text: "// c\n".into(),
            },
            Patch::Delete {
                file: "old.ilx".into(),
            },
            Patch::Rename {
                from: "move.ilx".into(),
                to: "lib/moved.ilx".into(),
            },
        ],
    ))
    .unwrap();
}

/// What the folder should hold: the library's files, and what is not.
fn expected(lib: &Library) -> BTreeMap<String, String> {
    let mut want = lib.files().clone();
    want.insert("notes.txt".into(), "not part of the library".into());
    want
}

#[test]
fn opening_reads_the_ilx_files_by_their_paths() {
    let root = folder("open");
    let lib = Library::open(&root).unwrap();
    let names: Vec<&str> = lib.files().keys().map(String::as_str).collect();
    assert_eq!(names, ["a.ilx", "lib/b.ilx", "move.ilx", "old.ilx"]);
    assert!(!lib.is_modified());
    // as the compiler takes them, in SPEC §1's order
    let order: Vec<String> = lib.sources().into_iter().map(|(p, _)| p).collect();
    assert_eq!(order, ["a.ilx", "lib/b.ilx", "move.ilx", "old.ilx"]);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_save_writes_every_change_at_once() {
    let root = folder("whole");
    let mut lib = Library::open(&root).unwrap();
    edit(&mut lib);
    assert!(lib.is_modified());
    let saved = lib.save().unwrap();
    assert_eq!(saved.written, ["a.ilx", "lib/moved.ilx", "new/c.ilx"]);
    assert_eq!(saved.removed, ["move.ilx", "old.ilx"]);
    assert_eq!(
        disk(&root),
        expected(&lib),
        "no journal, no temporary file left"
    );
    assert!(!lib.is_modified());
    // undoing back past the save is a change; redoing returns to it
    lib.undo();
    assert!(lib.is_modified());
    lib.redo();
    assert!(!lib.is_modified());
    // a save with nothing changed does nothing
    let again = lib.save().unwrap();
    assert!(again.written.is_empty() && again.removed.is_empty());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_save_interrupted_before_its_journal_did_not_happen() {
    let root = folder("before");
    let before = disk(&root);
    let mut lib = Library::open(&root).unwrap();
    edit(&mut lib);
    assert!(lib
        .save_with(Faults {
            before_journal: true,
            ..Faults::default()
        })
        .is_err());
    assert!(lib.is_modified(), "nothing was saved");
    assert!(
        disk(&root).keys().any(|p| p.ends_with(TEMP_SUFFIX)),
        "temporary files were left"
    );
    // opening again finds the old library, and no temporary file
    let reopened = Library::open(&root).unwrap();
    assert_eq!(disk(&root), before);
    assert_eq!(reopened.files().len(), 4);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_save_interrupted_after_its_journal_is_finished_on_opening() {
    // after none of the renames, after one, and after each but the last
    for k in 0..3 {
        let root = folder(&format!("after-{k}"));
        let mut lib = Library::open(&root).unwrap();
        edit(&mut lib);
        let want = expected(&lib);
        assert!(lib
            .save_with(Faults {
                after_renames: Some(k),
                ..Faults::default()
            })
            .is_err());
        let mid = disk(&root);
        assert!(
            mid.contains_key(JOURNAL),
            "k={k}: the journal stays until it is done"
        );
        assert_ne!(mid, want, "k={k}: half done");
        // opening finishes it
        let reopened = Library::open(&root).unwrap();
        assert_eq!(disk(&root), want, "k={k}");
        assert_eq!(reopened.files(), lib.files(), "k={k}");
        assert!(!reopened.is_modified());
        // and recovering again finds nothing to do
        assert!(!recover(&root).unwrap());
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn a_journal_that_is_not_one_is_an_error_not_a_guess() {
    let root = folder("bad-journal");
    std::fs::write(root.join(JOURNAL), "not json").unwrap();
    let err = Library::open(&root).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("is not a save journal"), "{err}");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_file_renamed_only_in_case_survives_the_save() {
    let root = folder("case");
    std::fs::write(root.join("Rack.ilx"), "// rack\n").unwrap();
    let mut lib = Library::open(&root).unwrap();
    lib.apply(Transaction::new(
        "Rename",
        vec![
            Patch::Rename {
                from: "Rack.ilx".into(),
                to: "rack.ilx".into(),
            },
            Patch::Splice {
                file: "rack.ilx".into(),
                at: 0,
                len: 0,
                text: "// renamed\n".into(),
            },
        ],
    ))
    .unwrap();
    lib.save().unwrap();
    // whether this file system tells the two names apart or not, the file
    // is there under its new name, with its new text, and only once
    let on_disk = disk(&root);
    assert_eq!(
        on_disk.get("rack.ilx").map(String::as_str),
        Some("// renamed\n// rack\n"),
        "{on_disk:?}"
    );
    assert!(!on_disk.contains_key("Rack.ilx"));
    // and recovering a journal of it again changes nothing
    assert_eq!(Library::open(&root).unwrap().files(), lib.files());
    std::fs::remove_dir_all(&root).unwrap();
}
