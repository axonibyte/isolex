//! The isolex engine battery: a seed-driven lexicon exerciser in the
//! house Tier-9 shape. A deterministic generator emits random valid
//! lexicons while a shadow model records the expected emission census
//! (icon names after axis products, shape counts per icon); the compiled
//! `isolex` binary is run against them and diffed, and every IR is held
//! to structural rules no valid compile may break (every number finite,
//! every name unique, every badge filled). Nemesis rounds mutate the
//! source (truncation, cycles, expression abuse, oversized and nested
//! repeats, colliding names, reserved names, overflow) and require
//! located diagnostics — never a panic. Failures shrink by dropping
//! definitions while the failure reproduces.
//!
//! The generator covers what the language has: inheritance with replace
//! and remove, axes, 3-D nodes and `at` with 2-D nodes and paths,
//! fragments in either dimension, parameter expressions with functions
//! and `if`, `when` guards, badges, and a declared vocabulary with a
//! reserved token.
//!
//! Usage: battery [--isolex <path>] [--seeds <file>] [--rounds <n>]
//! Env: SEED=<n> runs a single seed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::process::Command;

/// xorshift64* — deterministic across platforms, no dependencies.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
}

#[derive(Default)]
struct Shadow {
    /// expected icon name -> expected primitive count
    icons: BTreeMap<String, usize>,
}

/// Fragments a part can insert: 3-D ones (a row of `k` tiles) and 2-D
/// ones for inside `at` (a circle and a line, so two shapes).
#[derive(Default)]
struct Frags {
    rows: Vec<String>,
    glyphs: Vec<String>,
}

struct Gen {
    src: String,
    shadow: Shadow,
    /// base name -> shape counts by part name
    bases: Vec<(String, BTreeMap<String, usize>)>,
    axes: Vec<(String, Vec<String>, bool)>, // name, variants, emit_base
}

/// One 2-D node inside an `at`, and how many IR shapes it projects to.
fn gen_2d(rng: &mut Rng, frags: &Frags, src: &mut String, indent: &str) -> usize {
    match rng.below(5) {
        0 => {
            let _ = writeln!(
                src,
                "{indent}line \"x - 3\" \"y\" \"x + 3\" \"y + max(0, 1)\""
            );
            1
        }
        1 => {
            let r = 1 + rng.below(4);
            let _ = writeln!(
                src,
                "{indent}circle \"x\" \"y - 2\" \"{r} * abs(cos(60) * 2)\""
            );
            1
        }
        2 => {
            let _ = writeln!(
                src,
                "{indent}path d=\"M{{x - 4}},{{y}} h{{round(7.6)}} l-2,{{if(x > 0, 2, 3)}}\""
            );
            1
        }
        3 => {
            // a polygon is one shape whatever its points
            let n = 3 + rng.below(4);
            let _ = writeln!(src, "{indent}poly {{");
            let _ = writeln!(src, "{indent}  repeat {n} \"k\" {{");
            let _ = writeln!(
                src,
                "{indent}    pt \"x + 4 * cosr(k * 6.283185307179586 / {n})\" \"y - 4 * sinr(k * 6.283185307179586 / {n})\""
            );
            let _ = writeln!(src, "{indent}  }}");
            let _ = writeln!(src, "{indent}}}");
            1
        }
        _ if !frags.glyphs.is_empty() => {
            let g = &frags.glyphs[rng.below(frags.glyphs.len())];
            let r = 1 + rng.below(3);
            let _ = writeln!(src, "{indent}insert \"{g}\" r={r}");
            2
        }
        _ => {
            let _ = writeln!(src, "{indent}circle \"x\" \"y\" 2");
            1
        }
    }
}

