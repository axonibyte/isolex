//! The compiler as one function from JSON to JSON, for hosts that embed
//! it rather than run the CLI: the `wasm/` crate exports exactly this.
//!
//! In: `{"files": [{"name": "a.ilx", "text": "…"}, …]}`, compiled in the
//! order given. Out:
//!
//! ```json
//! {"isolex": "0.2.0", "ok": true, "errors": [], "warnings": [],
//!  "ir": { …the IR, as `isolex compile` writes it… }}
//! ```
//!
//! Each error and warning is `{"file", "line", "col", "message"}`. `ir`
//! is `null` whenever `ok` is false, as the CLI writes nothing then.
//! Input that is not that shape gives `ok: false` and an `input_error`
//! instead of diagnostics.

use serde::{Deserialize, Serialize};

use crate::ir::IrDocument;
use crate::parser::Diag;

#[derive(Deserialize)]
struct Input {
    files: Vec<File>,
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
    let parsed: Input = match serde_json::from_slice(input) {
        Ok(i) => i,
        Err(e) => {
            return to_string(&Output {
                isolex: env!("CARGO_PKG_VERSION"),
                ok: false,
                input_error: Some(format!(
                    "expected {{\"files\": [{{\"name\", \"text\"}}, …]}}: {e}"
                )),
                errors: &[],
                warnings: &[],
                ir: None,
            })
        }
    };
    let files: Vec<(String, String)> = parsed.files.into_iter().map(|f| (f.name, f.text)).collect();
    let c = crate::compile(&files);
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
