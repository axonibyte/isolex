//! The compiler as one JSON-to-JSON function (`isolex::json`), which the
//! WebAssembly module exports as it is: same IR as the CLI, diagnostics
//! as data, and never a panic on input it cannot read.

use isolex::json::compile_json;
use serde_json::{json, Value};

fn run(input: &Value) -> Value {
    let out = compile_json(input.to_string().as_bytes());
    serde_json::from_str(&out).expect("the output is JSON")
}

fn files(files: &[(&str, &str)]) -> Value {
    json!({ "files": files.iter().map(|(n, t)| json!({"name": n, "text": t})).collect::<Vec<_>>() })
}

#[test]
fn the_sample_compiles_to_the_cli_golden() {
    let out = run(&files(&[(
        "sample.ilx",
        include_str!("fixtures/sample.ilx"),
    )]));
    assert_eq!(out["isolex"], env!("CARGO_PKG_VERSION"));
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(out["errors"], json!([]));
    // the sample grants accent and reserves nothing, as the CLI warns
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{out}");
    assert_eq!(
        (
            &warnings[0]["file"],
            &warnings[0]["line"],
            &warnings[0]["col"]
        ),
        (&json!("sample.ilx"), &json!(82), &json!(3))
    );
    assert!(out.get("input_error").is_none());
    let golden: Value =
        serde_json::from_str(include_str!("fixtures/sample.ir.golden.json")).unwrap();
    assert_eq!(
        out["ir"], golden,
        "the IR differs from what `isolex compile` writes"
    );
}