/// One part body, and how many IR shapes it projects to. One box = 3
/// primitives (left/right/top); a tile, and each 2-D node, 1; an `at`
/// none of its own.
fn gen_part(rng: &mut Rng, frags: &Frags, src: &mut String, indent: &str) -> usize {
    let mut count = 0;
    let n = 1 + rng.below(3);
    for _ in 0..n {
        match rng.below(10) {
            0 | 1 => {
                // a guard keeps the even iterations
                let reps = 1 + rng.below(6);
                let _ = writeln!(src, "{indent}repeat {reps} \"i\" {{");
                let _ = writeln!(
                    src,
                    "{indent}  tile \"i * 0.5\" 0 0 0.4 0.4 fill=\"body-top\" when=\"i % 2 == 0\""
                );
                let _ = writeln!(src, "{indent}}}");
                count += reps.div_ceil(2);
            }
            2 => {
                let reps = 1 + rng.below(5);
                let _ = writeln!(src, "{indent}repeat {reps} \"i\" {{");
                let _ = writeln!(
                    src,
                    "{indent}  tile \"i * 0.5\" 0 0 0.4 0.4 fill=\"body-top\""
                );
                let _ = writeln!(src, "{indent}}}");
                count += reps;
            }
            3 | 4 => {
                let x = rng.below(4);
                let _ = writeln!(src, "{indent}box {x} 0 0 1 1 \"u * 0.5\"");
                count += 3;
            }
            5 => {
                // every value an expression, parameters and functions
                let _ = writeln!(
                    src,
                    "{indent}tile \"if(u > 1, 0.1, 0)\" \"0.5 * sinr(0)\" \"floor(u / 2)\" \"max(0.2, 0.4)\" \"abs(0 - 0.4)\""
                );
                count += 1;
            }
            6 if !frags.rows.is_empty() => {
                let f = &frags.rows[rng.below(frags.rows.len())];
                let k = 1 + rng.below(4);
                let _ = writeln!(src, "{indent}insert \"{f}\" k={k}");
                count += k;
            }
            7 | 8 => {
                let _ = writeln!(src, "{indent}at 0.5 0.5 \"u * 0.5\" {{");
                for _ in 0..1 + rng.below(3) {
                    count += gen_2d(rng, frags, src, &format!("{indent}  "));
                }
                let _ = writeln!(src, "{indent}}}");
            }
            _ => {
                let _ = writeln!(src, "{indent}tile 0 {} 0 1 1", rng.below(3));
                count += 1;
            }
        }
    }
    count
}

