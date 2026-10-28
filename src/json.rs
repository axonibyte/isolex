//! The compiler as one function from JSON to JSON, for hosts that embed
//! it rather than run the CLI: the `wasm/` crate exports exactly this.
//!
//! In: `{"files": [{"name": "a.ilx", "text": "…"}, …]}`, compiled in the
//! order given; with `"icon": "name"`, that icon alone, and with
//! `"params": {"u": 2, "label": "U12"}` too, those params set on it, a
//! number for a number param and a string for a text one
//! ([`crate::compile_one`]). Out:
//!
//! ```json
//! {"isolex": "0.2.0", "ok": true, "errors": [], "warnings": [],
//!  "ir": { …the IR, as `isolex compile` writes it… }}
//! ```
//!
//! Each error and warning is `{"file", "line", "col", "message"}`. `ir`
//! is `null` whenever `ok` is false, as the CLI writes nothing then.
//! Input that is not that shape, or asks for an icon the lexicon does not
//! emit or a param it does not have, gives `ok: false` and an
//! `input_error` instead of diagnostics.
//!
//! Many icons from one parse ([`crate::compile_many`]): `"requests":
//! [{"icon": "name", "params": {…}}, …]` in place of `"icon"` and
//! `"params"`. Out, the lexicon's own outcome, and a result for each
//! request, in order, shaped as one icon's output is but for `isolex`:
//!
//! ```json
//! {"isolex": "0.5.0", "ok": true, "errors": [], "warnings": [],
//!  "results": [{"ok": true, "errors": [], "warnings": [], "ir": { … }},
//!              {"ok": false, "input_error": "…", "errors": [], "warnings": [], "ir": null}]}
//! ```
//!
//! `ok` and `errors` at the top are the lexicon's parsing and resolving,
//! and `results` is `null` when it has errors; `warnings` are the
//! lexicon's, and each result's are empty.
//!
//! A lexicon with a `palette` block adds it at the top of either output,
//! by token, whether it compiled or not: `"palette": {"ink": {"light":
//! "#1f2933", "dark": "#e4e7eb"}, …}`. Without one, there is no
//! `palette`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ir::IrDocument;
use crate::parser::Diag;
use crate::{Colors, HostValue};

#[derive(Deserialize)]
struct Input {
    files: Vec<File>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    params: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    requests: Option<Vec<Request>>,
}

/// One icon of many to compile, with its params.
#[derive(Deserialize)]
struct Request {
    icon: String,
    #[serde(default)]
    params: BTreeMap<String, Value>,
}

/// A param's value: a number or a string, nothing else.
#[derive(Deserialize)]
#[serde(untagged)]
enum Value {
    Num(f64),
    Text(String),
}

#[derive(Deserialize)]
struct File {
    name: String,
    text: String,
}

#[derive(Serialize)]
struct Output<'a> {
    isolex: &'static str,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_error: Option<String>,
    errors: &'a [Diag],
    warnings: &'a [Diag],
    ir: Option<&'a IrDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette: Option<&'a BTreeMap<String, Colors>>,
}

/// The output for many icons: the lexicon's, and each request's.
#[derive(Serialize)]
struct ManyOutput<'a> {
    isolex: &'static str,
    ok: bool,
    errors: &'a [Diag],
    warnings: &'a [Diag],
    results: Option<Vec<OneOutput<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette: Option<&'a BTreeMap<String, Colors>>,
}

/// One request's result: an icon's output but for `isolex`.
#[derive(Serialize)]
struct OneOutput<'a> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_error: Option<&'a str>,
    errors: &'a [Diag],
    warnings: &'a [Diag],
    ir: Option<&'a IrDocument>,
}

/// Compiles the lexicon `input` describes and reports the outcome; never
/// panics on bad input.
pub fn compile_json(input: &[u8]) -> String {
    let refuse = |why: String| {
        to_string(&Output {
            isolex: env!("CARGO_PKG_VERSION"),
            ok: false,
            input_error: Some(why),
            errors: &[],
            warnings: &[],
            ir: None,
            palette: None,
        })
    };
    let parsed: Input = match serde_json::from_slice(input) {
        Ok(i) => i,
        Err(e) => {
            return refuse(format!(
                "expected {{\"files\": [{{\"name\", \"text\"}}, …], \"icon\"?, \"params\"?, \"requests\"?}}: {e}"
            ))
        }
    };
    let files: Vec<(String, String)> = parsed.files.into_iter().map(|f| (f.name, f.text)).collect();
    if let Some(requests) = parsed.requests {
        if parsed.icon.is_some() || parsed.params.is_some() {
            return refuse(
                "\"requests\" name each icon and its params; leave out \"icon\" and \"params\""
                    .to_string(),
            );
        }
        return many(&files, requests);
    }
    let c = match (parsed.icon, parsed.params) {
        (None, None) => crate::compile(&files),
        (None, Some(_)) => {
            return refuse("params are set on one icon; name it with \"icon\"".to_string())
        }
        (Some(icon), params) => {
            let params = host_values(params.unwrap_or_default());
            match crate::compile_one(&files, &icon, &params) {
                Ok(c) => c,
                Err(why) => return refuse(why),
            }
        }
    };
    let ok = c.errors.is_empty();
    to_string(&Output {
        isolex: env!("CARGO_PKG_VERSION"),
        ok,
        input_error: None,
        errors: &c.errors,
        warnings: &c.warnings,
        ir: ok.then_some(&c.doc),
        palette: c.palette.as_ref(),
    })
}

/// Compiles many icons from one parse, and reports each.
fn many(files: &[(String, String)], requests: Vec<Request>) -> String {
    let requests: Vec<crate::Only> = requests
        .into_iter()
        .map(|r| crate::Only {
            icon: r.icon,
            params: host_values(r.params),
        })
        .collect();
    let (lexicon, results) = crate::compile_many(files, &requests);
    let ok = lexicon.errors.is_empty();
    let results = ok.then(|| {
        results
            .iter()
            .map(|r| match r {
                Ok(c) => OneOutput {
                    ok: c.errors.is_empty(),
                    input_error: None,
                    errors: &c.errors,
                    warnings: &c.warnings,
                    ir: c.errors.is_empty().then_some(&c.doc),
                },
                Err(why) => OneOutput {
                    ok: false,
                    input_error: Some(why),
                    errors: &[],
                    warnings: &[],
                    ir: None,
                },
            })
            .collect()
    });
    serde_json::to_string(&ManyOutput {
        isolex: env!("CARGO_PKG_VERSION"),
        ok,
        errors: &lexicon.errors,
        warnings: &lexicon.warnings,
        results,
        palette: lexicon.palette.as_ref(),
    })
    .expect("the output serializes")
}

/// A request's params, as the compiler takes them.
fn host_values(params: BTreeMap<String, Value>) -> BTreeMap<String, HostValue> {
    params
        .into_iter()
        .map(|(k, v)| {
            let v = match v {
                Value::Num(n) => HostValue::Num(n),
                Value::Text(t) => HostValue::Text(t),
            };
            (k, v)
        })
        .collect()
}

fn to_string(out: &Output) -> String {
    serde_json::to_string(out).expect("the output serializes")
}
