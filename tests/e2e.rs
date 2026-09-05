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