fn generate(seed: u64) -> Gen {
    let mut rng = Rng::new(seed);
    let mut g = Gen {
        src: String::new(),
        shadow: Shadow::default(),
        bases: Vec::new(),
        axes: Vec::new(),
    };

    // a declared vocabulary holding every token the generator paints
    // with, written or defaulted, sometimes with accent reserved
    let reserved = if rng.chance(50) {
        let reserved = rng.chance(50);
        let _ = writeln!(g.src, "lexicon {{");
        let _ = writeln!(
            g.src,
            "  tokens \"ink\" \"outline\" \"body-top\" \"body-left\" \"body-right\" \"accent\""
        );
        let _ = writeln!(g.src, "  stroke-widths 1.2 1.4");
        if reserved {
            let _ = writeln!(g.src, "  reserved \"accent\"");
        }
        let _ = writeln!(g.src, "}}");
        reserved
    } else {
        false
    };

    // fragments, in both dimensions
    let mut frags = Frags::default();
    for f in 0..rng.below(3) {
        let name = format!("row{f}");
        let _ = writeln!(g.src, "fragment \"{name}\" {{");
        let _ = writeln!(g.src, "  param k 2");
        let _ = writeln!(g.src, "  part \"f\" {{");
        let _ = writeln!(
            g.src,
            "    repeat \"k\" \"j\" {{ tile \"j * 0.3\" 0 0 0.2 0.2 }}"
        );
        let _ = writeln!(g.src, "  }}");
        let _ = writeln!(g.src, "}}");
        frags.rows.push(name);
    }
    for f in 0..rng.below(3) {
        let name = format!("glyph{f}");
        let _ = writeln!(g.src, "fragment \"{name}\" {{");
        let _ = writeln!(g.src, "  param r 2");
        let _ = writeln!(g.src, "  part \"g\" {{");
        let _ = writeln!(g.src, "    circle \"x\" \"y\" \"r\"");
        let _ = writeln!(g.src, "    line \"x - r\" \"y\" \"x + r\" \"y\"");
        let _ = writeln!(g.src, "  }}");
        let _ = writeln!(g.src, "}}");
        frags.glyphs.push(name);
    }

    // a flat mark for badges, painted with the (perhaps reserved) accent
    let mark = rng.chance(60).then(|| {
        let _ = writeln!(g.src, "icon \"mark\" {{");
        let _ = writeln!(g.src, "  category \"state\"");
        let _ = writeln!(g.src, "  canvas 24 24");
        if reserved {
            let _ = writeln!(g.src, "  grant \"accent\" reason=\"the generated alarm\"");
        }
        let _ = writeln!(g.src, "  part \"g\" {{");
        let _ = writeln!(g.src, "    at 0 0 0 {{");
        let _ = writeln!(
            g.src,
            "      poly fill=\"accent\" stroke=\"none\" {{ pt 2 2; pt 22 2; pt 12 22 }}"
        );
        let _ = writeln!(g.src, "    }}");
        let _ = writeln!(g.src, "  }}");
        let _ = writeln!(g.src, "}}");
        g.shadow.icons.insert("mark".to_string(), 1);
    });

    // an axis or two
    let n_axes = rng.below(3);
    for a in 0..n_axes {
        let emit_base = rng.chance(70);
        let n_var = 1 + rng.below(3);
        let name = format!("ax{a}");
        let mut variants = Vec::new();
        let _ = writeln!(g.src, "axis \"{name}\" {{");
        if !emit_base {
            let _ = writeln!(g.src, "  emit-base #false");
        }
        for v in 0..n_var {
            let vname = format!("v{v}");
            let _ = writeln!(g.src, "  variant \"{vname}\" {{");
            let _ = writeln!(g.src, "    part \"vpart-{a}-{v}\" {{");
            let _ = writeln!(g.src, "      tile 0 0 {v} 1 1");
            let _ = writeln!(g.src, "    }}");
            let _ = writeln!(g.src, "  }}");
            variants.push(vname);
        }
        let _ = writeln!(g.src, "}}");
        g.axes.push((name, variants, emit_base));
    }

    // bases
    let n_bases = 1 + rng.below(3);
    for b in 0..n_bases {
        let name = format!("base{b}");
        let _ = writeln!(g.src, "base \"{name}\" {{");
        let _ = writeln!(g.src, "  category \"gen\"");
        let _ = writeln!(g.src, "  param u 1");
        let _ = writeln!(g.src, "  footprint 2 1 \"u * 0.5\"");
        let mut parts = BTreeMap::new();
        let n_parts = 1 + rng.below(2);
        for p in 0..n_parts {
            let pname = format!("p{p}");
            let _ = writeln!(g.src, "  part \"{pname}\" {{");
            let count = gen_part(&mut rng, &frags, &mut g.src, "    ");
            let _ = writeln!(g.src, "  }}");
            parts.insert(pname, count);
        }
        let _ = writeln!(g.src, "}}");
        g.bases.push((name, parts));
    }

    // icons extending bases with overrides
    let n_icons = 1 + rng.below(4);
    for i in 0..n_icons {
        let (bname, bparts) = &g.bases[rng.below(g.bases.len())];
        let name = format!("icon{i}");
        let mut parts = bparts.clone();
        let _ = writeln!(g.src, "icon \"{name}\" {{");
        let _ = writeln!(g.src, "  extends \"{bname}\"");
        if rng.chance(40) {
            let _ = writeln!(g.src, "  param u 2");
        }
        // maybe replace an inherited part
        if rng.chance(50) {
            let target = {
                let keys: Vec<&String> = parts.keys().collect();
                keys[rng.below(keys.len())].clone()
            };
            let _ = writeln!(g.src, "  replace \"{target}\" {{");
            let count = gen_part(&mut rng, &frags, &mut g.src, "    ");
            let _ = writeln!(g.src, "  }}");
            parts.insert(target, count);
        }
        // maybe remove one (only if more than one remains)
        if parts.len() > 1 && rng.chance(30) {
            let target = {
                let keys: Vec<&String> = parts.keys().collect();
                keys[rng.below(keys.len())].clone()
            };
            let _ = writeln!(g.src, "  remove \"{target}\"");
            parts.remove(&target);
        }
        // maybe add a fresh one
        if rng.chance(60) {
            let pname = format!("own{i}");
            let _ = writeln!(g.src, "  part \"{pname}\" {{");
            let count = gen_part(&mut rng, &frags, &mut g.src, "    ");
            let _ = writeln!(g.src, "  }}");
            parts.insert(pname, count);
        }
        // maybe wear the mark, at a random corner: one group
        if mark.is_some() && rng.chance(40) {
            let corner = ["ne", "nw", "se", "sw"][rng.below(4)];
            let _ = writeln!(
                g.src,
                "  part \"badge{i}\" {{ badge \"mark\" corner=\"{corner}\" }}"
            );
            parts.insert(format!("badge{i}"), 1);
        }
        // maybe an axis
        let mut axis_choice: Option<&(String, Vec<String>, bool)> = None;
        if !g.axes.is_empty() && rng.chance(50) {
            axis_choice = Some(&g.axes[rng.below(g.axes.len())]);
            let _ = writeln!(g.src, "  axes \"{}\"", axis_choice.unwrap().0);
        }
        let _ = writeln!(g.src, "}}");

        let body_count: usize = parts.values().sum();
        match axis_choice {
            None => {
                g.shadow.icons.insert(name.clone(), body_count);
            }
            Some((_, variants, emit_base)) => {
                if *emit_base {
                    g.shadow.icons.insert(name.clone(), body_count);
                }
                for v in variants {
                    // each variant adds exactly one tile part
                    g.shadow.icons.insert(format!("{name}-{v}"), body_count + 1);
                }
            }
        }
    }
    g
}

