//! What the drawing tools write (#83), through the edit engine: each kind
//! of node, as the interface writes it, with random numbers at the snap
//! step, inserted where the tools insert it (a part, or the anchor at the
//! origin flat shapes share), compiles and draws the shape, with the
//! numbers written; and a shape repeated as the Design tab writes it draws
//! a copy each, stepping. Seeded xorshift; the seed is in every message.

use std::collections::BTreeMap;

use isolex::compile;
use isolex_studio_core::edit::{plan, Intent, NodeRef, Plan, Request};
use isolex_studio_core::Library;

/// xorshift64*: deterministic, no dependencies.
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
    /// A number at `step`, from `lo` up to (not past) `hi`.
    fn at(&mut self, step: f64, lo: f64, hi: f64) -> f64 {
        let n = ((hi - lo) / step) as u64 + 1;
        lo + (self.next() % n) as f64 * step
    }
}

const FILE: &str = "drawn.ilx";

/// An icon with a part to draw in, a box to stand face rects on, and the
/// anchor flat shapes share, as the tools leave them.
const START: &str = r#"lexicon {
  tokens "ink" "outline" "body-top" "body-left" "body-right" "shadow"
  stroke-widths 1.2 1.4
}

icon "drawn" {
  category "test"
  footprint 4 4 2
  part "body" {
    box 0 0 0 2 1 1
    at 0 0 0 {
      line 0 0 1 0
    }
  }
}
"#;

fn library() -> Library {
    let files: BTreeMap<String, String> = [(FILE.to_string(), START.to_string())]
        .into_iter()
        .collect();
    Library::new("/nowhere", files)
}

