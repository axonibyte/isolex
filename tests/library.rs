//! A lexicon's folder read through the library (#54): SPEC §1's order,
//! and what is loaded from a folder.

use std::path::Path;

use isolex::library::{compare, load, order};

#[test]
fn files_are_ordered_directory_by_directory() {
    // (earlier, later) under SPEC §1
    for (a, b) in [
        ("a/x.ilx", "a-b/x.ilx"), // '/' is 0x2F and '-' 0x2D: whole strings sort the other way
        ("a/z.ilx", "a-b/a.ilx"),
        ("a.ilx", "b.ilx"),
        ("b/x.ilx", "b.ilx"), // a directory's files where its name sorts
        ("B.ilx", "a.ilx"),   // bytes, not letters
        ("a/b/c.ilx", "a/c.ilx"),
        ("x.ilx", "é.ilx"),
        ("lexicon.ilx", "library/rack.ilx"),
    ] {
        assert_eq!(compare(a, b), std::cmp::Ordering::Less, "{a} before {b}");
        assert_eq!(compare(b, a), std::cmp::Ordering::Greater, "{b} after {a}");
    }
    assert_eq!(compare("a/x.ilx", "a/x.ilx"), std::cmp::Ordering::Equal);
    assert_eq!(
        order(vec![
            "b.ilx",
            "a-b/x.ilx",
            "b/y.ilx",
            "a/x.ilx",
            "a/b/z.ilx"
        ]),
        ["a/b/z.ilx", "a/x.ilx", "a-b/x.ilx", "b/y.ilx", "b.ilx"]
    );
}

#[test]
fn a_folder_loads_its_ilx_files_in_order() {
    let dir = std::env::temp_dir().join(format!("isolex-library-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in [
        ("b.ilx", "// b\n"),
        ("a-b/x.ilx", "// a-b/x\n"),
        ("a/x.ilx", "// a/x\n"),
        ("a/deeper/y.ilx", "// a/deeper/y\n"),
        ("a/notes.txt", "not a lexicon file"),
        ("c.ilx.bak", "nor this"),
    ] {
        let file = dir.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    let files = load(&dir).unwrap();
    let texts: Vec<&str> = files.iter().map(|(_, t)| t.as_str()).collect();
    assert_eq!(
        texts,
        ["// a/deeper/y\n", "// a/x\n", "// a-b/x\n", "// b\n"]
    );
    // named as the directory joined with the path, as diagnostics name them
    assert_eq!(files[3].0, dir.join("b.ilx").display().to_string());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_folder_without_ilx_files_or_that_is_not_there_is_an_error() {
    let dir = std::env::temp_dir().join(format!("isolex-library-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/readme.txt"), "").unwrap();
    let err = load(&dir).unwrap_err();
    assert!(err.starts_with("no .ilx files under "), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
    let err = load(Path::new("no/such/folder")).unwrap_err();
    assert!(err.starts_with("cannot read no/such/folder: "), "{err}");
}
