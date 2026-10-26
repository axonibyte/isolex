//! isolex — reference compiler library for the Isolex icon-definition
//! language. See SPEC.md for the normative language definition.

pub mod affine;
pub mod ast;
pub mod expand;
pub mod expr;
pub mod ir;
pub mod json;
pub mod kdl;
pub mod parser;
pub mod project;
pub mod render;
pub mod resolve;
pub mod text;

use std::collections::BTreeMap;

use ast::Span;
pub use expand::HostValue;
use ir::IrDocument;
use parser::{parse_lexicon, Diag};
use resolve::SpanDiag;

/// What compiling a lexicon came to: the IR, the errors that fail it and
/// the warnings that don't (SPEC §8).
pub struct Compilation {
    /// The IR as far as projection got; only to be used when `errors` is
    /// empty.
    pub doc: IrDocument,
    /// Definitions parsed, emitted or not.
    pub definitions: usize,
    /// Parse, resolve, expand and projection errors, in that order.
    pub errors: Vec<Diag>,
    pub warnings: Vec<Diag>,
}

/// Compiles a lexicon given as (file name, source text) pairs, in the
/// order given (SPEC §1). Nothing here touches a file system, so it runs
/// wherever the library builds, WebAssembly included.
pub fn compile(files: &[(String, String)]) -> Compilation {
    let outcome = parse_lexicon(files);
    let mut errors: Vec<Diag> = outcome.diags.clone();
    let resolved = resolve::resolve(&outcome.lexicon);
    errors.extend(resolved.diags.iter().map(|d| locate(files, d)));
    let (expanded, ediags) = expand::expand(&resolved);
    errors.extend(ediags.iter().map(|d| locate(files, d)));
    let (doc, pdiags) = project::project(
        &expanded,
        &project::Vocabulary::of(outcome.lexicon.config.as_ref()),
    );
    errors.extend(pdiags.iter().map(|d| locate(files, d)));
    let warnings = resolved.warnings.iter().map(|w| locate(files, w)).collect();
    Compilation {
        doc,
        definitions: outcome.lexicon.defs.len(),
        errors,
        warnings,
    }
}

/// Compiles one icon of a lexicon, by the name the lexicon emits it under
/// (a variant's included), with params a host sets on it (SPEC §10): a
/// value known only when one thing is drawn, a name or a count, in the
/// slot of the param it sets, as a child's value would be. The whole
/// lexicon is parsed and resolved, but only this icon is expanded and
/// projected, and those it badges, which it draws; the IR holds it alone.
/// The same inputs always give the same IR, so a host can keep it by
/// them.
///
/// `Err` is a request the lexicon cannot answer: an icon it does not
/// emit, or a param the icon does not have or of another type. A lexicon
/// with errors of its own compiles to them, as [`compile`] does.
pub fn compile_one(
    files: &[(String, String)],
    icon: &str,
    params: &BTreeMap<String, HostValue>,
) -> Result<Compilation, String> {
    let outcome = parse_lexicon(files);
    let mut errors: Vec<Diag> = outcome.diags.clone();
    let resolved = resolve::resolve(&outcome.lexicon);
    errors.extend(resolved.diags.iter().map(|d| locate(files, d)));
    let warnings = resolved.warnings.iter().map(|w| locate(files, w)).collect();
    let mut c = Compilation {
        doc: IrDocument {
            ir_version: ir::IR_VERSION,
            icons: Vec::new(),
        },
        definitions: outcome.lexicon.defs.len(),
        errors,
        warnings,
    };
    if !c.errors.is_empty() {
        return Ok(c);
    }
    let only = expand::Only {
        icon: icon.to_string(),
        params: params.clone(),
    };
    let (expanded, ediags) = expand::expand_only(&resolved, &only)?;
    c.errors.extend(ediags.iter().map(|d| locate(files, d)));
    let (mut doc, pdiags) = project::project(
        &expanded,
        &project::Vocabulary::of(outcome.lexicon.config.as_ref()),
    );
    c.errors.extend(pdiags.iter().map(|d| locate(files, d)));
    doc.icons.retain(|i| i.name == icon);
    c.doc = doc;
    Ok(c)
}

/// Turns a byte span into the file name, line and column a person reads.
pub fn locate(files: &[(String, String)], d: &SpanDiag) -> Diag {
    let Span { file, start, .. } = d.span;
    let (name, text) = files
        .get(file)
        .map(|(n, t)| (n.clone(), t.as_str()))
        .unwrap_or_else(|| (String::from("<unknown>"), ""));
    let clamped = start.min(text.len());
    let before = &text[..clamped];
    let line = before.matches('\n').count() + 1;
    let col = before.chars().rev().take_while(|&c| c != '\n').count() + 1;
    Diag {
        file: name,
        line,
        col,
        message: d.message.clone(),
    }
}