#[test]
fn errors_are_located_data_and_there_is_no_ir() {
    let out = run(&files(&[
        ("a.ilx", "icon \"one\" { category \"c\"; canvas 9 9 }\n"),
        ("b.ilx", "icon \"two\" {\n  patr \"p\" { }\n}\n"),
    ]));
    assert_eq!(out["ok"], false);
    assert_eq!(out["ir"], Value::Null);
    let errors = out["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 2, "{out}");
    assert_eq!(errors[0]["file"], "b.ilx");
    assert_eq!(
        (&errors[0]["line"], &errors[0]["col"]),
        (&json!(2), &json!(3))
    );
    let message = errors[0]["message"].as_str().unwrap();
    assert!(message.contains("unknown node 'patr'"), "{message}");
    assert!(message.contains("did you mean 'part'?"), "{message}");
    // the other: "two" never got its category
    assert_eq!(errors[1]["file"], "b.ilx");
}

#[test]
fn warnings_come_apart_from_errors_and_keep_the_ir() {
    let src = "axis \"state\" { variant \"down\" { grant \"accent\" reason=\"danger\" } }\nicon \"i\" { category \"c\"; canvas 9 9; axes \"state\" }\n";
    let out = run(&files(&[("w.ilx", src)]));
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(out["errors"], json!([]));
    assert_eq!(
        out["warnings"],
        json!([{
            "file": "w.ilx",
            "line": 1,
            "col": 33,
            "message": "variant 'down' of axis 'state' grants 'accent', but the lexicon reserves \
                        no tokens, so the grant has no effect; reserve it with `reserved \"accent\"` \
                        in the lexicon block, or remove the grant"
        }])
    );
    let names: Vec<&str> = out["ir"]["icons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["i", "i-down"]);
}

#[test]
fn files_compile_in_the_order_given() {
    let a = ("a.ilx", "icon \"one\" { category \"c\"; canvas 9 9 }\n");
    let b = ("b.ilx", "icon \"two\" { category \"c\"; canvas 9 9 }\n");
    let names = |out: Value| -> Vec<String> {
        out["ir"]["icons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names(run(&files(&[a, b]))), ["one", "two"]);
    assert_eq!(names(run(&files(&[b, a]))), ["two", "one"]);
}

#[test]
fn input_it_cannot_read_is_an_input_error_not_a_panic() {
    for input in [
        &b"not json"[..],
        &b"{\"files\": [{\"name\": \"a.ilx\"}]}"[..],
        &b"{\"files\": \"a.ilx\"}"[..],
        &[0xff, 0xfe, 0x00][..],
        &b""[..],
    ] {
        let out: Value = serde_json::from_str(&compile_json(input)).unwrap();
        assert_eq!(out["ok"], false, "{out}");
        assert_eq!(out["ir"], Value::Null);
        assert_eq!(out["errors"], json!([]));
        let e = out["input_error"].as_str().unwrap();
        assert!(e.starts_with("expected {\"files\""), "{e}");
    }
}

#[test]
fn an_empty_lexicon_is_fine_and_empty() {
    let out = run(&json!({ "files": [] }));
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(out["ir"]["icons"], json!([]));
}

#[test]
fn one_icon_with_params_compiles_alone() {
    let mut input = files(&[("text.ilx", include_str!("../tests/fixtures/text.ilx"))]);
    input["icon"] = json!("port-label");
    input["params"] = json!({"ports": 24});
    let out = run(&input);
    assert_eq!(out["ok"], true, "{out}");
    let icons = out["ir"]["icons"].as_array().unwrap();
    assert_eq!(icons.len(), 1);
    assert_eq!(icons[0]["name"], "port-label");

    // what it cannot answer is the request's fault, not the lexicon's
    input["params"] = json!({"ports": "24"});
    let out = run(&input);
    assert_eq!(out["ok"], false);
    assert_eq!(
        out["input_error"],
        "param 'ports' of icon 'port-label' is a number; give it a number"
    );
    assert_eq!(out["ir"], Value::Null);
    input["params"] = json!({"ports": [1]});
    let out = run(&input);
    assert!(
        out["input_error"]
            .as_str()
            .unwrap()
            .starts_with("expected {\"files\""),
        "{out}"
    );
    input["icon"] = json!("nope");
    input["params"] = json!({});
    assert_eq!(
        run(&input)["input_error"],
        "the lexicon emits no icon 'nope'"
    );
    let mut input = files(&[("text.ilx", include_str!("../tests/fixtures/text.ilx"))]);
    input["params"] = json!({"ports": 24});
    assert_eq!(
        run(&input)["input_error"],
        "params are set on one icon; name it with \"icon\""
    );
}

#[test]
fn many_icons_compile_from_one_input_each_as_one_would() {
    let text = include_str!("../tests/fixtures/text.ilx");
    let mut input = files(&[("text.ilx", text)]);
    input["requests"] = json!([
        {"icon": "port-label", "params": {"ports": 24}},
        {"icon": "tagged-a", "params": {"tag": "Drill 3"}},
        {"icon": "nope"},
        {"icon": "port-label", "params": {"ports": "24"}},
        {"icon": "styles"},
    ]);
    let out = run(&input);
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(out["errors"], json!([]));
    let results = out["results"].as_array().unwrap();
    assert_eq!(results.len(), 5);
    // each is what asking for it alone gives, but for "isolex"
    for (i, r) in input["requests"].as_array().unwrap().iter().enumerate() {
        let mut one = files(&[("text.ilx", text)]);
        one["icon"] = r["icon"].clone();
        one["params"] = r.get("params").cloned().unwrap_or(json!({}));
        let mut want = run(&one);
        want.as_object_mut().unwrap().remove("isolex");
        assert_eq!(results[i], want, "request {i}");
    }
    assert_eq!(
        results[2]["input_error"],
        "the lexicon emits no icon 'nope'"
    );
    assert_eq!(results[2]["ir"], Value::Null);

    // the lexicon's own errors: no results
    let mut input = files(&[("b.ilx", "icon \"two\" {\n  patr \"p\" { }\n}\n")]);
    input["requests"] = json!([{"icon": "two"}]);
    let out = run(&input);
    assert_eq!(out["ok"], false);
    assert_eq!(out["results"], Value::Null);
    assert_eq!(out["errors"][0]["file"], "b.ilx");
    assert!(out.get("input_error").is_none(), "{out}");

    // none at all is fine
    let mut input = files(&[("text.ilx", text)]);
    input["requests"] = json!([]);
    assert_eq!(run(&input)["results"], json!([]));

    // requests name their own icons
    input["icon"] = json!("styles");
    assert_eq!(
        run(&input)["input_error"],
        "\"requests\" name each icon and its params; leave out \"icon\" and \"params\""
    );
}

#[test]
fn the_palette_block_is_returned_beside_the_ir() {
    let lexicon = include_str!("fixtures/palette.ilx");
    let out = run(&files(&[("palette.ilx", lexicon)]));
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(
        out["palette"]["accent"],
        json!({"light": "#c62828", "dark": "#ef9a9a"})
    );
    assert_eq!(out["palette"].as_object().unwrap().len(), 6);
    // the IR does not change
    assert!(out["ir"].get("palette").is_none());

    // one icon, and many, give it too
    let mut one = files(&[("palette.ilx", lexicon)]);
    one["icon"] = json!("crate");
    assert_eq!(run(&one)["palette"], out["palette"]);
    let mut many = files(&[("palette.ilx", lexicon)]);
    many["requests"] = json!([{"icon": "crate"}]);
    let many = run(&many);
    assert_eq!(many["palette"], out["palette"]);
    assert!(many["results"][0].get("palette").is_none());

    // and so does a lexicon that fails, so a host can still show its colors
    let broken = format!("{lexicon}\nicon \"x\" {{\n  patr \"p\" {{ }}\n}}\n");
    let out = run(&files(&[("palette.ilx", &broken)]));
    assert_eq!(out["ok"], false);
    assert_eq!(out["palette"]["ink"]["dark"], "#f0f4f8");

    // without a block, there is no palette
    let out = run(&files(&[(
        "sample.ilx",
        include_str!("fixtures/sample.ilx"),
    )]));
    assert!(out.get("palette").is_none(), "{out}");
}