/// The census checker — self-tested below without any compiler run.
fn check_census(shadow: &Shadow, ir: &serde_json::Value) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for icon in ir["icons"].as_array().cloned().unwrap_or_default() {
        let name = icon["name"].as_str().unwrap_or("?").to_string();
        let shapes = icon["shapes"].as_array().map(|a| a.len()).unwrap_or(0);
        seen.insert(name, shapes);
    }
    for (name, want) in &shadow.icons {
        match seen.get(name) {
            None => problems.push(format!("expected icon '{name}' missing from IR")),
            Some(got) if got != want => problems.push(format!(
                "icon '{name}': expected {want} primitives, IR has {got}"
            )),
            _ => {}
        }
    }
    for name in seen.keys() {
        if !shadow.icons.contains_key(name) {
            problems.push(format!("IR contains unexpected icon '{name}'"));
        }
    }
    problems
}

/// Rules any IR from a successful compile keeps, whatever the lexicon:
/// no null anywhere (serde_json writes a non-finite number as null, and
/// nothing else in the IR is optional), every icon name once, and every
/// badge group filled. Self-tested below without any compiler run.
fn check_structure(ir: &serde_json::Value) -> Vec<String> {
    fn nulls(v: &serde_json::Value, path: &str, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Null => out.push(format!("null at {path}")),
            serde_json::Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    nulls(x, &format!("{path}[{i}]"), out);
                }
            }
            serde_json::Value::Object(o) => {
                for (k, x) in o {
                    nulls(x, &format!("{path}.{k}"), out);
                }
            }
            _ => {}
        }
    }
    fn empty_groups(shapes: &[serde_json::Value], icon: &str, out: &mut Vec<String>) {
        for s in shapes {
            if s["kind"] == "group" {
                let inner = s["shapes"].as_array().cloned().unwrap_or_default();
                if inner.is_empty() {
                    out.push(format!("icon '{icon}' has an empty badge group"));
                }
                empty_groups(&inner, icon, out);
            }
        }
    }
    let mut problems = Vec::new();
    nulls(ir, "ir", &mut problems);
    let mut seen = BTreeSet::new();
    for icon in ir["icons"].as_array().cloned().unwrap_or_default() {
        let name = icon["name"].as_str().unwrap_or("?").to_string();
        if !seen.insert(name.clone()) {
            problems.push(format!("icon '{name}' is emitted twice"));
        }
        empty_groups(
            &icon["shapes"].as_array().cloned().unwrap_or_default(),
            &name,
            &mut problems,
        );
    }
    problems
}

struct Harness {
    isolex: String,
    dir: std::path::PathBuf,
}

