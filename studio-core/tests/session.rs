//! A library open in the studio, as the shell serves it (#66): each command
//! against a window of the test's own, which records what it is told.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use isolex_studio_core::session::{Answer, Ask, JsonValue, Node, Notice, Session, Sink};

/// A window that keeps every notice, and wakes whoever waits for one.
#[derive(Default)]
struct Window {
    seen: Mutex<Vec<Notice>>,
    arrived: Condvar,
}

impl Sink for Window {
    fn notify(&self, notice: Notice) {
        self.seen.lock().unwrap().push(notice);
        self.arrived.notify_all();
    }
}

impl Window {
    /// Waits (up to a minute) for a notice `want` accepts, among those
    /// after the first `from`; returns its position.
    fn wait(&self, from: usize, want: impl Fn(&Notice) -> bool) -> usize {
        let until = Instant::now() + Duration::from_secs(60);
        let mut seen = self.seen.lock().unwrap();
        loop {
            if let Some(i) = seen.iter().skip(from).position(&want) {
                return from + i;
            }
            let left = until.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "timed out waiting; saw {:?}",
                seen.iter().skip(from).map(kind).collect::<Vec<_>>()
            );
            seen = self.arrived.wait_timeout(seen, left).unwrap().0;
        }
    }

    fn len(&self) -> usize {
        self.seen.lock().unwrap().len()
    }

    fn last_library(&self) -> Notice {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|n| matches!(n, Notice::Library { .. }))
            .cloned()
            .unwrap()
    }
}

fn kind(n: &Notice) -> String {
    serde_json::to_value(n).unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_string()
}

fn folder(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "isolex-studio-session-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("sample.ilx"),
        include_str!("../../tests/fixtures/sample.ilx"),
    )
    .unwrap();
    root
}

/// The node in sample.ilx whose source starts with `prefix`, as the
/// interface names it.
fn node(s: &Session, prefix: &str) -> Node {
    let text = &s.library().files()["sample.ilx"];
    let start = text.find(prefix).unwrap_or_else(|| panic!("no {prefix}"));
    let doc = isolex::kdl::Document::parse(text).unwrap();
    let mut todo: Vec<_> = doc.nodes().collect();
    while let Some(n) = todo.pop() {
        if n.span().offset() == start {
            return Node {
                file: "sample.ilx".into(),
                start,
                len: n.span().len(),
            };
        }
        todo.extend(n.children().into_iter().flat_map(|c| c.nodes()));
    }
    panic!("{prefix} is not a node")
}

