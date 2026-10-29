//! Compiling in the background (#62): the scheduler and a generation driven
//! with a clock and a compiler of the test's own, so they are
//! deterministic; then the threads, with isolex, end to end.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use isolex::ir::IrIcon;
use isolex_studio_core::compile::{
    order, run_generation, Background, Compiler, Diag, Drawing, Event, Generation, Isolex, Outcome,
    Problems, Report, Scheduler, SvgCache, QUIET,
};

fn files(text: &str) -> Arc<Vec<(String, String)>> {
    Arc::new(vec![("a.ilx".to_string(), text.to_string())])
}

#[test]
fn a_generation_starts_after_a_quiet_moment_and_cancels_the_one_before() {
    let t0 = Instant::now();
    let ms = |n: u64| t0 + Duration::from_millis(n);
    let mut s = Scheduler::default();
    assert!(s.poll(ms(0)).is_none(), "nothing edited");
    s.edit(1, files("1"), ms(0));
    assert!(s.poll(ms(50)).is_none(), "not quiet yet");
    // another edit starts the wait again
    s.edit(2, files("2"), ms(100));
    assert!(s.poll(ms(200)).is_none());
    let g1 = s.poll(ms(100) + QUIET).expect("quiet now");
    assert_eq!(
        (g1.number, g1.revision),
        (1, 2),
        "the newest edit, not each one"
    );
    assert!(s.poll(ms(1000)).is_none(), "nothing new to compile");
    s.edit(3, files("3"), ms(1000));
    let g2 = s.poll(ms(1000) + QUIET).unwrap();
    assert_eq!((g2.number, g2.revision), (2, 3));
    assert!(
        g1.cancel.load(Ordering::Relaxed),
        "the generation before is cancelled"
    );
    assert!(!g2.cancel.load(Ordering::Relaxed));
}

#[test]
fn icons_compile_the_open_one_first_then_those_shown_then_the_rest() {
    let icons: Vec<String> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let visible = vec![
        "d".to_string(),
        "zz".to_string(),
        "c".to_string(),
        "d".to_string(),
    ];
    assert_eq!(
        order(&icons, Some("e"), &visible),
        ["e", "d", "c", "a", "b"]
    );
    assert_eq!(order(&icons, Some("nope"), &[]), ["a", "b", "c", "d", "e"]);
}

fn drawing(name: &str) -> Arc<Drawing> {
    Arc::new(Drawing {
        ir: ir(name),
        sources: Vec::new(),
    })
}

fn ir(name: &str) -> IrIcon {
    IrIcon {
        name: name.to_string(),
        category: "c".into(),
        footprint: None,
        canvas: Some([1.0, 1.0]),
        viewbox: [0.0, 0.0, 1.0, 1.0],
        describe: Vec::new(),
        grants: Vec::new(),
        shapes: Vec::new(),
    }
}

fn drawn(g: u64, name: &str, hash: u64) -> Report {
    Report::Icon {
        generation: g,
        name: name.into(),
        outcome: Outcome::Drawn {
            drawing: drawing(name),
            hash,
        },
    }
}

fn lexicon(g: u64, icons: &[&str]) -> Report {
    Report::Lexicon {
        generation: g,
        revision: g,
        problems: Default::default(),
        icons: icons.iter().map(|s| s.to_string()).collect(),
    }
}

fn kinds(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .map(|e| match e {
            Event::Lexicon { generation, .. } => format!("lexicon {generation}"),
            Event::Changed { name, hash, .. } => format!("changed {name} {hash}"),
            Event::Broken { name, last, .. } => format!("broken {name} last={}", last.is_some()),
            Event::Gone { name } => format!("gone {name}"),
        })
        .collect()
}