impl Harness {
    fn run(&self, src: &str, subcmd: &[&str]) -> (bool, String, String, Option<i32>) {
        std::fs::create_dir_all(&self.dir).unwrap();
        std::fs::write(self.dir.join("gen.ilx"), src).unwrap();
        let out = Command::new(&self.isolex)
            .args(subcmd)
            .arg(&self.dir)
            .args(if subcmd == ["compile"] {
                vec!["--ir", "-"]
            } else {
                vec![]
            })
            .output()
            .expect("isolex binary runs");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.status.code(),
        )
    }
}

fn positive_round(h: &Harness, seed: u64) -> Result<(), String> {
    let g = generate(seed);
    let (ok, stdout, stderr, _) = h.run(&g.src, &["compile"]);
    if !ok {
        return Err(format!(
            "compile failed on a generated-valid lexicon:\n{stderr}"
        ));
    }
    let (ok2, stdout2, _, _) = h.run(&g.src, &["compile"]);
    if !ok2 || stdout != stdout2 {
        return Err("double compile was not byte-identical".to_string());
    }
    let ir: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("IR is not JSON: {e}"))?;
    let problems = check_census(&g.shadow, &ir);
    if !problems.is_empty() {
        return Err(format!("census mismatch: {problems:?}"));
    }
    let problems = check_structure(&ir);
    if !problems.is_empty() {
        return Err(format!("structurally invalid IR: {problems:?}"));
    }
    Ok(())
}

