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
    assert_eq!(group["scale"], 0.5);
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