/// The node whose text starts `prefix`.
fn node(lib: &Library, prefix: &str) -> NodeRef {
    let text = &lib.files()[FILE];
    let start = text
        .find(prefix)
        .unwrap_or_else(|| panic!("{prefix} in {text}"));
    // to the end of the block or the line
    let len = if prefix.ends_with('{') {
        let mut depth = 0;
        let mut end = start;
        for (i, c) in text[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        end - start
    } else {
        text[start..].find('\n').unwrap()
    };
    NodeRef {
        file: FILE.into(),
        start,
        len,
    }
}

fn apply(lib: &mut Library, intent: Intent) {
    let req = Request::new(lib, None, "draw", intent.clone());
    let Plan::Apply(tx) = plan(lib, &req).unwrap_or_else(|e| panic!("{intent:?}: {e}")) else {
        panic!("{intent:?} asked for a choice")
    };
    lib.apply(tx).unwrap();
}

/// Each copy's numbers drawn by the node whose first line is `line`.
fn drawn(lib: &Library, line: &str) -> Vec<Vec<f64>> {
    let files: Vec<(String, String)> = lib
        .files()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let c = compile(&files);
    assert!(c.errors.is_empty(), "{:?}\n{}", c.errors, files[0].1);
    let text = &files[0].1;
    let mut out: Vec<Vec<f64>> = Vec::new();
    for p in &c.provenance[0] {
        let first = text[p.span.start..p.span.start + p.span.len]
            .lines()
            .next()
            .unwrap()
            .trim();
        if first == line && out.last() != Some(&p.values) {
            out.push(p.values.clone());
        }
    }
    out
}

/// A number as the tools write it.
fn n(v: f64) -> String {
    format!("{v}")
}

#[test]
fn each_kind_the_tools_write_compiles_and_draws_its_numbers() {
    for seed in 1..=60u64 {
        let mut rng = Rng(0xD1A3_0083 ^ (seed << 9));
        let q = |rng: &mut Rng, lo: f64, hi: f64| rng.at(0.25, lo, hi);
        let u = |rng: &mut Rng, lo: f64, hi: f64| rng.at(0.5, lo, hi);
        // (what is written, whether it is flat, the numbers it draws)
        let (x, y, z) = (
            q(&mut rng, 0.0, 3.0),
            q(&mut rng, 0.0, 3.0),
            q(&mut rng, 0.0, 1.0),
        );
        let (w, d, h) = (
            q(&mut rng, 0.25, 1.0),
            q(&mut rng, 0.25, 1.0),
            q(&mut rng, 0.25, 1.0),
        );
        let (fx, fy) = (u(&mut rng, -20.0, 20.0), u(&mut rng, -20.0, 20.0));
        let (fw, fh) = (u(&mut rng, 0.5, 10.0), u(&mut rng, 0.5, 10.0));
        let a0 = rng.at(15.0, 0.0, 345.0);
        let sweep = rng.at(15.0, 15.0, 345.0);
        let cases: Vec<(String, bool, Vec<f64>)> = vec![
            (
                format!("box {} {} {} {} {} {}", n(x), n(y), n(z), n(w), n(d), n(h)),
                false,
                vec![x, y, z, w, d, h],
            ),
            (
                format!("tile {} {} {} {} {}", n(x), n(y), n(z), n(w), n(d)),
                false,
                vec![x, y, z, w, d],
            ),
            (
                format!("cylinder {} {} {} {} {}", n(x), n(y), n(z), n(w), n(h)),
                false,
                vec![x, y, z, w, h],
            ),
            (
                format!(
                    "facerect \"front\" {} 1 {} {} {}",
                    n(x / 2.0),
                    n(z / 2.0),
                    n(w),
                    n(h / 2.0)
                ),
                false,
                vec![x / 2.0, 1.0, z / 2.0, w, h / 2.0],
            ),
            (
                format!("shadow {} {} {} {}", n(x), n(y), n(w), n(d)),
                false,
                vec![x, y, w, d],
            ),
            (
                format!("seg {} {} {} {} {} {}", n(x), n(y), n(z), n(w), n(d), n(h)),
                false,
                vec![x, y, z, w, d, h],
            ),
            (
                format!(
                    "poly3d {{\n  pt {} {} {}\n  pt {} {} {}\n  pt {} {} {}\n}}",
                    n(x),
                    n(y),
                    n(z),
                    n(x + w),
                    n(y),
                    n(z),
                    n(x),
                    n(y + d),
                    n(z)
                ),
                false,
                vec![],
            ),
            (
                format!("line {} {} {} {}", n(fx), n(fy), n(fx + fw), n(fy)),
                true,
                vec![fx, fy, fx + fw, fy],
            ),
            (
                format!("rect {} {} {} {}", n(fx), n(fy), n(fw), n(fh)),
                true,
                vec![fx, fy, fw, fh, 0.0],
            ),
            (
                format!("circle {} {} {}", n(fx), n(fy), n(fw)),
                true,
                vec![fx, fy, fw],
            ),
            (
                format!("ellipse {} {} {} {}", n(fx), n(fy), n(fw), n(fh)),
                true,
                vec![fx, fy, fw, fh],
            ),
            (
                format!(
                    "arc {} {} {} {} {}",
                    n(fx),
                    n(fy),
                    n(fw),
                    n(a0),
                    n(a0 + sweep)
                ),
                true,
                vec![fx, fy, fw, a0, a0 + sweep],
            ),
            (
                format!(
                    "poly {{\n  pt {} {}\n  pt {} {}\n  pt {} {}\n}}",
                    n(fx),
                    n(fy),
                    n(fx + fw),
                    n(fy),
                    n(fx),
                    n(fy + fh)
                ),
                true,
                vec![fx, fy, fx + fw, fy, fx, fy + fh],
            ),
            (
                format!(
                    "path d=\"M{},{} C{},{} {},{} {},{} Z\"",
                    n(fx),
                    n(fy),
                    n(fx + fw),
                    n(fy),
                    n(fx + fw),
                    n(fy + fh),
                    n(fx),
                    n(fy + fh)
                ),
                true,
                vec![],
            ),
        ];
        for (text, flat, want) in cases {
            let mut lib = library();
            let parent = if flat {
                node(&lib, "at 0 0 0 {")
            } else {
                node(&lib, "part \"body\" {")
            };
            apply(
                &mut lib,
                Intent::Insert {
                    parent,
                    text: text.clone(),
                    before: None,
                },
            );
            let line = text.lines().next().unwrap().trim();
            let got = drawn(&lib, line);
            assert_eq!(got.len(), 1, "seed {seed}: {text} draws once");
            assert_eq!(got[0], want, "seed {seed}: {text}");
        }
    }
}

#[test]
fn a_shape_repeated_as_the_design_tab_writes_it_draws_each_copy() {
    for seed in 1..=30u64 {
        let mut rng = Rng(0x2E9E_A700 ^ (seed << 7));
        let count = 2 + (rng.next() % 5) as usize;
        let step = rng.at(0.25, 0.25, 1.0);
        let from = rng.at(0.25, 0.0, 1.0);
        let mut lib = library();
        let tile = format!("tile {} 2 0 0.5 0.5", n(from));
        let parent = node(&lib, "part \"body\" {");
        apply(
            &mut lib,
            Intent::Insert {
                parent,
                text: tile.clone(),
                before: None,
            },
        );
        // the node, typed over by itself in a repeat, as the tab types it
        let at = node(&lib, &tile);
        let wrapped = format!(
            "repeat {count} \"i\" {{\n      tile \"{} + i * {}\" 2 0 0.5 0.5\n    }}",
            n(from),
            n(step)
        );
        apply(
            &mut lib,
            Intent::Type {
                file: FILE.into(),
                at: at.start,
                len: at.len,
                text: wrapped,
            },
        );
        let copies = drawn(
            &lib,
            &format!("tile \"{} + i * {}\" 2 0 0.5 0.5", n(from), n(step)),
        );
        assert_eq!(copies.len(), count, "seed {seed}: a copy each");
        for (k, c) in copies.iter().enumerate() {
            assert!(
                (c[0] - (from + k as f64 * step)).abs() < 1e-9,
                "seed {seed}: copy {k} at {c:?}"
            );
        }
    }
}