/// Nemesis mutations: each must produce a located diagnostic and a
/// non-panic exit.
fn nemesis_round(h: &Harness, seed: u64) -> Result<(), String> {
    let g = generate(seed);
    let mut rng = Rng::new(seed ^ 0xDEAD_BEEF);
    // Truncation can land on a definition boundary and stay valid, so
    // its only claim is "never panic" (found by the battery's own hunt:
    // seeds 424254 and 7777780, promoted to the fixed set).
    let truncated = g.src[..g.src.len() / 2].to_string();
    let (_, _, tstderr, tcode) = h.run(&truncated, &["check"]);
    if !matches!(tcode, Some(0) | Some(1)) {
        return Err(format!(
            "truncation exited with {tcode:?} (panic?):\n{tstderr}"
        ));
    }

    let mutations: Vec<(&str, String)> = vec![
        (
            "unclosed definition",
            format!("{}\nicon \"broken\" {{\n  part \"p\" {{\n", g.src),
        ),
        (
            "extends cycle",
            format!("{}\nbase \"cyc\" {{ extends \"cyc\" }}\n", g.src),
        ),
        (
            "expression abuse",
            format!(
                "{}\nicon \"bad\" {{ category \"c\"; footprint 1 1 \"__import__ + )\" }}\n",
                g.src
            ),
        ),
        (
            "oversized repeat",
            format!(
                "{}\nicon \"huge\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{ repeat 999999 \"i\" {{ tile 0 0 0 1 1 }} }} }}\n",
                g.src
            ),
        ),
        (
            "unknown fragment",
            format!(
                "{}\nicon \"frag\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{ insert \"nope{}\" }} }}\n",
                g.src,
                rng.below(100)
            ),
        ),
        // each repeat small, together 10^6: the shape-limit, not the
        // machine's memory, must stop it (#5)
        (
            "nested repeats",
            format!(
                "{}\nicon \"nest\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{ repeat 1000 \"a\" {{ repeat {} \"b\" {{ tile 0 0 0 1 1 }} }} }} }}\n",
                g.src,
                1000 + rng.below(100)
            ),
        ),
        // a definition spelling a name an axis also produces (#3)
        (
            "colliding names",
            format!(
                "{}\naxis \"nax\" {{ variant \"q\" {{ part \"nq\" {{ tile 0 0 0 1 1 }} }} }}\nicon \"nem\" {{ category \"c\"; footprint 1 1 1; axes \"nax\" }}\nicon \"nem-q\" {{ category \"c\"; footprint 1 1 1 }}\n",
                g.src
            ),
        ),
        // the anchor's names, as a param (#6)
        (
            "reserved name",
            format!(
                "{}\nicon \"nxy\" {{ category \"c\"; footprint 1 1 1; param {} 1 }}\n",
                g.src,
                ["x", "y"][rng.below(2)]
            ),
        ),
        // a value past f64's range (#2)
        (
            "overflow",
            format!(
                "{}\nicon \"nbig\" {{ category \"c\"; footprint 1 1 1; param big 1e300; part \"p\" {{ box 0 0 0 1 1 \"big * big\" }} }}\n",
                g.src
            ),
        ),
    ];
    // with a vocabulary declared, a token outside it (#8)
    let mut mutations = mutations;
    if g.src.contains("lexicon {") {
        mutations.push((
            "undeclared token",
            format!(
                "{}\nicon \"ntok\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{ tile 0 0 0 1 1 fill=\"inkk\" }} }}\n",
                g.src
            ),
        ));
    }
    // every mutation, so one that fails does not hide the next
    let mut failures = Vec::new();
    for (what, src) in mutations {
        let (ok, _, stderr, code) = h.run(&src, &["check"]);
        if ok {
            failures.push(format!("nemesis '{what}' was accepted as valid"));
        } else if code != Some(1) {
            failures.push(format!(
                "nemesis '{what}' exited with {code:?} instead of a clean diagnostic failure (panic?):\n{stderr}"
            ));
        } else if !stderr.contains("gen.ilx:") {
            failures.push(format!(
                "nemesis '{what}' produced no located diagnostic:\n{stderr}"
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

/// Shrink by repeatedly dropping the last half of top-level definitions
/// while the failure reproduces.
fn shrink(h: &Harness, src: &str) -> String {
    let mut current = src.to_string();
    loop {
        let defs: Vec<&str> = current.split("\n}").collect();
        if defs.len() <= 2 {
            return current;
        }
        let keep = defs.len() / 2;
        let candidate = defs[..keep].join("\n}") + "\n}";
        let (ok, _, _, _) = h.run(&candidate, &["compile"]);
        if !ok {
            current = candidate;
        } else {
            return current;
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut isolex = std::env::var("ISOLEX_BIN").unwrap_or_else(|_| "target/release/isolex".into());
    let mut seeds_file = String::from("ci/battery-seeds.txt");
    let mut rounds = 1usize;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--isolex" => isolex = it.next().expect("--isolex <path>").clone(),
            "--seeds" => seeds_file = it.next().expect("--seeds <file>").clone(),
            "--rounds" => rounds = it.next().expect("--rounds <n>").parse().unwrap(),
            other => {
                eprintln!("unknown arg {other}");
                std::process::exit(2);
            }
        }
    }

    let seeds: Vec<u64> = if let Ok(s) = std::env::var("SEED") {
        vec![s.parse().expect("SEED must be a number")]
    } else {
        std::fs::read_to_string(&seeds_file)
            .unwrap_or_else(|_| panic!("cannot read {seeds_file}"))
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|l| l.trim().parse().expect("seed lines are numbers"))
            .collect()
    };

    let mut failures = 0;
    for &seed in &seeds {
        for round in 0..rounds {
            let s = seed.wrapping_add(round as u64);
            let h = Harness {
                isolex: isolex.clone(),
                dir: std::env::temp_dir().join(format!("isolex-battery-{s}")),
            };
            match positive_round(&h, s) {
                Ok(()) => println!("seed {s}: positive ok"),
                Err(e) => {
                    failures += 1;
                    let g = generate(s);
                    let minimal = shrink(&h, &g.src);
                    eprintln!(
                        "seed {s}: POSITIVE FAILED: {e}\n--- shrunk reproducer ---\n{minimal}"
                    );
                }
            }
            match nemesis_round(&h, s) {
                Ok(()) => println!("seed {s}: nemesis ok"),
                Err(e) => {
                    failures += 1;
                    eprintln!("seed {s}: NEMESIS FAILED: {e}");
                }
            }
        }
    }
    if failures > 0 {
        eprintln!("battery: {failures} failure(s)");
        std::process::exit(1);
    }
    println!("battery: all seeds green");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stackless oracle self-test: the census checker must complain when
    /// fed compiler output a broken engine would produce.
    #[test]
    fn census_checker_fires_on_doctored_ir() {
        let mut shadow = Shadow::default();
        shadow.icons.insert("a".into(), 3);
        shadow.icons.insert("b".into(), 1);

        // missing icon
        let ir: serde_json::Value = serde_json::json!({"icons": [
            {"name": "a", "shapes": [1, 2, 3]}
        ]});
        let p = check_census(&shadow, &ir);
        assert!(
            p.iter().any(|m| m.contains("expected icon 'b' missing")),
            "{p:?}"
        );

        // wrong shape count
        let ir = serde_json::json!({"icons": [
            {"name": "a", "shapes": [1]}, {"name": "b", "shapes": [1]}
        ]});
        let p = check_census(&shadow, &ir);
        assert!(
            p.iter().any(|m| m.contains("expected 3 primitives")),
            "{p:?}"
        );

        // phantom icon
        let ir = serde_json::json!({"icons": [
            {"name": "a", "shapes": [1,2,3]}, {"name": "b", "shapes": [1]},
            {"name": "ghost", "shapes": []}
        ]});
        let p = check_census(&shadow, &ir);
        assert!(
            p.iter().any(|m| m.contains("unexpected icon 'ghost'")),
            "{p:?}"
        );

        // clean baseline stays quiet
        let ir = serde_json::json!({"icons": [
            {"name": "a", "shapes": [1,2,3]}, {"name": "b", "shapes": [1]}
        ]});
        assert!(check_census(&shadow, &ir).is_empty());
    }

    #[test]
    fn generator_is_deterministic_per_seed() {
        assert_eq!(generate(42).src, generate(42).src);
        assert_ne!(generate(42).src, generate(43).src);
    }

    /// Stackless oracle self-test: the structure checker must complain
    /// about IR a broken engine would produce, and only that.
    #[test]
    fn structure_checker_fires_on_doctored_ir() {
        let clean = serde_json::json!({"ir_version": 1, "icons": [
            {"name": "a", "viewbox": [0.0, 0.0, 1.0, 1.0], "shapes": [
                {"kind": "group", "tx": 0.0, "ty": 0.0, "scale": 0.5, "shapes": [
                    {"kind": "circle", "cx": 1.0}
                ]}
            ]},
            {"name": "b", "viewbox": [0.0, 0.0, 1.0, 1.0], "shapes": []}
        ]});
        assert_eq!(check_structure(&clean), Vec::<String>::new());

        // a non-finite number, as serde_json writes one
        let mut ir = clean.clone();
        ir["icons"][1]["viewbox"][2] = serde_json::Value::Null;
        let p = check_structure(&ir);
        assert!(
            p.iter()
                .any(|m| m.contains("null at ir.icons[1].viewbox[2]")),
            "{p:?}"
        );

        // a name twice
        let mut ir = clean.clone();
        ir["icons"][1]["name"] = "a".into();
        let p = check_structure(&ir);
        assert!(
            p.iter().any(|m| m.contains("'a' is emitted twice")),
            "{p:?}"
        );

        // a badge left unfilled
        let mut ir = clean.clone();
        ir["icons"][0]["shapes"][0]["shapes"] = serde_json::json!([]);
        let p = check_structure(&ir);
        assert!(
            p.iter().any(|m| m.contains("'a' has an empty badge group")),
            "{p:?}"
        );
    }

    /// The generator really emits each construct it is meant to cover,
    /// somewhere in the fixed seeds' neighborhood.
    #[test]
    fn generator_covers_the_language() {
        let all: String = (0..200).map(|s| generate(s).src).collect();
        for construct in [
            "lexicon {",
            "reserved \"accent\"",
            "fragment \"",
            "insert \"row",
            "insert \"glyph",
            "at 0.5 0.5",
            "line \"x",
            "circle \"x",
            "path d=",
            "poly {",
            "pt \"x",
            "when=\"",
            "if(",
            "cosr(",
            "badge \"mark\"",
            "grant \"accent\"",
            "axis \"",
            "emit-base #false",
            "replace \"",
            "remove \"",
        ] {
            assert!(all.contains(construct), "no seed generates {construct}");
        }
    }
}
