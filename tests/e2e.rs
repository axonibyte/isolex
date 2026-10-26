//! End-to-end CLI tests: the compiled binary against the sample lexicon.
//! The IR golden is the compiler's output contract; UPDATE_GOLDENS=1
//! regenerates it and the diff is the review artifact.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_isolex"))
}

fn lexicon_dir() -> PathBuf {
    // a directory containing only the sample fixture
    let dir = std::env::temp_dir().join(format!("isolex-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sample.ilx"), include_str!("fixtures/sample.ilx")).unwrap();
    dir
}

#[test]
fn check_reports_counts_and_exits_zero() {
    let dir = lexicon_dir();
    let out = bin().arg("check").arg(&dir).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("10 definition(s), 7 icon(s) emitted"),
        "{stdout}"
    );
}

#[test]
fn compile_matches_the_ir_golden_and_is_deterministic() {
    let dir = lexicon_dir();
    let run = || {
        let out = bin()
            .arg("compile")
            .arg(&dir)
            .arg("--ir")
            .arg("-")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "double compile must be byte-identical");

    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sample.ir.golden.json"
    );
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(golden_path, &a).unwrap();
    }
    let want = std::fs::read_to_string(golden_path)
        .expect("golden missing: run with UPDATE_GOLDENS=1 once, then review the diff");
    assert_eq!(a, want, "IR drifted from the golden");
}

