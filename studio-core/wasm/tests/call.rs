//! The module's calls, natively: what the interface outside the app is
//! told, call by call.

use isolex_studio_wasm::call;
use serde_json::{json, Value};

fn send(v: Value) -> Value {
    serde_json::from_str(&call(&v.to_string())).unwrap()
}

fn kinds(reply: &Value) -> Vec<String> {
    reply["notices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["kind"].as_str().unwrap().to_string())
        .collect()
}

const SAMPLE: &str = include_str!("../../../tests/fixtures/sample.ilx");

#[test]
fn a_library_opened_edited_undone_and_saved() {
    let opened = send(json!({ "call": "open", "files": [["sample.ilx", SAMPLE]] }));
    let k = kinds(&opened);
    assert_eq!(k[..3], ["graph", "library", "compiled"]);
    assert!(k.iter().filter(|k| *k == "icon").count() > 5);

    // a param previewed: the icon drawn with it, and nothing told
    let preview = send(json!({ "call": "preview", "icon": "switch-l3", "params": { "ports": 4 } }));
    let ports = preview["result"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["kind"] == "facerect")
        .count();
    assert_eq!(ports, 4);
    assert!(kinds(&preview).is_empty());
    let none = send(json!({ "call": "preview", "icon": "nope", "params": {} }));
    assert!(none["result"].is_null());

    let source = send(json!({ "call": "source", "file": "sample.ilx" }));
    assert_eq!(source["result"]["text"], SAMPLE);
    let outline = send(json!({ "call": "outline", "definition": "switch-poe" }));
    assert_eq!(outline["result"]["parts"].as_array().unwrap().len(), 3);

    // a box added to switch-l3, compiled by the time the call returns
    let node = &outline["result"]["node"];
    let l3 =
        send(json!({ "call": "outline", "definition": "switch-l3" }))["result"]["node"].clone();
    assert_ne!(node, &l3);
    let asked = send(json!({
        "call": "ask",
        "label": "Add part",
        "ask": { "kind": "insert", "parent": l3, "text": "part \"x\" {\n  box 0 0 0.28 1 1 0.5\n}", "before": null }
    }));
    assert_eq!(asked["result"], json!({ "kind": "applied", "revision": 1 }));
    assert!(kinds(&asked).contains(&"icon".to_string()));
    let text = send(json!({ "call": "source", "file": "sample.ilx" }))["result"]["text"].clone();
    assert!(text
        .as_str()
        .unwrap()
        .contains("    box 0 0 0.28 1 1 0.5\n"));

    assert_eq!(send(json!({ "call": "save" }))["error"], Value::Null);
    let undone = send(json!({ "call": "undo" }));
    assert_eq!(undone["result"], "Add part");
    let library = undone["notices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["kind"] == "library")
        .unwrap();
    assert_eq!(library["modified"], true, "undone past the save");

    // the lexicon's blocks; a library op; a change "on disk", loaded
    let lex = send(json!({ "call": "lexicon" }));
    assert_eq!(lex["result"]["lexicon"]["rows"][0]["name"], "tokens");
    let op = send(json!({ "call": "library_op", "op": { "op": "new-file", "path": "more.ilx" } }));
    assert_eq!(op["result"]["kind"], "applied");
    let undone = send(json!({ "call": "undo" }));
    assert_eq!(undone["result"], "New file more.ilx");
    // nothing unsaved, so it loads
    send(json!({ "call": "save" }));
    let disk = send(
        json!({ "call": "outside", "changes": [["sample.ilx", SAMPLE.replace("param ports 12", "param ports 16")]] }),
    );
    assert!(
        kinds(&disk).contains(&"reloaded".to_string()),
        "{:?}",
        kinds(&disk)
    );
    send(json!({ "call": "undo" }));
}

#[test]
fn what_it_refuses() {
    assert!(send(json!("nope"))["error"].is_string());
    assert!(call("{").contains("not JSON"));
    assert_eq!(send(json!({ "call": "undo" }))["error"], "no library open");
    send(json!({ "call": "open", "files": [["a.ilx", ""]] }));
    assert_eq!(send(json!({ "call": "nope" }))["error"], "no call nope");
    assert!(
        send(json!({ "call": "ask", "ask": { "kind": "nope" } }))["error"]
            .as_str()
            .unwrap()
            .starts_with("ask: ")
    );
    assert_eq!(
        send(json!({ "call": "about" }))["result"]["studio"],
        "0.1.0"
    );
}

#[test]
fn the_library_exported_as_svg_ir_and_a_zip() {
    send(json!({ "call": "open", "files": [["sample.ilx", SAMPLE]] }));
    let svgs = send(json!({ "call": "export_svgs", "icons": ["switch-l3"], "ground": "dark" }));
    let svgs = svgs["result"].as_array().unwrap();
    assert_eq!(svgs.len(), 1);
    assert_eq!(svgs[0]["name"], "switch-l3");
    assert!(svgs[0]["svg"].as_str().unwrap().starts_with("<svg"));

    let ir = send(json!({ "call": "export_ir", "icons": null }));
    let doc: Value = serde_json::from_str(ir["result"].as_str().unwrap()).unwrap();
    assert!(doc["icons"].as_array().unwrap().len() > 5);

    let zip = send(json!({ "call": "export_zip" }));
    let bytes: Vec<u8> = serde_json::from_value(zip["result"].clone()).unwrap();
    let files = isolex_studio_core::export::unzip(&bytes).unwrap();
    assert_eq!(files["sample.ilx"], SAMPLE);

    let refused = send(json!({ "call": "export_svgs", "icons": ["nope"], "ground": "light" }));
    assert!(refused["error"].as_str().unwrap().contains("nope"));
}
