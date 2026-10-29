//! The studio at scale (#71): a generated library of 500 icons, shaped as
//! a real one is (bam's lexicon lives in each site's database, so it is
//! modelled, not copied): bases extending bases, icons extending them,
//! ports repeated from a fragment, a state axis whose variants badge flat
//! icons. Timed: compiling it all, the compile after an edit to one icon
//! and to a base many extend, and drawing every thumbnail. The library is
//! kept as `tests/fixtures/library-500.ilx`, which the interface's tests
//! open too; UPDATE_GOLDENS=1 writes it again.

use std::sync::Arc;
use std::time::{Duration, Instant};

use isolex_studio_core::compile::{Background, Event, Inline, Isolex, SvgCache};
use isolex_studio_core::tree::tree;

const ICONS: usize = 500;
const BASES: usize = 20;

/// The library: deterministic, so its fixture is checked like a golden.
fn library() -> String {
    let mut s = String::from(
        "// 500 icons for timing the studio (#71): written by\n\
         // studio-core/tests/scale.rs; UPDATE_GOLDENS=1 writes it again.\n\n\
         lexicon {\n  tokens \"ink\" \"ink-muted\" \"outline\" \"body-top\" \"body-left\" \"body-right\" \"accent\"\n  stroke-widths 1.2 1.4\n}\n\n\
         fragment \"led-row\" {\n  param n 4\n  param z 0.1\n  part \"leds\" {\n    repeat \"n\" \"i\" {\n      facerect \"front\" \"0.1 + i * (1.8 / n)\" 1 \"z\" 0.08 0.08 fill=\"ink\" opacity=0.6\n    }\n  }\n}\n\n\
         axis \"state\" {\n  variant \"down\" {\n    part \"state-badge\" {\n      badge \"state-down\" corner=\"ne\"\n    }\n  }\n  variant \"planned\" {\n    part \"state-badge\" {\n      badge \"state-planned\" corner=\"ne\"\n    }\n  }\n}\n\n\
         icon \"state-down\" {\n  category \"state\"\n  canvas 24 24\n  part \"x\" {\n    at 0 0 0 {\n      circle 12 12 8 fill=\"accent\"\n      path d=\"M8,8 L16,16 M16,8 L8,16\" stroke=\"ink\" width=1.4\n    }\n  }\n}\n\n\
         icon \"state-planned\" {\n  category \"state\"\n  canvas 24 24\n  part \"x\" {\n    at 0 0 0 {\n      rect 3 3 18 18 rx=3 stroke=\"ink-muted\" width=1.4\n    }\n  }\n}\n",
    );
    for b in 0..BASES {
        // every fifth base extends the one before it
        let extends = if b % 5 == 4 {
            format!(" extends=\"rack-{}\"", b - 1)
        } else {
            String::new()
        };
        // one extending another adds a rail to its chassis
        let part = if extends.is_empty() {
            "  part \"chassis\" {\n    box 0 0 0 2 1 \"u * 0.28\"\n  }\n"
        } else {
            "  part \"rail\" {\n    tile 0 0.9 \"u * 0.28\" 2 0.1 fill=\"ink-muted\"\n  }\n"
        };
        s += &format!(
            "\nbase \"rack-{b}\"{extends} {{\n  category \"rack\"\n  param u {}\n  footprint 2 1 \"u * 0.28\"\n{part}}}\n",
            1 + b % 3
        );
    }
    for i in 0..ICONS {
        let axes = if i % 10 == 0 {
            "\n  axes \"state\""
        } else {
            ""
        };
        s += &format!(
            "\nicon \"dev-{i}\" {{\n  extends \"rack-{}\"{axes}\n  part \"ports\" {{\n    insert \"led-row\" n={} z=0.1\n  }}\n  part \"label\" {{\n    tile 0.2 0.2 \"u * 0.28\" 0.6 0.3 fill=\"accent\"\n  }}\n}}\n",
            i % BASES,
            1 + i % 8
        );
    }
    s
}

fn files(text: &str) -> Arc<Vec<(String, String)>> {
    Arc::new(vec![("library-500.ilx".to_string(), text.to_string())])
}

/// The icons an edit's events redrew (their drawing changed, not only
/// where it is written), and how long it took.
fn timed(c: &mut Inline, revision: u64, text: &str) -> (usize, Duration) {
    let at = Instant::now();
    let events = c.edit(revision, files(text));
    let took = at.elapsed();
    for e in &events {
        if let Event::Broken { name, diags, .. } = e {
            panic!("{name} does not compile: {:?}", diags.first());
        }
    }
    let redrawn = events
        .iter()
        .filter(|e| matches!(e, Event::Changed { redrawn: true, .. }))
        .count();
    (redrawn, took)
}

/// Generous bounds: the jail is slower than a desktop, and a debug build
/// slower still. What they guard against is the order of magnitude.
fn bound(release: u64) -> Duration {
    Duration::from_millis(if cfg!(debug_assertions) {
        release * 12
    } else {
        release
    })
}