#[test]
fn ir_carries_axis_expansion_and_badge_groups() {
    let dir = lexicon_dir();
    let out = bin()
        .arg("compile")
        .arg(&dir)
        .arg("--ir")
        .arg("-")
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let names: Vec<&str> = json["icons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"switch-poe"));
    assert!(names.contains(&"switch-poe-down"));
    assert!(names.contains(&"switch-poe-planned"));

    // the badge on switch-poe resolved into a group holding the state
    // icon's real shapes, not a placeholder
    let poe = &json["icons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "switch-poe")
        .unwrap();
    let group = poe["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "group")
        .expect("badge group present");
    assert!(
        group["shapes"].as_array().unwrap().len() >= 2,
        "badge group carries the target icon's shapes"
    );
    assert_eq!(group["matrix"][0], 0.5);
    assert_eq!(group["matrix"][3], 0.5);
}

#[test]
fn docs_emits_reference_tables() {
    let dir = lexicon_dir();
    let out = bin().arg("docs").arg(&dir).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("## net"), "{stdout}");
    assert!(
        stdout.contains("Adds the routing-arrows badge on the top face. |"),
        "{stdout}"
    );
    let l3_row = stdout.lines().find(|l| l.contains("`switch-l3`")).unwrap();
    assert!(
        l3_row.contains("Layer-2 access switch."),
        "describe concatenates the chain: {l3_row}"
    );
}

/// `isolex check` on a lexicon of the given files: (succeeded, stderr).
fn check_files(tag: &str, files: &[(&str, &str)]) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!("isolex-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in files {
        std::fs::write(dir.join(name), text).unwrap();
    }
    let out = bin().arg("check").arg(&dir).output().unwrap();
    (out.status.success(), String::from_utf8(out.stderr).unwrap())
}

#[test]
fn an_expression_that_overflows_is_a_located_error() {
    // serde_json writes a non-finite f64 as null: this compiled, exit 0,
    // with eight nulls where the box's top corners belong
    let (ok, stderr) = check_files(
        "overflow",
        &[(
            "huge.ilx",
            "icon \"huge\" {\n  category \"c\"\n  param a 1e300\n  footprint 1 1 1\n  part \"p\" { box 0 0 0 1 1 \"a * a\" }\n}\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("huge.ilx:5:14: in 'a * a': the result is inf, not a finite number"),
        "{stderr}"
    );
}

#[test]
fn geometry_that_overflows_when_projected_is_reported_at_its_node() {
    // every value finite, but 1e307 tile units are past f64's range once
    // projected to output units
    let (ok, stderr) = check_files(
        "projected",
        &[(
            "wide.ilx",
            "icon \"ok\" { category \"c\"; footprint 1 1 1; part \"p\" { box 0 0 0 1 1 1 } }\nicon \"wide\" {\n  category \"c\"\n  footprint 1 1 1\n  part \"p\" { box 0 0 0 1e307 1 1 }\n}\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains(
            "wide.ilx:5:14: icon 'wide' projects to inf, not a finite number: its geometry is too large to draw"
        ),
        "{stderr}"
    );
    assert!(!stderr.contains("'ok'"), "{stderr}");

    // path data is formatted at projection, where an overflow is text
    let (ok, stderr) = check_files(
        "projected-path",
        &[(
            "seg.ilx",
            "icon \"far\" { category \"c\"; footprint 1 1 1; part \"p\" { seg 0 0 0 1e307 0 0 } }\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("seg.ilx:1:56: icon 'far' projects to inf, not a finite number"),
        "{stderr}"
    );
}

#[test]
fn an_anchor_expression_that_overflows_is_an_error() {
    // x is only known at projection, so this one is evaluated there
    let (ok, stderr) = check_files(
        "anchor",
        &[(
            "at.ilx",
            "icon \"a\" { category \"c\"; footprint 1 1 1; param big 1e307\n  part \"p\" { at 1 0 0 { circle \"x * big\" 0 1 } } }\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("at.ilx:2:25: in 'x * big': the result is inf, not a finite number"),
        "{stderr}"
    );
}

#[test]
fn badge_errors_are_reported_at_the_badge() {
    // these were all reported at the first file's 1:1, wherever they were
    let (ok, stderr) = check_files(
        "badge",
        &[
            (
                "a.ilx",
                "icon \"first\" { category \"c\"; footprint 1 1 1; part \"p\" { tile 0 0 0 1 1 } }\n",
            ),
            (
                "b.ilx",
                "\n\n\nicon \"second\" {\n  category \"c\"\n  footprint 1 1 1\n  part \"p\" {\n    badge \"nosuch\"\n  }\n}\n",
            ),
        ],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("b.ilx:8:5: badge references unknown icon 'nosuch'"),
        "{stderr}"
    );

    let (ok, stderr) = check_files(
        "badge-nested",
        &[(
            "n.ilx",
            "icon \"dot\" { category \"c\"; canvas 4 4; part \"p\" { at 0 0 0 { circle 2 2 1 } } }\nicon \"marked\" { category \"c\"; footprint 1 1 1\n  part \"p\" { badge \"dot\" } }\nicon \"outer\" { category \"c\"; footprint 1 1 1\n  part \"p\" { badge \"marked\" } }\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr
            .contains("n.ilx:5:14: badge target 'marked' itself contains a badge — one level only"),
        "{stderr}"
    );
}

#[test]
fn anchor_expression_errors_are_reported_at_their_node() {
    let (ok, stderr) = check_files(
        "anchor-errors",
        &[(
            "e.ilx",
            "icon \"e\" { category \"c\"; footprint 1 1 1\n  part \"p\" {\n    at 1 0 0 {\n      circle \"1 / (x - x)\" 0 1\n      path d=\"M{x},{1 / (y - y)}\"\n    }\n  }\n}\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("e.ilx:4:7: in '1 / (x - x)': division by zero"),
        "{stderr}"
    );
    assert!(
        stderr.contains("e.ilx:5:7: in '1 / (y - y)': division by zero"),
        "{stderr}"
    );
}

#[test]
fn a_name_emitted_twice_is_reported_where_it_is_emitted_again() {
    let (ok, stderr) = check_files(
        "collide",
        &[(
            "names.ilx",
            "axis \"state\" { variant \"down\" { part \"b\" { tile 0 0 1 1 1 } } }\nicon \"srv\" { category \"c\"; footprint 1 1 1; axes \"state\"; part \"p\" { tile 0 0 0 1 1 } }\nicon \"srv-down\" { category \"c\"; footprint 1 1 1; part \"p\" { box 0 0 0 1 1 1 } }\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("names.ilx:3:1: 'srv-down' is emitted twice"),
        "{stderr}"
    );
}

#[test]
fn nested_repeats_fail_cleanly_at_the_shape_limit() {
    // the lexicon from #5: 100 million tiles, which aborted (exit 134)
    // allocating 5.1 GB; it now stops at the default shape-limit
    let dir = std::env::temp_dir().join(format!("isolex-e2e-limit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("big.ilx"),
        "icon \"big\" { category \"c\"; footprint 1 1 1; part \"p\" {\n  repeat 10000 \"a\" { repeat 10000 \"b\" { tile 0 0 0 1 1 } } } }\n",
    )
    .unwrap();
    let out = bin().arg("check").arg(&dir).output().unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("big.ilx:2:41: icon 'big' goes here past its shape-limit of 100000"),
        "{stderr}"
    );
}

#[test]
fn files_are_read_directory_by_directory_in_name_order() {
    // SPEC §1: each directory's entries in byte order of their names, a
    // directory's files where its name sorts. So a/ comes before a-b/,
    // where sorting whole paths as strings would put "a-b/" first ('-'
    // is 0x2D, '/' is 0x2F) and change the emission order.
    let dir = std::env::temp_dir().join(format!("isolex-e2e-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, icon) in [
        ("a-b/x.ilx", "second"),
        ("a/x.ilx", "first"),
        ("b.ilx", "third"),
    ] {
        let file = dir.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            file,
            format!("icon \"{icon}\" {{ category \"c\"; footprint 1 1 1 }}\n"),
        )
        .unwrap();
    }
    let out = bin()
        .arg("compile")
        .arg(&dir)
        .arg("--ir")
        .arg("-")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let names: Vec<&str> = json["icons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["first", "second", "third"]);
}

#[test]
fn undeclared_tokens_and_widths_are_errors() {
    // the lexicon from #8, which compiled clean
    let (ok, stderr) = check_files(
        "vocab",
        &[(
            "v.ilx",
            "lexicon { tokens \"ink\" \"outline\" \"body-top\"; stroke-widths 1.2 }\nicon \"t\" { category \"c\"; footprint 1 1 1\n  part \"p\" { tile 0 0 0 1 1 fill=\"inkk\" width=9.9 } }\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains(
            "v.ilx:3:14: icon 't' paints with 'inkk', which is not one of the lexicon's tokens \
             (did you mean 'ink'?); declare it in the lexicon block, or paint this node with one \
             that is"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "v.ilx:3:14: icon 't' strokes at width 9.90, which is not one of the lexicon's \
             stroke-widths (1.20)"
        ),
        "{stderr}"
    );
}

#[test]
fn program_defaults_are_held_to_the_vocabulary() {
    // SPEC §2: a token or width the compiler fills in reaches the IR as if
    // the author had written it, so it is held to the same declarations
    let (ok, stderr) = check_files(
        "defaults",
        &[(
            "d.ilx",
            "lexicon { tokens \"ink\"; stroke-widths 1.4 }\nicon \"b\" { category \"c\"; footprint 1 1 1; part \"p\" { box 0 0 0 1 1 1 } }\n",
        )],
    );
    assert!(!ok, "{stderr}");
    for token in ["body-left", "body-right", "body-top", "outline"] {
        assert!(
            stderr.contains(&format!("d.ilx:2:54: icon 'b' paints with '{token}'")),
            "{token}: {stderr}"
        );
    }
    assert!(
        stderr.contains("d.ilx:2:54: icon 'b' strokes at width 1.20"),
        "{stderr}"
    );
    // each once, though the box has three faces
    assert_eq!(stderr.matches("'outline'").count(), 1, "{stderr}");

    // declared, they are fine
    let (ok, stderr) = check_files(
        "defaults-declared",
        &[(
            "d.ilx",
            "lexicon { tokens \"body-left\" \"body-right\" \"body-top\" \"outline\"; stroke-widths 1.2 }\nicon \"b\" { category \"c\"; footprint 1 1 1; part \"p\" { box 0 0 0 1 1 1 } }\n",
        )],
    );
    assert!(ok, "{stderr}");
}

#[test]
fn none_is_no_paint_and_an_unstroked_width_is_not_checked() {
    // a facerect fills with ink and has no stroke (width 0): neither
    // `none` nor its 0 needs declaring
    let (ok, stderr) = check_files(
        "none",
        &[(
            "n.ilx",
            "lexicon { tokens \"ink\"; stroke-widths 1.4 }\nicon \"f\" { category \"c\"; footprint 1 1 1; part \"p\" { facerect \"front\" 0 1 0 1 1 } }\n",
        )],
    );
    assert!(ok, "{stderr}");
}

#[test]
fn widths_compare_at_the_precision_a_renderer_sees() {
    // 1.401 is written out as 1.40, the declared 1.4; 1.406 as 1.41
    let lexicon = "lexicon { tokens \"ink\" \"outline\"; stroke-widths 1.4 }\n";
    let at = |w: &str| {
        format!(
            "{lexicon}icon \"g\" {{ category \"c\"; canvas 9 9; part \"p\" {{ at 0 0 0 {{ circle 4 4 3 width={w} }} }} }}\n"
        )
    };
    let (ok, stderr) = check_files("w1", &[("w.ilx", &at("1.401"))]);
    assert!(ok, "{stderr}");
    let (ok, stderr) = check_files("w2", &[("w.ilx", &at("1.406"))]);
    assert!(!ok && stderr.contains("strokes at width 1.41"), "{stderr}");
}

#[test]
fn a_glyph_inside_at_is_reported_at_its_own_line() {
    let (ok, stderr) = check_files(
        "glyph",
        &[(
            "g.ilx",
            "lexicon { tokens \"ink\" \"outline\" \"body-top\" \"body-left\" \"body-right\"; stroke-widths 1.2 1.4 }\nicon \"g\" { category \"c\"; footprint 1 1 1\n  part \"p\" {\n    at 0.5 0.5 1 {\n      line 0 0 1 1\n      circle \"x\" \"y\" 3 stroke=\"inq\"\n    }\n  }\n}\n",
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("g.ilx:6:7: icon 'g' paints with 'inq'")
            && stderr.contains("did you mean 'ink'?"),
        "{stderr}"
    );
}

#[test]
fn without_a_declared_vocabulary_nothing_is_checked() {
    let icon = "icon \"i\" { category \"c\"; footprint 1 1 1; part \"p\" { tile 0 0 0 1 1 fill=\"anything\" width=7 } }\n";
    let (ok, stderr) = check_files("novocab", &[("i.ilx", icon)]);
    assert!(ok, "{stderr}");
    // a lexicon block that declares other things declares no vocabulary
    let (ok, stderr) = check_files(
        "novocab2",
        &[("i.ilx", &format!("lexicon {{ shape-limit 50 }}\n{icon}"))],
    );
    assert!(ok, "{stderr}");
}

const RESERVING: &str = "lexicon {\n  tokens \"ink\" \"outline\" \"accent\" \"body-left\" \"body-right\" \"body-top\"\n  reserved \"accent\"\n}\n";

#[test]
fn a_reserved_token_needs_a_grant() {
    let red = "part \"p\" { at 0 0 0 { circle 4 4 3 fill=\"accent\" stroke=\"none\" } }";
    let (ok, stderr) = check_files(
        "reserved",
        &[(
            "r.ilx",
            &format!("{RESERVING}icon \"down\" {{ category \"c\"; canvas 9 9\n  {red} }}\n"),
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains(
            "r.ilx:6:25: icon 'down' paints with 'accent', which the lexicon reserves; grant it \
             in this icon's definition chain with `grant \"accent\" reason=\"…\"`"
        ),
        "{stderr}"
    );

    // granted directly, or by an ancestor
    let (ok, stderr) = check_files(
        "reserved-granted",
        &[(
            "r.ilx",
            &format!(
                "{RESERVING}icon \"down\" {{ category \"c\"; canvas 9 9; grant \"accent\" reason=\"danger\"\n  {red} }}\n"
            ),
        )],
    );
    assert!(ok, "{stderr}");
    let (ok, stderr) = check_files(
        "reserved-inherited",
        &[(
            "r.ilx",
            &format!(
                "{RESERVING}base \"alarm\" {{ category \"c\"; canvas 9 9; grant \"accent\" reason=\"danger\" }}\nicon \"down\" {{ extends \"alarm\"\n  {red} }}\n"
            ),
        )],
    );
    assert!(ok, "{stderr}");
}

#[test]
fn a_reserved_default_needs_a_grant_too() {
    // SPEC §1, Defaults: a box's outline reaches the IR as if written
    let lexicon = RESERVING.replace("reserved \"accent\"", "reserved \"outline\"");
    let boxed = "part \"p\" { box 0 0 0 1 1 1 }";
    let (ok, stderr) = check_files(
        "reserved-default",
        &[(
            "r.ilx",
            &format!("{lexicon}icon \"b\" {{ category \"c\"; footprint 1 1 1; {boxed} }}\n"),
        )],
    );
    assert!(
        !ok && stderr.contains("icon 'b' paints with 'outline', which the lexicon reserves"),
        "{stderr}"
    );
    let (ok, stderr) = check_files(
        "reserved-default-granted",
        &[(
            "r.ilx",
            &format!(
                "{lexicon}icon \"b\" {{ category \"c\"; footprint 1 1 1; grant \"outline\" reason=\"edges\"; {boxed} }}\n"
            ),
        )],
    );
    assert!(ok, "{stderr}");
}

#[test]
fn a_badge_is_granted_by_its_target() {
    // the badge's red is the target's, which grants it; the icon wearing
    // the badge paints no red of its own
    let (ok, stderr) = check_files(
        "reserved-badge",
        &[(
            "r.ilx",
            &format!(
                "{RESERVING}icon \"dot\" {{ category \"c\"; canvas 9 9; grant \"accent\" reason=\"danger\"\n  part \"p\" {{ at 0 0 0 {{ circle 4 4 3 fill=\"accent\" stroke=\"none\" }} }} }}\nicon \"srv\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{ badge \"dot\" }} }}\n"
            ),
        )],
    );
    assert!(ok, "{stderr}");
}

#[test]
fn a_grant_of_a_token_that_is_not_reserved() {
    // with tokens reserved, it is an error
    let (ok, stderr) = check_files(
        "stray-grant",
        &[(
            "g.ilx",
            &format!(
                "{RESERVING}icon \"i\" {{ category \"c\"; canvas 9 9\n  grant \"acent\" reason=\"typo\" }}\n"
            ),
        )],
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains(
            "g.ilx:6:3: 'i' grants 'acent', which the lexicon does not reserve (did you mean \
             'accent'?); reserve it in the lexicon block, or remove the grant"
        ),
        "{stderr}"
    );

    // with none reserved, it has no effect: a warning, and nothing else
    // changes, the exit status included
    let src = "axis \"state\" { variant \"down\" { grant \"accent\" reason=\"danger\" } }\nicon \"i\" { category \"c\"; canvas 9 9; axes \"state\" }\n";
    let (ok, stderr) = check_files("stray-warning", &[("w.ilx", src)]);
    assert!(ok, "{stderr}");
    assert!(
        stderr.contains(
            "w.ilx:1:33: warning: variant 'down' of axis 'state' grants 'accent', but the lexicon \
             reserves no tokens, so the grant has no effect; reserve it with `reserved \"accent\"` \
             in the lexicon block, or remove the grant"
        ),
        "{stderr}"
    );
}

/// Compile `src` to IR on the command line: (succeeded, stderr, IR).
fn compile_src(tag: &str, src: &str) -> (bool, String, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("isolex-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("n.ilx"), src).unwrap();
    let out = bin()
        .arg("compile")
        .arg(&dir)
        .arg("--ir")
        .arg("-")
        .output()
        .unwrap();
    let ir = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);
    (
        out.status.success(),
        String::from_utf8(out.stderr).unwrap(),
        ir,
    )
}

#[test]
fn deep_node_nesting_never_overflows_the_stack() {
    // #28: nested blocks overflowed the stack in the kdl crate at a few
    // thousand, and then in the parser, the AST and the expander
    const N: usize = 100_000;
    let deep = |open: &str, inner: &str| format!("{}{inner}\n{}", open.repeat(N), "}\n".repeat(N));

    // repeats among 3-D shapes
    let src = format!(
        "icon \"deep\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{\n{} }} }}\n",
        deep("repeat 1 \"i\" {\n", "tile 0 0 0 1 1")
    );
    let (ok, stderr, ir) = compile_src("deep3d", &src);
    assert!(ok, "{}", &stderr[..stderr.len().min(300)]);
    assert_eq!(ir["icons"][0]["shapes"].as_array().unwrap().len(), 1);

    // and among 2-D ones, inside an at
    let src = format!(
        "icon \"deep\" {{ category \"c\"; canvas 9 9; part \"p\" {{ at 0 0 0 {{\n{} }} }} }}\n",
        deep("repeat 1 \"i\" {\n", "circle 4 4 2")
    );
    let (ok, stderr, ir) = compile_src("deep2d", &src);
    assert!(ok, "{}", &stderr[..stderr.len().min(300)]);
    assert_eq!(ir["icons"][0]["shapes"][0]["kind"], "circle");

    // a chain of fragments, each inserting the next: past the default
    // insert-depth a located error, and with none, all the way down
    const F: usize = 10_000;
    let mut chain = String::new();
    for f in 0..F {
        chain.push_str(&format!(
            "fragment \"f{f}\" {{ part \"p\" {{ insert \"f{}\" }} }}\n",
            f + 1
        ));
    }
    chain.push_str(&format!(
        "fragment \"f{F}\" {{ part \"p\" {{ tile 0 0 0 1 1 }} }}\n"
    ));
    let icon = "icon \"chain\" { category \"c\"; footprint 1 1 1; part \"p\" { insert \"f0\" } }\n";
    let (ok, stderr, _) = compile_src("chain", &format!("{chain}{icon}"));
    assert!(!ok);
    assert!(
        stderr.contains("icon 'chain' nests fragment insertions here past its insert-depth of 32"),
        "{}",
        &stderr[..stderr.len().min(300)]
    );
    let (ok, stderr, ir) = compile_src(
        "chain-unlimited",
        &format!("lexicon {{ insert-depth #null }}\n{chain}{icon}"),
    );
    assert!(ok, "{}", &stderr[..stderr.len().min(300)]);
    assert_eq!(ir["icons"][0]["shapes"].as_array().unwrap().len(), 1);
}

#[test]
fn deep_expressions_never_overflow_the_stack() {
    // #18: 100 000 parentheses around a 1 aborted the compiler (exit 134,
    // "has overflowed its stack"). Parentheses add no depth: it compiles.
    let parens = format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000));
    let (ok, stderr) = check_files(
        "deep-parens",
        &[(
            "d.ilx",
            &format!("icon \"d\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{ box 0 0 0 1 1 \"{parens}\" }} }}\n"),
        )],
    );
    assert!(ok, "{stderr}");

    // a tree 100 001 deep is past the default expression-depth, a
    // located error and exit 1, not a crash
    let deep = format!("{}1{}", "-(".repeat(100_000), ")".repeat(100_000));
    let src = |limit: &str| {
        format!(
            "{limit}icon \"d\" {{ category \"c\"; footprint 1 1 1\n  part \"p\" {{ box 0 0 0 1 1 \"{deep}\" }} }}\n"
        )
    };
    let (ok, stderr) = check_files("deep-tree", &[("d.ilx", &src(""))]);
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("d.ilx:2:14: icon 'd' nests an expression 100001 deep here past its expression-depth of 256"),
        "{}",
        &stderr[..stderr.len().min(400)]
    );

    // and with no limit at all, it compiles: depth costs memory, never
    // the stack
    let (ok, stderr) = check_files(
        "deep-tree-unlimited",
        &[("d.ilx", &src("lexicon { expression-depth #null }\n"))],
    );
    assert!(ok, "{}", &stderr[..stderr.len().min(400)]);
}

/// Run `isolex render` on a lexicon of the given files: (succeeded,
/// stderr, the output directory).
fn render(tag: &str, files: &[(&str, &str)], palette: Option<&str>) -> (bool, String, PathBuf) {
    let base = std::env::temp_dir().join(format!("isolex-e2e-render-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let src = base.join("src");
    std::fs::create_dir_all(&src).unwrap();
    for (name, text) in files {
        std::fs::write(src.join(name), text).unwrap();
    }
    let out = base.join("svg");
    let mut cmd = bin();
    cmd.arg("render").arg(&src).arg("--svg").arg(&out);
    if let Some(p) = palette {
        let path = base.join("palette.json");
        std::fs::write(&path, p).unwrap();
        cmd.arg("--palette").arg(path);
    }
    let run = cmd.output().unwrap();
    (
        run.status.success(),
        String::from_utf8(run.stderr).unwrap(),
        out,
    )
}

/// Every SVG in a directory, in name order, as one text.
fn svgs(dir: &std::path::Path) -> String {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
        .iter()
        .map(|n| {
            format!(
                "== {n} ==\n{}",
                std::fs::read_to_string(dir.join(n)).unwrap()
            )
        })
        .collect()
}

#[test]
fn render_matches_the_svg_golden_and_is_deterministic() {
    let sample = include_str!("fixtures/sample.ilx");
    let (ok, stderr, out) = render("golden", &[("sample.ilx", sample)], None);
    assert!(ok, "{stderr}");
    let a = svgs(&out);
    let (_, _, again) = render("golden-again", &[("sample.ilx", sample)], None);
    assert_eq!(a, svgs(&again), "a second render must be byte-identical");
    // one file per icon the IR holds
    assert_eq!(a.matches("== ").count(), 7, "{a}");

    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sample.svg.golden"
    );
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(golden_path, &a).unwrap();
    }
    let want = std::fs::read_to_string(golden_path)
        .expect("golden missing: run with UPDATE_GOLDENS=1 once, then review the diff");
    assert_eq!(a, want, "SVG drifted from the golden");
}

#[test]
fn render_takes_a_palette_over_the_built_in_one() {
    let icon = "icon \"i\" { category \"c\"; canvas 9 9; part \"p\" { at 0 0 0 { circle 4 4 3 fill=\"brand\" stroke=\"ink\" } } }\n";
    // brand has no built-in color: nothing is written
    let (ok, stderr, out) = render("nopalette", &[("i.ilx", icon)], None);
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("the palette has no color for 'brand' (painted by i); add it to a palette file and pass --palette"),
        "{stderr}"
    );
    assert!(!out.exists(), "a failed render writes nothing");

    let (ok, stderr, out) = render(
        "palette",
        &[("i.ilx", icon)],
        Some(r##"{"brand": "#c00", "ink": "black"}"##),
    );
    assert!(ok, "{stderr}");
    let svg = std::fs::read_to_string(out.join("i.svg")).unwrap();
    assert!(
        svg.contains("<circle cx=\"4.00\" cy=\"4.00\" r=\"3.00\" fill=\"#c00\" stroke=\"black\" stroke-width=\"1.20\"/>"),
        "{svg}"
    );

    let (ok, stderr, _) = render("badpalette", &[("i.ilx", icon)], Some("[\"red\"]"));
    assert!(!ok && stderr.contains("must be a JSON object"), "{stderr}");
}

#[test]
fn render_refuses_an_icon_name_that_is_not_a_file_name() {
    // icon names become file names; this one would climb out of the
    // output directory
    let (ok, stderr, out) = render(
        "unsafe",
        &[(
            "u.ilx",
            "icon \"../escape\" { category \"c\"; canvas 9 9; part \"p\" { at 0 0 0 { circle 4 4 3 } } }\n",
        )],
        None,
    );
    assert!(!ok, "{stderr}");
    assert!(
        stderr.contains("icon '../escape' cannot be written as a file name"),
        "{stderr}"
    );
    assert!(!out.exists() && !out.parent().unwrap().join("escape.svg").exists());
}

#[test]
fn path_data_never_writes_negative_zero() {
    // {0 - 0.001} is formatted in expansion, {x - 0.001} at projection,
    // once x is known; both wrote -0.00
    let dir = std::env::temp_dir().join(format!("isolex-e2e-negzero-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("z.ilx"),
        "icon \"z\" { category \"c\"; canvas 9 9\n  part \"p\" { at 0 0 0 { path d=\"M{0 - 0.001},0 L{x - 0.001},{y - 0.004}\" } } }\n",
    )
    .unwrap();
    let out = bin()
        .arg("compile")
        .arg(&dir)
        .arg("--ir")
        .arg("-")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["icons"][0]["shapes"][0]["d"], "M0.00,0 L0.00,0.00");
}

#[test]
fn check_failures_render_file_line_col_and_exit_nonzero() {
    let dir = std::env::temp_dir().join(format!("isolex-e2e-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bad.ilx"), "icon \"x\" {\n  extends \"nope\"\n}\n").unwrap();
    let out = bin().arg("check").arg(&dir).output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("bad.ilx:1:1: 'x' extends unknown definition 'nope'"),
        "{stderr}"
    );
}

// --- transforms (#43) ---------------------------------------------------

/// The transforms fixture compiled and rendered in-process: its files,
/// the IR, and each icon's SVG.
fn transforms() -> Vec<(String, String)> {
    vec![(
        "transforms.ilx".to_string(),
        include_str!("fixtures/transforms.ilx").to_string(),
    )]
}

fn sample() -> Vec<(String, String)> {
    vec![(
        "sample.ilx".to_string(),
        include_str!("fixtures/sample.ilx").to_string(),
    )]
}

/// Golden bytes, rewritten with UPDATE_GOLDENS=1.
fn golden(name: &str, got: &str) {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(&path, got).unwrap();
    }
    let want = std::fs::read_to_string(&path)
        .expect("golden missing: run with UPDATE_GOLDENS=1 once, then review the diff");
    assert_eq!(got, want, "{name} drifted from the golden");
}

#[test]
fn transforms_match_their_goldens_and_are_deterministic() {
    let compile = || {
        let c = isolex::compile(&transforms());
        assert!(c.errors.is_empty(), "{:?}", c.errors);
        c.doc
    };
    let ir = serde_json::to_string_pretty(&compile()).unwrap();
    assert_eq!(ir, serde_json::to_string_pretty(&compile()).unwrap());
    golden("transforms.ir.golden.json", &format!("{ir}\n"));

    let palette = isolex::render::Palette::default();
    let svgs: String = compile()
        .icons
        .iter()
        .map(|icon| {
            format!(
                "<!-- {} -->\n{}",
                icon.name,
                isolex::render::render_icon(icon, &palette)
            )
        })
        .collect();
    golden("transforms.svg.golden", &svgs);
}

/// Every shape a golden draws, drawn through its group's matrix and cut
/// by its clip, lies inside its icon's viewBox: whatever a transform's
/// bounds, the viewBox is the drawing's true extent.
#[test]
fn every_golden_draws_inside_its_viewbox() {
    use isolex::affine::{shape_extent, Extent, IDENTITY};
    for files in [sample(), transforms()] {
        let c = isolex::compile(&files);
        assert!(c.errors.is_empty(), "{:?}", c.errors);
        for icon in &c.doc.icons {
            let frame = Extent::of_viewbox(icon.viewbox);
            for shape in &icon.shapes {
                if let Some(e) = shape_extent(shape, &IDENTITY).unwrap() {
                    assert!(
                        e.within(&frame),
                        "icon '{}' draws {e:?} outside its viewBox {:?}: {shape:?}",
                        icon.name,
                        icon.viewbox
                    );
                }
            }
        }
    }
}

/// SPEC §7: path data is formatted once, at compile time, and passes
/// through rendering unchanged, groups and transforms included.
#[test]
fn every_path_passes_through_render_byte_for_byte() {
    use isolex::ir::IrShape;
    fn paths<'a>(shapes: &'a [IrShape], out: &mut Vec<&'a str>) {
        for s in shapes {
            match s {
                IrShape::Path { d, .. } => out.push(d),
                IrShape::Group { shapes, .. } => paths(shapes, out),
                _ => {}
            }
        }
    }
    let palette = isolex::render::Palette::default();
    let mut seen = 0;
    for files in [sample(), transforms()] {
        let c = isolex::compile(&files);
        for icon in &c.doc.icons {
            let svg = isolex::render::render_icon(icon, &palette);
            let mut ds = Vec::new();
            paths(&icon.shapes, &mut ds);
            for d in ds {
                seen += 1;
                assert!(
                    svg.contains(&format!("d=\"{d}\"")),
                    "{}: {d}\n{svg}",
                    icon.name
                );
            }
        }
    }
    assert!(seen > 10, "the goldens draw paths: {seen}");
}

#[test]
fn a_transform_that_leaves_its_frame_is_an_error_unless_its_bounds_say() {
    let icon = |bounds: &str, default: &str| {
        format!(
            "icon \"t\" {{ category \"c\"; canvas 24 24{default}\n  part \"p\" {{\n    transform translate=\"10 0\"{bounds} {{\n      at 0 0 0 {{ rect 4 4 16 16 fill=\"ink\" }}\n    }}\n  }}\n}}\n"
        )
    };
    let (ok, stderr) = check_files("bounds-error", &[("t.ilx", &icon("", ""))]);
    assert!(!ok);
    assert!(
        stderr.contains("t.ilx:3:5: icon 't' draws this transform out to 14.00 4.00 to 30.00 20.00, outside its frame, 0.00 0.00 to 24.00 24.00; set bounds=\"grow\""),
        "{stderr}"
    );
    // the icon's default applies, and the transform's own wins over it
    let (ok, stderr) = check_files(
        "bounds-default",
        &[("t.ilx", &icon("", "\n  transform-bounds \"clip\""))],
    );
    assert!(ok, "{stderr}");
    let (ok, stderr) = check_files(
        "bounds-own",
        &[(
            "t.ilx",
            &icon(" bounds=\"error\"", "\n  transform-bounds \"clip\""),
        )],
    );
    assert!(!ok && stderr.contains("outside its frame"), "{stderr}");
    // inside its frame, a transform needs no bounds at all
    let (ok, stderr) = check_files(
        "bounds-inside",
        &[("t.ilx", "icon \"t\" { category \"c\"; canvas 24 24\n  part \"p\" { transform translate=\"2 0\" { at 0 0 0 { rect 4 4 16 16 } } }\n}\n")],
    );
    assert!(ok, "{stderr}");
}

#[test]
fn a_skew_of_ninety_degrees_is_an_error_where_it_is_written() {
    let (ok, stderr) = check_files(
        "skew-90",
        &[("t.ilx", "icon \"t\" { category \"c\"; canvas 24 24\n  part \"p\" {\n    transform skew-x=90 { at 0 0 0 { rect 4 4 16 16 } }\n  }\n}\n")],
    );
    assert!(!ok);
    assert!(
        stderr.contains("t.ilx:3:5: icon 't' transforms by a matrix that is not finite"),
        "{stderr}"
    );
}

#[test]
fn turns_take_quarter_turns_and_views_need_a_footprint() {
    let (ok, stderr) = check_files(
        "turn-45",
        &[("t.ilx", "icon \"t\" { category \"c\"; footprint 1 1 1\n  part \"p\" {\n    turn 45 { box 0 0 0 1 1 1 }\n  }\n}\n")],
    );
    assert!(!ok);
    assert!(
        stderr.contains("t.ilx:3:5: turn takes a quarter-turn, 0, 90, 180 or 270 degrees, got 45"),
        "{stderr}"
    );
    let (ok, stderr) = check_files(
        "view-canvas",
        &[("t.ilx", "icon \"t\" { category \"c\"; canvas 24 24; view 90\n  part \"p\" { at 0 0 0 { circle 4 4 2 } }\n}\n")],
    );
    assert!(!ok);
    assert!(
        stderr.contains("icon 't' has a view but no footprint"),
        "{stderr}"
    );
    // any multiple of 90, from a param, both ways round
    let (ok, stderr) = check_files(
        "turn-param",
        &[("t.ilx", "icon \"t\" { category \"c\"; footprint 1 1 1; param q -1; view \"q * 450\"\n  part \"p\" { turn \"q * -90\" origin=\"0.5 0.5\" { box 0 0 0 1 1 1 } }\n}\n")],
    );
    assert!(ok, "{stderr}");
}

#[test]
fn a_chain_of_transforms_stops_at_its_transform_chain() {
    let src = |limit: &str| {
        format!(
            "fragment \"f\" {{ part \"p\" {{ transform rotate=1 scale=1 {{ tile 0 0 0 1 1 }} }} }}\n\
             icon \"t\" {{ category \"c\"; footprint 1 1 1{limit}\n  part \"p\" {{\n    transform rotate=0 {{ insert \"f\" }}\n  }}\n}}\n"
        )
    };
    let (ok, stderr) = check_files("chain-2", &[("t.ilx", &src("; transform-chain 2"))]);
    assert!(!ok);
    assert!(
        stderr.contains("icon 't' draws shapes through 3 transform steps here past its transform-chain of 2 (steps, set by 't')"),
        "{stderr}"
    );
    let (ok, stderr) = check_files("chain-3", &[("t.ilx", &src("; transform-chain 3"))]);
    assert!(ok, "{stderr}");
}

#[test]
fn deep_transforms_never_overflow_the_stack() {
    // transforms nest as markers in a flat list: deep nesting costs
    // memory, never the call stack
    const N: usize = 50_000;
    let src = format!(
        "lexicon {{ transform-chain #null }}\nicon \"deep\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{\n{}tile 0 0 0 1 1\n{} }} }}\n",
        "transform translate=\"0 0\" {\n".repeat(N),
        "}\n".repeat(N)
    );
    let (ok, stderr, ir) = compile_src("deep-transforms", &src);
    assert!(ok, "{}", &stderr[..stderr.len().min(300)]);
    // translating by nothing is no transform: the tile, ungrouped
    assert_eq!(ir["icons"][0]["shapes"][0]["kind"], "polygon");
}

#[test]
fn groups_never_nest_and_badges_fold_into_their_transforms() {
    let c = isolex::compile(&transforms());
    let badged = c.doc.icons.iter().find(|i| i.name == "badged").unwrap();
    let groups: Vec<_> = badged
        .shapes
        .iter()
        .filter_map(|s| match s {
            isolex::ir::IrShape::Group { matrix, shapes, .. } => Some((matrix, shapes)),
            _ => None,
        })
        .collect();
    assert_eq!(groups.len(), 2, "the badge, and the badge turned");
    // the turned badge's matrix is the turn times the badge's
    assert_eq!(groups[1].0[0], -groups[0].0[0]);
    for icon in &c.doc.icons {
        for s in &icon.shapes {
            if let isolex::ir::IrShape::Group { shapes, .. } = s {
                assert!(
                    !shapes
                        .iter()
                        .any(|s| matches!(s, isolex::ir::IrShape::Group { .. })),
                    "icon '{}' nests a group",
                    icon.name
                );
            }
        }
    }
}
