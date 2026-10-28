//! One undo history over the whole library (#61): random sequences of
//! edits across files, undone and redone, against a record of every state
//! the library passed through.

use std::collections::BTreeMap;

use isolex_studio_core::{EditError, Library, Patch, Transaction};

/// xorshift64*, as the compiler's tests use: deterministic, no dependencies.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn text(&mut self) -> String {
        const PIECES: &[&str] = &[
            "box 0 0 0 1 1 1",
            "\n",
            "  ",
            "é",
            "// c",
            "{",
            "}",
            "x",
            "日",
        ];
        (0..self.below(5))
            .map(|_| PIECES[self.below(PIECES.len())])
            .collect()
    }
}

/// A library held in a temporary folder of its own, which saves write to.
fn library_in(tag: &str) -> Library {
    let root = std::env::temp_dir().join(format!(
        "isolex-studio-history-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut lib = library();
    lib = Library::new(root, lib.files().clone());
    lib
}

fn library() -> Library {
    let mut files = BTreeMap::new();
    files.insert(
        "a.ilx".to_string(),
        "icon \"a\" {\n  part \"p\" {}\n}\n".to_string(),
    );
    files.insert("lib/b.ilx".to_string(), "// b\n".to_string());
    files.insert("c.ilx".to_string(), String::new());
    Library::new("/nowhere", files)
}

/// A char boundary of `s` at or before `i`.
fn boundary(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// A random patch for `files`, valid or not.
fn patch(rng: &mut Rng, files: &BTreeMap<String, String>) -> Patch {
    let names: Vec<&String> = files.keys().collect();
    let existing = |rng: &mut Rng| {
        if names.is_empty() || rng.below(10) == 0 {
            "missing.ilx".to_string()
        } else {
            names[rng.below(names.len())].clone()
        }
    };
    let fresh = |rng: &mut Rng| format!("f{}/n{}.ilx", rng.below(3), rng.below(6));
    match rng.below(10) {
        0..=5 => {
            let file = existing(rng);
            let len_of = files.get(&file).map(|t| t.len()).unwrap_or(0);
            let body = files.get(&file).map(String::as_str).unwrap_or("");
            let at = boundary(body, rng.below(len_of + 2));
            let end = boundary(body, at + rng.below(6));
            // now and then a range past the end, or through a character
            let len = if rng.below(12) == 0 {
                len_of + 3
            } else {
                end - at
            };
            Patch::Splice {
                file,
                at,
                len,
                text: rng.text(),
            }
        }
        6 => Patch::Create {
            file: if rng.below(8) == 0 {
                existing(rng)
            } else {
                fresh(rng)
            },
            text: rng.text(),
        },
        7 => Patch::Delete {
            file: existing(rng),
        },
        8 => Patch::Rename {
            from: existing(rng),
            to: if rng.below(8) == 0 {
                existing(rng)
            } else {
                fresh(rng)
            },
        },
        _ => Patch::Create {
            file: "../out.ilx".to_string(),
            text: String::new(),
        },
    }
}

#[test]
fn undoing_everything_gives_the_original_and_redoing_everything_the_last() {
    for seed in 1..=40u64 {
        let mut rng = Rng(0x5EED_0061 ^ (seed << 17));
        let mut lib = library_in(&seed.to_string());
        let original = lib.files().clone();
        // every state an undo step leads back to, oldest first
        let mut states = vec![original.clone()];
        let mut saved_state = original.clone();
        let mut in_gesture = false;
        for _ in 0..200 {
            match rng.below(14) {
                0 if !in_gesture => {
                    lib.begin_gesture();
                    in_gesture = true;
                }
                1 if in_gesture => {
                    lib.end_gesture();
                    in_gesture = false;
                }
                2 => {
                    lib.save().unwrap();
                    saved_state = lib.files().clone();
                    assert!(!lib.is_modified(), "seed {seed}: saved");
                }
                _ => {
                    let before = lib.files().clone();
                    let steps_before = lib.history().len();
                    let n = 1 + rng.below(3);
                    let patches: Vec<Patch> =
                        (0..n).map(|_| patch(&mut rng, lib.files())).collect();
                    match lib.apply(Transaction::new("edit", patches)) {
                        Ok(()) => {
                            if lib.history().len() > steps_before {
                                states.push(lib.files().clone());
                            } else {
                                // joined the gesture's step
                                *states.last_mut().unwrap() = lib.files().clone();
                            }
                        }
                        Err(_) => {
                            assert_eq!(
                                lib.files(),
                                &before,
                                "seed {seed}: a refused edit changed nothing"
                            );
                            assert_eq!(lib.history().len(), steps_before, "seed {seed}");
                        }
                    }
                }
            }
            assert_eq!(
                lib.is_modified(),
                lib.files() != &saved_state,
                "seed {seed}: modified"
            );
        }
        lib.end_gesture();
        let last = lib.files().clone();
        assert_eq!(states.len(), lib.history().len() + 1, "seed {seed}");
        // back through every state, to the original
        for k in (0..states.len() - 1).rev() {
            lib.undo().expect("a step to undo");
            assert_eq!(lib.files(), &states[k], "seed {seed}: undo to step {k}");
            assert_eq!(
                lib.is_modified(),
                lib.files() != &saved_state,
                "seed {seed}"
            );
        }
        assert!(lib.undo().is_none());
        assert_eq!(lib.files(), &original, "seed {seed}");
        // and forward again, to the last
        for state in &states[1..] {
            lib.redo().expect("a step to redo");
            assert_eq!(lib.files(), state, "seed {seed}: redo");
        }
        assert!(lib.redo().is_none());
        assert_eq!(lib.files(), &last, "seed {seed}");
        let _ = std::fs::remove_dir_all(lib.root());
    }
}

#[test]
fn a_gesture_is_one_step_and_a_refused_edit_changes_nothing() {
    let mut lib = library();
    let text = lib.files()["a.ilx"].clone();
    lib.begin_gesture();
    for i in 0..5 {
        lib.apply(Transaction::new(
            "Move box",
            vec![Patch::Splice {
                file: "a.ilx".into(),
                at: 0,
                len: 0,
                text: i.to_string(),
            }],
        ))
        .unwrap();
    }
    lib.end_gesture();
    assert_eq!(lib.history(), ["Move box"]);
    assert_eq!(lib.files()["a.ilx"], format!("43210{text}"));
    assert_eq!(lib.undo().as_deref(), Some("Move box"));
    assert_eq!(lib.files()["a.ilx"], text);
    assert!(!lib.is_modified());

    // a transaction is whole or nothing: its second patch is refused
    let err = lib.apply(Transaction::new(
        "Rename",
        vec![
            Patch::Rename {
                from: "a.ilx".into(),
                to: "z.ilx".into(),
            },
            Patch::Delete {
                file: "nope.ilx".into(),
            },
        ],
    ));
    assert_eq!(err, Err(EditError::NoSuchFile("nope.ilx".into())));
    assert!(lib.files().contains_key("a.ilx") && !lib.files().contains_key("z.ilx"));
    assert!(lib.can_redo(), "a refused edit leaves the redo history");

    for bad in [
        "",
        "/abs.ilx",
        "../up.ilx",
        "a\\b.ilx",
        "c:x.ilx",
        "notes.txt",
        "a//b.ilx",
    ] {
        let r = lib.apply(Transaction::new(
            "Create",
            vec![Patch::Create {
                file: bad.into(),
                text: String::new(),
            }],
        ));
        assert_eq!(r, Err(EditError::BadPath(bad.into())), "{bad:?}");
    }
}
