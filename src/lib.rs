//! isolex — reference compiler library for the Isolex icon-definition
//! language. See SPEC.md for the normative language definition.

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

use ast::Span;
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