#[test]
fn five_hundred_icons_compile_redraw_and_draw_in_time() {
    let text = library();
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/library-500.ilx"
    );
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(fixture, &text).unwrap();
    }
    let kept = std::fs::read_to_string(fixture)
        .expect("fixture missing: run with UPDATE_GOLDENS=1 once")
        .replace("\r\n", "\n");
    assert_eq!(kept, text, "the fixture drifted from its generator");

    // the tree: every icon and variant listed
    let t = tree(&files(&text));
    let rows = |rows: &[isolex_studio_core::tree::TreeRow]| -> usize {
        fn count(rs: &[isolex_studio_core::tree::TreeRow]) -> usize {
            rs.iter().map(|r| 1 + count(&r.children)).sum()
        }
        count(rows)
    };
    // 20 bases, 500 icons, 100 variants, 2 flat icons
    assert_eq!(rows(&t.icons), BASES + ICONS + 100 + 2);

    let mut c = Inline::new(Isolex);
    let (all, open) = timed(&mut c, 1, &text);
    assert_eq!(all, ICONS + 100 + 2, "every icon drawn once");

    // one icon's number: it alone redrawn
    let leaf = text.replace(
        "icon \"dev-7\" {\n  extends \"rack-7\"\n  part \"ports\" {\n    insert \"led-row\" n=8",
        "icon \"dev-7\" {\n  extends \"rack-7\"\n  part \"ports\" {\n    insert \"led-row\" n=3",
    );
    assert_ne!(leaf, text);
    let (one, after_leaf) = timed(&mut c, 2, &leaf);
    assert_eq!(one, 1, "only dev-7 redrawn");

    // a base's chassis: every icon extending it, and those extending the
    // base that extends it; it is shorter, so every icon after it is told
    // again, moved, but not redrawn
    let base = leaf.replace(
        "base \"rack-3\" {\n  category \"rack\"\n  param u 1\n  footprint 2 1 \"u * 0.28\"\n  part \"chassis\" {\n    box 0 0 0 2 1",
        "base \"rack-3\" {\n  category \"rack\"\n  param u 1\n  footprint 2 1 \"u * 0.28\"\n  part \"chassis\" {\n    box 0 0 0 2 0.9",
    );
    assert_ne!(base, leaf);
    let (many, after_base) = timed(&mut c, 3, &base);
    // rack-3's 25 and rack-4's (which extends it) 25; none has an axis
    assert_eq!(many, 50, "only what rack-3 reaches redrawn");

    // in the background, as the app compiles: the icon open is drawn
    // first, before the rest, after the quiet moment an edit waits for
    let (bg, rx) = Background::start(Isolex, 4);
    bg.view(Some("dev-7".into()), Vec::new());
    bg.edit(1, files(&text));
    let mut seen = 0;
    while seen < all {
        if let Ok(Event::Changed { .. }) = rx.recv_timeout(Duration::from_secs(60)) {
            seen += 1;
        }
    }
    let at = Instant::now();
    bg.edit(2, files(&leaf));
    let open_redrawn = loop {
        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(Event::Changed { name, .. }) if name == "dev-7" => break at.elapsed(),
            Ok(_) => {}
            Err(e) => panic!("dev-7 never redrawn: {e}"),
        }
    };
    drop(bg);

    // every thumbnail drawn as SVG
    let drawings: Vec<_> = Inline::new(Isolex)
        .edit(1, files(&text))
        .into_iter()
        .filter_map(|e| match e {
            Event::Changed { drawing, hash, .. } => Some((drawing, hash)),
            _ => None,
        })
        .collect();
    let palette = isolex::render::Palette::default();
    let mut cache = SvgCache::new(2048);
    let at = Instant::now();
    for (d, hash) in &drawings {
        cache.svg(&d.ir, *hash, &palette, 0);
    }
    let thumbnails = at.elapsed();
    // drawn again from the cache: nothing redrawn that did not change
    let at = Instant::now();
    for (d, hash) in &drawings {
        cache.svg(&d.ir, *hash, &palette, 0);
    }
    let cached = at.elapsed();

    eprintln!(
        "500 icons: all compiled in {open:?}; after one icon's edit {after_leaf:?} \
         ({one} redrawn), the open icon redrawn in the background in \
         {open_redrawn:?}; after a base's {after_base:?} ({many} redrawn); \
         {} thumbnails drawn in {thumbnails:?}, again from the cache in {cached:?}",
        drawings.len()
    );
    assert!(open < bound(8000), "all compiled in {open:?}");
    assert!(
        after_leaf < bound(4000),
        "one icon's edit in {after_leaf:?}"
    );
    assert!(after_base < bound(8000), "a base's edit in {after_base:?}");
    // the quiet moment and one icon, not the library
    assert!(
        open_redrawn < bound(1000),
        "the open icon redrawn in {open_redrawn:?}"
    );
    assert!(thumbnails < bound(3000), "thumbnails in {thumbnails:?}");
}