#[test]
fn opening_editing_choosing_undoing_and_saving() {
    let root = folder("edit");
    let window = Arc::new(Window::default());
    let mut s = Session::open(&root, window.clone()).unwrap();
    // what opening tells: the library, its graph, then every icon compiled
    let Notice::Library {
        revision: 0,
        modified: false,
        ..
    } = window.last_library()
    else {
        panic!()
    };
    window.wait(0, |n| matches!(n, Notice::Graph { .. }));
    let compiled = window.wait(0, |n| matches!(n, Notice::Compiled { revision: 0, .. }));
    // the sample's grant is a warning, not an error, and it has no palette
    let Notice::Compiled {
        errors,
        warnings,
        palette,
        ..
    } = window.seen.lock().unwrap()[compiled].clone()
    else {
        unreachable!()
    };
    assert!(errors.is_empty());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(palette.is_none());
    // and its text, for the Source tab
    let src = s.source("sample.ilx").unwrap();
    assert_eq!(
        (src.revision, src.text.as_str()),
        (0, include_str!("../../tests/fixtures/sample.ilx"))
    );
    assert!(s.source("nope.ilx").is_none());
    window.wait(
        compiled,
        |n| matches!(n, Notice::Icon { name, .. } if name == "switch-poe"),
    );

    // a literal set: applied, one step, and compiled again
    let mark = window.len();
    let seg = node(&s, "seg 0 0 0 2 0 0");
    let answer = s.ask(
        &Ask::SetArg {
            node: seg,
            index: 3,
            value: JsonValue::Number(1.5),
        },
        "Move segment",
    );
    let Answer::Applied { revision } = answer else {
        panic!("{answer:?}")
    };
    let Notice::Library {
        modified: true,
        history,
        ..
    } = window.last_library()
    else {
        panic!()
    };
    assert_eq!(history, ["Move segment"]);
    let at = window.wait(
        mark,
        |n| matches!(n, Notice::Compiled { revision: r, .. } if *r == revision),
    );
    window.wait(
        at,
        |n| matches!(n, Notice::Icon { name, .. } if name == "switch-poe"),
    );

    // an expression asks; the person picks editing the param
    s.view(
        Some("rack-unit".into()),
        Some("switch-l2".into()),
        Vec::new(),
    );
    let boxed = node(&s, "box 0 0 0 2 1 \"u * 0.28\"");
    let answer = s.ask(
        &Ask::SetArg {
            node: boxed,
            index: 5,
            value: JsonValue::Number(0.56),
        },
        "Resize box",
    );
    let Answer::Choose { id, options } = answer else {
        panic!("{answer:?}")
    };
    assert_eq!(options[0].label, "Edit param u");
    assert_eq!(options[1].label, "Replace with a number");
    assert!(matches!(s.choose(id, 0), Answer::Applied { .. }));
    assert!(s.library().files()["sample.ilx"].contains("  param u 2\n"));
    // a choice is answered once
    assert!(matches!(s.choose(id, 1), Answer::Refused { .. }));

    // undo and redo
    assert_eq!(s.undo().as_deref(), Some("Set param u in rack-unit"));
    assert_eq!(s.redo().as_deref(), Some("Set param u in rack-unit"));

    // saving writes the folder, and the library is not modified
    s.save().unwrap();
    let Notice::Library {
        modified: false, ..
    } = window.last_library()
    else {
        panic!()
    };
    let on_disk = std::fs::read_to_string(root.join("sample.ilx")).unwrap();
    assert_eq!(&on_disk, &s.library().files()["sample.ilx"]);
    drop(s);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn changes_on_disk_are_loaded_or_flagged() {
    let root = folder("disk");
    let window = Arc::new(Window::default());
    let mut s = Session::open(&root, window.clone()).unwrap();
    // written outside, with nothing unsaved: loaded
    std::fs::write(
        root.join("extra.ilx"),
        "icon \"extra\" { category \"c\"; canvas 9 9; part \"p\" { at 0 0 0 { circle 4 4 3 } } }\n",
    )
    .unwrap();
    let mark = window.len();
    s.tick().unwrap();
    window.wait(
        mark,
        |n| matches!(n, Notice::Reloaded { files } if files == &["extra.ilx".to_string()]),
    );
    window.wait(
        mark,
        |n| matches!(n, Notice::Icon { name, .. } if name == "extra"),
    );

    // an edit unsaved, then a change outside: out of sync, then reloaded
    let seg = node(&s, "seg 0 0 0 2 0 0");
    assert!(matches!(
        s.ask(
            &Ask::SetArg {
                node: seg,
                index: 3,
                value: JsonValue::Number(1.0)
            },
            "x"
        ),
        Answer::Applied { .. }
    ));
    std::fs::remove_file(root.join("extra.ilx")).unwrap();
    let mark = window.len();
    s.tick().unwrap();
    window.wait(
        mark,
        |n| matches!(n, Notice::OutOfSync { files } if files == &["extra.ilx".to_string()]),
    );
    s.reload().unwrap();
    let Notice::Library {
        modified: false,
        files,
        ..
    } = window.last_library()
    else {
        panic!()
    };
    assert_eq!(files, ["sample.ilx"]);
    window.wait(
        mark,
        |n| matches!(n, Notice::Gone { name } if name == "extra"),
    );
    drop(s);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn asks_and_notices_are_the_json_the_interface_speaks() {
    let ask: Ask = serde_json::from_str(
        r#"{"kind":"set-arg","node":{"file":"a.ilx","start":10,"len":20},"index":3,"value":2.5}"#,
    )
    .unwrap();
    assert_eq!(
        ask,
        Ask::SetArg {
            node: Node {
                file: "a.ilx".into(),
                start: 10,
                len: 20
            },
            index: 3,
            value: JsonValue::Number(2.5)
        }
    );
    let ask: Ask = serde_json::from_str(r#"{"kind":"set-prop","node":{"file":"a.ilx","start":0,"len":1},"key":"fill","value":"ink"}"#).unwrap();
    assert!(matches!(ask, Ask::SetProp { value: JsonValue::Text(ref t), .. } if t == "ink"));
    let ask: Ask = serde_json::from_str(r#"{"kind":"insert","parent":{"file":"a.ilx","start":0,"len":1},"text":"box 0 0 0 1 1 1","before":null}"#).unwrap();
    assert!(matches!(ask, Ask::Insert { before: None, .. }));
    let notice = serde_json::to_value(Notice::OutOfSync {
        files: vec!["a.ilx".into()],
    })
    .unwrap();
    assert_eq!(
        notice,
        serde_json::json!({"kind": "out-of-sync", "files": ["a.ilx"]})
    );
    let answer = serde_json::to_value(Answer::Refused {
        reason: "no".into(),
    })
    .unwrap();
    assert_eq!(
        answer,
        serde_json::json!({"kind": "refused", "reason": "no"})
    );
}
