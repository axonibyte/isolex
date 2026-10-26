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

/// A font in some of its styles, as a tool that converts fonts sees it:
/// its metrics and kerning, in its em grid, and each glyph's shapes there,
/// y up from the baseline, exactly as text draws them.
#[derive(Debug)]
pub struct FontOutline {
    pub name: String,
    /// Its `describe` lines, notices included.
    pub describe: Vec<String>,
    pub kind: ast::FontKind,
    pub em: f64,
    pub ascent: f64,
    pub descent: f64,
    /// Degrees, as skew-x leans.
    pub slant: f64,
    /// In name order.
    pub glyphs: Vec<GlyphOutline>,
    pub notdef: Option<GlyphOutline>,
    /// (left, right, space between), in name order.
    pub kerns: Vec<(String, String, f64)>,
}

/// One glyph: the text it draws (`.notdef` for notdef), its advance, and
/// its shapes, perhaps in groups.
#[derive(Debug)]
pub struct GlyphOutline {
    pub name: String,
    pub advance: f64,
    pub shapes: Vec<ir::IrShape>,
}

/// A font of a lexicon in the styles named, every glyph drawn: what
/// `isolex font export` writes a standard font from. The whole lexicon
/// is parsed and resolved, and its errors are this one's.
pub fn font_outline(
    files: &[(String, String)],
    font: &str,
    styles: &[String],
) -> Result<FontOutline, Vec<Diag>> {
    let outcome = parse_lexicon(files);
    let mut errors: Vec<Diag> = outcome.diags.clone();
    let resolved = resolve::resolve(&outcome.lexicon);
    errors.extend(resolved.diags.iter().map(|d| locate(files, d)));
    if !errors.is_empty() {
        return Err(errors);
    }
    if !resolved.fonts.contains_key(font) {
        // a request no node is at fault for
        return Err(vec![Diag {
            file: String::new(),
            line: 0,
            col: 0,
            message: format!(
                "the lexicon has no font '{font}'{}",
                resolve::suggest(font, resolved.fonts.keys().map(|k| k.as_str()))
            ),
        }]);
    }
    let f = expand::expand_font(&resolved, font, styles)
        .map_err(|ds| ds.iter().map(|d| locate(files, d)).collect::<Vec<_>>())?;
    let mut icons: Vec<expand::ExpandedIcon> = f.glyphs.iter().map(|(_, _, i)| i.clone()).collect();
    icons.extend(f.notdef.iter().map(|(_, i)| i.clone()));
    let (doc, pdiags) = project::project(&icons, &project::Vocabulary::default());
    if !pdiags.is_empty() {
        return Err(pdiags.iter().map(|d| locate(files, d)).collect());
    }
    let mut drawn = doc.icons.into_iter();
    let glyphs = f
        .glyphs
        .iter()
        .map(|(name, advance, _)| GlyphOutline {
            name: name.clone(),
            advance: *advance,
            shapes: drawn.next().expect("a glyph each").shapes,
        })
        .collect();
    let notdef = f.notdef.as_ref().map(|(advance, _)| GlyphOutline {
        name: ".notdef".to_string(),
        advance: *advance,
        shapes: drawn.next().expect("and notdef").shapes,
    });
    Ok(FontOutline {
        name: f.name,
        describe: f.describe,
        kind: f.kind,
        em: f.em,
        ascent: f.ascent,
        descent: f.descent,
        slant: f.slant,
        glyphs,
        notdef,
        kerns: f.kerns,
    })
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
