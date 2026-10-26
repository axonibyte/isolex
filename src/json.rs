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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ir::IrDocument;
use crate::parser::Diag;
use crate::HostValue;

#[derive(Deserialize)]
struct Input {
    files: Vec<File>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    params: Option<BTreeMap<String, Value>>,
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
        })
    };
    let parsed: Input = match serde_json::from_slice(input) {
        Ok(i) => i,
        Err(e) => {
            return refuse(format!(
                "expected {{\"files\": [{{\"name\", \"text\"}}, …], \"icon\"?, \"params\"?}}: {e}"
            ))
        }
    };
    let files: Vec<(String, String)> = parsed.files.into_iter().map(|f| (f.name, f.text)).collect();
    let c = match (parsed.icon, parsed.params) {
        (None, None) => crate::compile(&files),
        (None, Some(_)) => {
            return refuse("params are set on one icon; name it with \"icon\"".to_string())
        }
        (Some(icon), params) => {
            let params = params
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| {
                    let v = match v {
                        Value::Num(n) => HostValue::Num(n),
                        Value::Text(t) => HostValue::Text(t),
                    };
                    (k, v)
                })
                .collect();
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
    })
}

fn to_string(out: &Output) -> String {
    serde_json::to_string(out).expect("the output serializes")
}