#[test]
fn icons_are_told_as_changed_unchanged_or_broken_and_stale_results_dropped() {
    let mut t = isolex_studio_core::compile::Tracker::default();
    assert_eq!(kinds(&t.accept(lexicon(1, &["a", "b"]))), ["lexicon 1"]);
    assert_eq!(
        kinds(&t.accept(drawn(1, "a", 10))),
        ["changed a 10"],
        "first drawn"
    );
    assert!(
        t.accept(drawn(1, "a", 10)).is_empty(),
        "the same again says nothing"
    );
    assert_eq!(kinds(&t.accept(lexicon(2, &["a", "b"]))), ["lexicon 2"]);
    assert!(
        t.accept(drawn(1, "a", 99)).is_empty(),
        "an older generation's result is dropped"
    );
    assert!(t.accept(drawn(2, "a", 10)).is_empty(), "unchanged");
    assert_eq!(kinds(&t.accept(drawn(2, "a", 11))), ["changed a 11"]);
    let broken = Report::Icon {
        generation: 2,
        name: "a".into(),
        outcome: Outcome::Broken(vec![Diag {
            file: "a.ilx".into(),
            line: 1,
            col: 1,
            message: "no".into(),
        }]),
    };
    let events = t.accept(broken);
    assert_eq!(
        kinds(&events),
        ["broken a last=true"],
        "it keeps its last good drawing"
    );
    let Event::Broken {
        last: Some(last), ..
    } = &events[0]
    else {
        unreachable!()
    };
    assert_eq!(last.ir.name, "a");
    assert_eq!(
        kinds(&t.accept(drawn(2, "a", 11))),
        ["changed a 11"],
        "fixed, even as it was before"
    );
    // an icon the library stops emitting
    assert_eq!(
        kinds(&t.accept(lexicon(3, &["b"]))),
        ["lexicon 3", "gone a"]
    );
}

/// A compiler that records what it was asked, and can be made to cancel.
struct Fake {
    icons: Vec<String>,
    asked: Mutex<Vec<String>>,
    /// Sets the generation's flag after this many icons.
    cancel_after: Option<usize>,
    flag: Mutex<Option<Arc<AtomicBool>>>,
}

