//! Prepared lexicons and cancellable compiles (#58): parsed and resolved
//! once, compiled from as many threads as want to, each compile stopped
//! once it is no longer wanted.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use isolex::{compile, Only, Prepared, Refused};

const FIXTURES: &[(&str, &str)] = &[
    ("sample.ilx", include_str!("fixtures/sample.ilx")),
    ("transforms.ilx", include_str!("fixtures/transforms.ilx")),
    ("text.ilx", include_str!("fixtures/text.ilx")),
    ("palette.ilx", include_str!("fixtures/palette.ilx")),
    ("graph.ilx", include_str!("fixtures/graph.ilx")),
];

fn files(name: &str, src: &str) -> Vec<(String, String)> {
    vec![(name.to_string(), src.to_string())]
}

fn request(icon: &str) -> Only {
    Only {
        icon: icon.to_string(),
        params: BTreeMap::new(),
    }
}

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn it_is_send_and_sync() {
    fn shareable<T: Send + Sync>() {}
    shareable::<Prepared>();
}

/// Each icon, compiled alone from one Prepared, as `compile` drew it in the
/// whole lexicon.
fn each_icon(prepared: &Prepared, names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|name| {
            let c = prepared.icon(&request(name), &never()).unwrap();
            assert!(c.errors.is_empty(), "{name}: {:?}", c.errors);
            assert_eq!(c.doc.icons.len(), 1);
            format!(
                "{}\n{:?}",
                serde_json::to_string(&c.doc.icons[0]).unwrap(),
                c.provenance[0]
            )
        })
        .collect()
}

#[test]
fn every_icon_compiles_as_compile_draws_it() {
    for (name, src) in FIXTURES {
        let files = files(name, src);
        let whole = compile(&files);
        assert!(whole.errors.is_empty(), "{name}");
        let prepared = Prepared::new(&files);
        assert!(prepared.outcome().errors.is_empty());
        assert_eq!(prepared.outcome().warnings, whole.warnings, "{name}");
        assert_eq!(prepared.outcome().palette, whole.palette, "{name}");
        let names: Vec<String> = whole.doc.icons.iter().map(|i| i.name.clone()).collect();
        let want: Vec<String> = whole
            .doc
            .icons
            .iter()
            .zip(&whole.provenance)
            .map(|(icon, prov)| format!("{}\n{prov:?}", serde_json::to_string(icon).unwrap()))
            .collect();
        assert_eq!(each_icon(&prepared, &names), want, "{name}");
    }
}

#[test]
fn four_threads_sharing_one_give_what_one_gives() {
    let files = files("text.ilx", FIXTURES[2].1);
    let names: Vec<String> = compile(&files)
        .doc
        .icons
        .iter()
        .map(|i| i.name.clone())
        .collect();
    let prepared = Prepared::new(&files);
    let alone = each_icon(&prepared, &names);
    let shared: Vec<Vec<String>> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|t| {
                let (prepared, names) = (&prepared, &names);
                // each its own order, so they overlap differently
                scope.spawn(move || {
                    let mut order: Vec<String> = names.clone();
                    order.rotate_left(t * names.len() / 4);
                    let mut got = each_icon(prepared, &order);
                    got.rotate_right(t * names.len() / 4);
                    got
                })
            })
            .collect();
        threads.into_iter().map(|t| t.join().unwrap()).collect()
    });
    for got in shared {
        assert_eq!(got, alone);
    }
}

#[test]
fn a_set_flag_cancels() {
    let files = files("sample.ilx", FIXTURES[0].1);
    let prepared = Prepared::new(&files);
    let cancel = AtomicBool::new(true);
    assert_eq!(
        prepared.icon(&request("switch-l3"), &cancel).unwrap_err(),
        Refused::Cancelled
    );
    let all = prepared.icons(&[request("switch-l2"), request("switch-l3")], &cancel);
    assert!(all
        .iter()
        .all(|r| r.as_ref().unwrap_err() == &Refused::Cancelled));
    // and a request it cannot answer is still refused as one
    let nope = prepared.icon(&request("nope"), &never()).unwrap_err();
    assert!(matches!(nope, Refused::Request(ref why) if why.contains("emits no icon 'nope'")));
}

#[test]
fn a_compile_cancelled_while_it_runs_stops_soon_after() {
    // a hundred million iterations that draw nothing: long, uncancelled
    let src = r#"lexicon {
  repeat-limit #null
}
icon "long" {
  category "c"
  footprint 1 1 1
  part "p" {
    repeat 100000000 "i" {
      tile 0 0 0 1 1 when="i < 0"
    }
  }
}
"#;
    let files = files("long.ilx", src);
    let prepared = Prepared::new(&files);
    assert!(
        prepared.outcome().errors.is_empty(),
        "{:?}",
        prepared.outcome().errors
    );
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let result = std::thread::scope(|scope| {
        let compile = scope.spawn(|| prepared.icon(&request("long"), &cancel));
        std::thread::sleep(Duration::from_millis(100));
        cancel.store(true, Ordering::Relaxed);
        compile.join().unwrap()
    });
    assert_eq!(result.unwrap_err(), Refused::Cancelled);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "took {:?} to stop",
        started.elapsed()
    );
}
