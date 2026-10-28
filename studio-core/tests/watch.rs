//! Watching the folder (#63): what a change on disk does to the library,
//! with unsaved edits and without, and the studio's own saves.

use std::collections::BTreeMap;
use std::path::PathBuf;

use isolex_studio_core::{
    DiskChange, DiskOutcome, Library, Patch, PollingWatcher, Transaction, Watcher,
};

fn files() -> BTreeMap<String, String> {
    [("a.ilx", "// a\n"), ("b.ilx", "// b\n")]
        .into_iter()
        .map(|(p, t)| (p.to_string(), t.to_string()))
        .collect()
}

fn edited(mut lib: Library) -> Library {
    lib.apply(Transaction::new(
        "edit",
        vec![Patch::Splice {
            file: "b.ilx".into(),
            at: 0,
            len: 0,
            text: "// mine\n".into(),
        }],
    ))
    .unwrap();
    lib
}

fn written(p: &str, t: &str) -> DiskChange {
    DiskChange::Written(p.into(), t.into())
}

#[test]
fn what_a_change_on_disk_does_as_a_table() {
    use DiskOutcome::*;
    // (with unsaved edits, the change, what it comes to, a.ilx after, modified after)
    let cases: Vec<(bool, DiskChange, DiskOutcome, Option<&str>, bool)> = vec![
        // the studio's own save, or a write of what is saved: nothing
        (
            false,
            written("a.ilx", "// a\n"),
            Nothing,
            Some("// a\n"),
            false,
        ),
        (
            true,
            written("a.ilx", "// a\n"),
            Nothing,
            Some("// a\n"),
            true,
        ),
        // a file removed that the library never had: nothing
        (
            false,
            DiskChange::Removed("z.ilx".into()),
            Nothing,
            Some("// a\n"),
            false,
        ),
        // no unsaved edits: loaded, and still unmodified
        (
            false,
            written("a.ilx", "// theirs\n"),
            Reloaded(vec!["a.ilx".into()]),
            Some("// theirs\n"),
            false,
        ),
        (
            false,
            written("c.ilx", "// new\n"),
            Reloaded(vec!["c.ilx".into()]),
            Some("// a\n"),
            false,
        ),
        (
            false,
            DiskChange::Removed("a.ilx".into()),
            Reloaded(vec!["a.ilx".into()]),
            None,
            false,
        ),
        // unsaved edits: out of sync, and nothing loaded
        (
            true,
            written("a.ilx", "// theirs\n"),
            OutOfSync(vec!["a.ilx".into()]),
            Some("// a\n"),
            true,
        ),
        (
            true,
            DiskChange::Removed("a.ilx".into()),
            OutOfSync(vec!["a.ilx".into()]),
            Some("// a\n"),
            true,
        ),
    ];
    for (dirty, change, want, a_after, modified) in cases {
        let what = format!("dirty={dirty} {change:?}");
        let lib = Library::new("/nowhere", files());
        let mut lib = if dirty { edited(lib) } else { lib };
        let steps = lib.history().len();
        assert_eq!(lib.on_disk(vec![change]), want, "{what}");
        assert_eq!(
            lib.files().get("a.ilx").map(String::as_str),
            a_after,
            "{what}"
        );
        assert_eq!(lib.is_modified(), modified, "{what}");
        if let Reloaded(_) = want {
            // one undoable step, which brings back what the library had
            assert_eq!(lib.history().len(), steps + 1, "{what}");
            assert!(
                lib.history()
                    .last()
                    .unwrap()
                    .starts_with("Changed on disk: "),
                "{what}"
            );
            lib.undo();
            assert_eq!(lib.files(), &files(), "{what}");
            assert!(lib.is_modified(), "{what}: the disk differs from it now");
        } else {
            assert_eq!(lib.history().len(), steps, "{what}");
        }
    }
}

#[test]
fn out_of_sync_then_saved_writes_over_the_outside_change() {
    let mut lib = edited(Library::new("/nowhere", files()));
    assert_eq!(
        lib.on_disk(vec![written("a.ilx", "// theirs\n")]),
        DiskOutcome::OutOfSync(vec!["a.ilx".into()])
    );
    // a save now writes a.ilx back as the library has it, with b.ilx
    let paths: Vec<String> = lib.changes().iter().map(|c| c.path().to_string()).collect();
    assert_eq!(paths, ["a.ilx", "b.ilx"]);
}

fn folder(tag: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("isolex-studio-watch-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for (p, t) in files() {
        std::fs::write(root.join(p), t).unwrap();
    }
    root
}

#[test]
fn outside_writes_and_the_studios_own_saves_through_the_polling_watcher() {
    let root = folder("poll");
    let mut lib = Library::open(&root).unwrap();
    let mut watcher = PollingWatcher::new(&root).unwrap();
    assert!(watcher.poll().unwrap().is_empty(), "nothing has changed");

    // written outside, with nothing unsaved: loaded
    std::fs::write(root.join("a.ilx"), "// written by git\n").unwrap();
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(root.join("lib/c.ilx"), "// new\n").unwrap();
    let changes = watcher.poll().unwrap();
    assert_eq!(
        lib.on_disk(changes),
        DiskOutcome::Reloaded(vec!["a.ilx".into(), "lib/c.ilx".into()])
    );
    assert_eq!(lib.files()["a.ilx"], "// written by git\n");
    assert!(!lib.is_modified());

    // the studio's own save: seen, and nothing
    lib.apply(Transaction::new(
        "edit",
        vec![Patch::Splice {
            file: "b.ilx".into(),
            at: 0,
            len: 0,
            text: "// saved here, longer\n".into(),
        }],
    ))
    .unwrap();
    lib.save().unwrap();
    let changes = watcher.poll().unwrap();
    assert_eq!(changes.len(), 1, "the watcher sees the save: {changes:?}");
    assert_eq!(lib.on_disk(changes), DiskOutcome::Nothing);

    // written outside with an edit unsaved: out of sync, then reloaded
    lib.apply(Transaction::new(
        "edit",
        vec![Patch::Splice {
            file: "a.ilx".into(),
            at: 0,
            len: 0,
            text: "// unsaved\n".into(),
        }],
    ))
    .unwrap();
    std::fs::remove_file(root.join("lib/c.ilx")).unwrap();
    let changes = watcher.poll().unwrap();
    assert_eq!(
        lib.on_disk(changes),
        DiskOutcome::OutOfSync(vec!["lib/c.ilx".into()])
    );
    assert!(
        lib.files().contains_key("lib/c.ilx"),
        "nothing is loaded while out of sync"
    );
    let reloaded = lib.reload_from_disk().unwrap();
    assert_eq!(
        reloaded,
        ["a.ilx", "lib/c.ilx"],
        "the unsaved edit is discarded, and the removal loaded"
    );
    assert!(!lib.is_modified());
    assert_eq!(lib.history().last().copied(), Some("Reload from disk"));
    // which is undoable
    lib.undo();
    assert!(lib.files()["a.ilx"].starts_with("// unsaved\n"));
    std::fs::remove_dir_all(&root).unwrap();
}