impl Compiler for Fake {
    type Prepared = ();
    fn prepare(&self, _: &[(String, String)]) -> ((), Problems, Vec<String>) {
        ((), Problems::default(), self.icons.clone())
    }
    fn icon(&self, _: &(), name: &str, cancel: &AtomicBool) -> Option<Outcome> {
        let mut asked = self.asked.lock().unwrap();
        asked.push(name.to_string());
        if self.cancel_after == Some(asked.len()) {
            if let Some(f) = &*self.flag.lock().unwrap() {
                f.store(true, Ordering::Relaxed);
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        Some(Outcome::Drawn {
            drawing: drawing(name),
            hash: name.len() as u64,
        })
    }
}

fn fake(cancel_after: Option<usize>) -> Fake {
    Fake {
        icons: (0..8).map(|i| format!("i{i}")).collect(),
        asked: Mutex::new(Vec::new()),
        cancel_after,
        flag: Mutex::new(None),
    }
}

fn generation() -> Generation {
    Generation {
        number: 1,
        revision: 1,
        files: files(""),
        cancel: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn a_generation_sends_the_lexicon_then_each_icon_in_order() {
    let c = fake(None);
    let sent = Mutex::new(Vec::new());
    run_generation(
        &c,
        &generation(),
        Some("i5"),
        &["i2".to_string()],
        1,
        &|r| sent.lock().unwrap().push(r),
    );
    let names: Vec<String> = sent
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|r| match r {
            Report::Lexicon { .. } => "lexicon".to_string(),
            Report::Icon { name, .. } => name,
        })
        .collect();
    assert_eq!(
        names,
        ["lexicon", "i5", "i2", "i0", "i1", "i3", "i4", "i6", "i7"]
    );
}

#[test]
fn a_cancelled_generation_stops_and_sends_nothing_more() {
    let c = fake(Some(3));
    let g = generation();
    *c.flag.lock().unwrap() = Some(g.cancel.clone());
    let sent = Mutex::new(Vec::new());
    run_generation(&c, &g, None, &[], 1, &|r| sent.lock().unwrap().push(r));
    let icons = sent
        .into_inner()
        .unwrap()
        .into_iter()
        .filter(|r| matches!(r, Report::Icon { .. }))
        .count();
    assert_eq!(
        icons, 2,
        "the third was cancelled as it ran, and none after it asked"
    );
    assert_eq!(c.asked.lock().unwrap().len(), 3);
    // one cancelled before it starts sends nothing at all
    let g = generation();
    g.cancel.store(true, Ordering::Relaxed);
    let sent = Mutex::new(Vec::new());
    run_generation(&fake(None), &g, None, &[], 4, &|r| {
        sent.lock().unwrap().push(r)
    });
    assert!(sent.into_inner().unwrap().is_empty());
}

#[test]
fn svg_is_kept_by_ir_and_palette() {
    let icon = ir("a");
    let p = isolex::render::Palette::default();
    let mut cache = SvgCache::new(2);
    let first = cache.svg(&icon, 1, &p, 7);
    assert!(
        Arc::ptr_eq(&first, &cache.svg(&icon, 1, &p, 7)),
        "kept, not drawn again"
    );
    cache.svg(&icon, 2, &p, 7);
    cache.svg(&icon, 3, &p, 7);
    assert_eq!(cache.len(), 2, "the least recently used went");
    assert!(
        !Arc::ptr_eq(&first, &cache.svg(&icon, 1, &p, 7)),
        "drawn again once it went"
    );
}

/// Events until `done` says so, or a minute.
fn collect(
    rx: &std::sync::mpsc::Receiver<Event>,
    mut done: impl FnMut(&[Event]) -> bool,
) -> Vec<Event> {
    let until = Instant::now() + Duration::from_secs(60);
    let mut out = Vec::new();
    while !done(&out) {
        let left = until.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "timed out with {:?}", kinds(&out));
        if let Ok(e) = rx.recv_timeout(left) {
            out.push(e);
        }
    }
    out
}

#[test]
fn in_the_background_an_edit_redraws_only_what_it_changes() {
    let sample = include_str!("../../tests/fixtures/sample.ilx");
    let (bg, rx) = Background::start(Isolex, 4);
    bg.view(Some("switch-poe".into()), Vec::new());
    bg.edit(1, files(sample));
    let all: BTreeSet<String> = isolex::compile(&files(sample))
        .doc
        .icons
        .iter()
        .map(|i| i.name.clone())
        .collect();
    let first = collect(&rx, |e| {
        e.iter()
            .filter(|e| matches!(e, Event::Changed { .. }))
            .count()
            == all.len()
    });
    let Event::Lexicon { revision: 1, .. } = &first[0] else {
        panic!("{:?}", kinds(&first))
    };
    let changed: Vec<String> = first
        .iter()
        .filter_map(|e| match e {
            Event::Changed { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    // it starts first, but with four workers on however many cores, any of
    // the others may finish before it: the next test holds the order
    assert_eq!(changed.iter().cloned().collect::<BTreeSet<_>>(), all);

    // a different disk count in switch-poe: only it and its variants redraw
    let edited = sample.replace(
        "insert \"disk-stack\" cx=0.5 cy=0.5 z=0.12 n=2",
        "insert \"disk-stack\" cx=0.5 cy=0.5 z=0.12 n=3",
    );
    assert_ne!(edited, sample);
    bg.edit(2, files(&edited));
    let second = collect(&rx, |e| {
        e.iter()
            .filter(|e| matches!(e, Event::Changed { .. }))
            .count()
            == 3
    });
    // nothing more comes from that generation
    std::thread::sleep(Duration::from_millis(500));
    let more: Vec<Event> = rx.try_iter().collect();
    let names: BTreeSet<String> = second
        .iter()
        .chain(&more)
        .filter_map(|e| match e {
            Event::Changed { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    let want: BTreeSet<String> = ["switch-poe", "switch-poe-down", "switch-poe-planned"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(names, want);

    // broken: it keeps its last drawing
    let broken = edited.replace("cylinder 0.5 0.5 0 0.4 0.12", "cylinder 0.5 0.5 0 0.4");
    bg.edit(3, files(&broken));
    let third = collect(&rx, |e| {
        e.iter()
            .any(|e| matches!(e, Event::Lexicon { revision: 3, .. }))
    });
    let Event::Lexicon { problems, .. } = third.last().unwrap() else {
        unreachable!()
    };
    assert!(
        !problems.errors.is_empty(),
        "the lexicon's own error is told, as an error"
    );
    drop(bg);
}

#[test]
fn in_the_background_the_open_icon_is_drawn_first() {
    // one worker finishes in the order it starts, on any number of cores
    let sample = include_str!("../../tests/fixtures/sample.ilx");
    let (bg, rx) = Background::start(Isolex, 1);
    bg.view(Some("switch-poe".into()), Vec::new());
    bg.edit(1, files(sample));
    let events = collect(&rx, |e| {
        e.iter().any(|e| matches!(e, Event::Changed { .. }))
    });
    let Some(Event::Changed { name, .. }) = events.last() else {
        unreachable!()
    };
    assert_eq!(name, "switch-poe", "{:?}", kinds(&events));
    drop(bg);
}

/// Shapes as provenance counts them: a group, then its own.
fn count(shapes: &[isolex::ir::IrShape]) -> usize {
    shapes
        .iter()
        .map(|s| match s {
            isolex::ir::IrShape::Group { shapes, .. } => 1 + count(shapes),
            _ => 1,
        })
        .sum()
}

fn draw(text: &str, name: &str) -> (Arc<Drawing>, u64) {
    let (ready, _, _) = Isolex.prepare(&files(text));
    match Isolex.icon(&ready, name, &AtomicBool::new(false)) {
        Some(Outcome::Drawn { drawing, hash }) => (drawing, hash),
        other => panic!("{name}: {other:?}"),
    }
}

#[test]
fn a_drawing_says_where_each_shape_is_written() {
    let sample = include_str!("../../tests/fixtures/sample.ilx");
    let (d, _) = draw(sample, "switch-l3");
    assert_eq!(d.sources.len(), count(&d.ir.shapes));
    for s in &d.sources {
        assert_eq!(s.node.file, "a.ilx");
        let text = &sample[s.node.start..s.node.start + s.node.len];
        assert!(
            ["box", "facerect", "path"]
                .iter()
                .any(|k| text.starts_with(k)),
            "{text}"
        );
    }
    let from = |kind: &str| {
        d.sources
            .iter()
            .find(|s| sample[s.node.start..].starts_with(kind))
            .unwrap()
    };
    for s in &d.sources {
        assert!(sample[s.node.start..].starts_with(&format!("{} ", s.kind)));
    }
    assert_eq!(from("box").kind, "box");
    assert_eq!(from("box").values, [0.0, 0.0, 0.0, 2.0, 1.0, 0.28]);
    assert!(from("path").values.is_empty());
    assert_eq!(from("box").definition, "rack-unit");
    assert_eq!(from("box").part.as_deref(), Some("chassis"));
    assert_eq!(from("path").definition, "switch-l3");
    // a port is drawn through its repeat
    let port = from("facerect");
    assert_eq!(port.definition, "switch");
    assert_eq!(port.sites.len(), 1);
    assert!(sample[port.sites[0].start..].starts_with("repeat \"ports\""));
}

#[test]
fn the_same_icon_written_elsewhere_is_drawn_again() {
    let sample = include_str!("../../tests/fixtures/sample.ilx");
    let (before, h1) = draw(sample, "switch-l3");
    let moved = format!("// a line above everything\n{sample}");
    let (after, h2) = draw(&moved, "switch-l3");
    assert_eq!(
        serde_json::to_string(&before.ir).unwrap(),
        serde_json::to_string(&after.ir).unwrap()
    );
    assert_ne!(h1, h2, "selecting goes by where shapes are written");
    let shift = "// a line above everything\n".len();
    assert_eq!(
        after.sources[0].node.start,
        before.sources[0].node.start + shift
    );
    assert_eq!(
        draw(sample, "switch-l3").1,
        h1,
        "and the same source hashes the same"
    );
}
