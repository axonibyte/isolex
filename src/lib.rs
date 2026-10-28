//! isolex — reference compiler library for the Isolex icon-definition
//! language. See SPEC.md for the normative language definition.

pub mod affine;
pub mod ast;
pub mod expand;
pub mod expr;
pub mod graph;
pub mod ir;
pub mod json;
pub mod kdl;
pub mod library;
pub mod parser;
pub mod project;
pub mod render;
pub mod resolve;
pub mod text;

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

use ast::Span;
pub use expand::{HostValue, Only};
use ir::IrDocument;
use parser::{parse_lexicon, Diag};
use resolve::SpanDiag;

/// What compiling a lexicon came to: the IR, the errors that fail it and
/// the warnings that don't (SPEC §8).
#[derive(Debug, Clone)]
pub struct Compilation {
    /// The IR as far as projection got; only to be used when `errors` is
    /// empty.
    pub doc: IrDocument,
    /// Definitions parsed, emitted or not.
    pub definitions: usize,
    /// Parse, resolve, expand and projection errors, in that order.
    pub errors: Vec<Diag>,
    pub warnings: Vec<Diag>,
    /// The lexicon's `palette` block, by token, when it has one (SPEC §2).
    pub palette: Option<BTreeMap<String, Colors>>,
    /// Where each shape came from, for each icon of `doc` in order: one
    /// entry per shape, a group's before its own, as the IR lists them.
    /// Outside the IR, which it never changes.
    pub provenance: Vec<Vec<ShapeSource>>,
}

pub use graph::graph;
pub use project::{Map3, ShapeSource};

/// Each source's definition: the one its node is written in.
fn written_in(lexicon: &ast::Lexicon, provenance: &mut [Vec<ShapeSource>]) {
    for source in provenance.iter_mut().flatten() {
        let at = source.span;
        source.definition = lexicon
            .defs
            .iter()
            .find(|d| {
                d.span.file == at.file
                    && d.span.start <= at.start
                    && at.start + at.len <= d.span.start + d.span.len
            })
            .map(|d| d.name.clone())
            .unwrap_or_default();
    }
}

/// A token's colors in the `palette` block: on a light ground and on a
/// dark one.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Colors {
    pub light: String,
    pub dark: String,
}

fn palette_of(lexicon: &ast::Lexicon) -> Option<BTreeMap<String, Colors>> {
    lexicon.palette.as_ref().map(|p| {
        p.colors
            .iter()
            .map(|c| {
                let colors = Colors {
                    light: c.light.clone(),
                    dark: c.dark.clone(),
                };
                (c.token.clone(), colors)
            })
            .collect()
    })
}

/// A token an emitted icon paints that the palette block gives no color, a
/// warning at the block; with no block, none (SPEC §2).
fn uncolored(lexicon: &ast::Lexicon, doc: &IrDocument) -> Vec<SpanDiag> {
    let Some(block) = &lexicon.palette else {
        return Vec::new();
    };
    let colored = render::Palette::of_tokens(block.colors.iter().map(|c| (&c.token, &c.light)));
    render::missing_colors(&doc.icons, &colored)
        .into_iter()
        .map(|(token, icons)| {
            let mut by: Vec<&str> = icons.iter().map(|i| i.as_str()).take(3).collect();
            if icons.len() > 3 {
                by.push("…");
            }
            SpanDiag {
                span: block.span,
                message: format!(
                    "'{token}' is painted (by {}) but the palette gives it no color",
                    by.join(", ")
                ),
            }
        })
        .collect()
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
    let (doc, mut provenance, pdiags) = project::project_traced(
        &expanded,
        &project::Vocabulary::of(outcome.lexicon.config.as_ref()),
    );
    written_in(&outcome.lexicon, &mut provenance);
    errors.extend(pdiags.iter().map(|d| locate(files, d)));
    let warnings = resolved
        .warnings
        .iter()
        .chain(uncolored(&outcome.lexicon, &doc).iter())
        .map(|w| locate(files, w))
        .collect();
    Compilation {
        doc,
        definitions: outcome.lexicon.defs.len(),
        errors,
        warnings,
        palette: palette_of(&outcome.lexicon),
        provenance,
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
    let only = expand::Only {
        icon: icon.to_string(),
        params: params.clone(),
    };
    Prepared::new(files)
        .icon(&only, &AtomicBool::new(false))
        .map_err(|refused| refused.to_string())
}

/// Compiles several icons of a lexicon, each with params a host sets on
/// it, from one parse: for each request, what [`compile_one`] gives, but
/// the lexicon is parsed and resolved once for all of them, and a font
/// text is set in is built once for all, which are most of the work. A
/// host drawing many things at once (a list, a sheet of labels) asks for
/// them together.
///
/// The first part is the lexicon's own outcome: its definitions, its parse
/// and resolve errors and its warnings, and no icons. When it has errors,
/// nothing is expanded and there are no results; otherwise there is one
/// for each request, in order, holding that request's expansion and
/// projection errors and its IR, or `Err` for a request the lexicon cannot
/// answer, as [`compile_one`] refuses it.
pub fn compile_many(
    files: &[(String, String)],
    requests: &[expand::Only],
) -> (Compilation, Vec<Result<Compilation, String>>) {
    let prepared = Prepared::new(files);
    let results = prepared
        .icons(requests, &AtomicBool::new(false))
        .into_iter()
        .map(|r| r.map_err(|refused| refused.to_string()))
        .collect();
    (prepared.outcome, results)
}

/// A lexicon parsed and resolved once, to compile icons of as often as
/// asked, from as many threads at once as want to: what an editor keeps
/// while nothing changes, and throws away when something does. Each
/// compile can be cancelled, from another thread, once it is no longer
/// wanted.
pub struct Prepared {
    files: Vec<(String, String)>,
    lexicon: ast::Lexicon,
    resolved: resolve::Resolved,
    vocabulary: project::Vocabulary,
    outcome: Compilation,
}

pub use expand::Refused;

impl Prepared {
    /// Parses and resolves `files`, in the order given (SPEC §1).
    pub fn new(files: &[(String, String)]) -> Prepared {
        let outcome = parse_lexicon(files);
        let mut errors: Vec<Diag> = outcome.diags.clone();
        let resolved = resolve::resolve(&outcome.lexicon);
        errors.extend(resolved.diags.iter().map(|d| locate(files, d)));
        let warnings = resolved.warnings.iter().map(|w| locate(files, w)).collect();
        let lexicon = Compilation {
            doc: IrDocument {
                ir_version: ir::IR_VERSION,
                icons: Vec::new(),
            },
            definitions: outcome.lexicon.defs.len(),
            errors,
            warnings,
            palette: palette_of(&outcome.lexicon),
            provenance: Vec::new(),
        };
        Prepared {
            files: files.to_vec(),
            vocabulary: project::Vocabulary::of(outcome.lexicon.config.as_ref()),
            lexicon: outcome.lexicon,
            resolved,
            outcome: lexicon,
        }
    }

    /// The lexicon's own outcome: its definitions, its parse and resolve
    /// errors, its warnings and its palette, and no icons.
    pub fn outcome(&self) -> &Compilation {
        &self.outcome
    }

    /// One icon, as [`compile_one`] gives it, or [`Refused::Cancelled`]
    /// once `cancel` is set: it is looked at between parts and between
    /// iterations of a repeat, so a compile that is no longer wanted stops
    /// soon after.
    pub fn icon(&self, request: &Only, cancel: &AtomicBool) -> Result<Compilation, Refused> {
        if !self.outcome.errors.is_empty() {
            return Ok(self.outcome.clone());
        }
        let mut c = self
            .icons(std::slice::from_ref(request), cancel)
            .pop()
            .expect("one result for one request")?;
        c.warnings = self.outcome.warnings.clone();
        c.palette = self.outcome.palette.clone();
        Ok(c)
    }

    /// Several icons, as [`compile_many`] gives them after the lexicon's
    /// own outcome, sharing the fonts their text is set in; none when the
    /// lexicon has errors. Once `cancel` is set, the icon being compiled
    /// and every one after it is [`Refused::Cancelled`].
    pub fn icons(
        &self,
        requests: &[Only],
        cancel: &AtomicBool,
    ) -> Vec<Result<Compilation, Refused>> {
        if !self.outcome.errors.is_empty() {
            return Vec::new();
        }
        let files = &self.files;
        expand::expand_many_until(&self.resolved, requests, Some(cancel))
            .into_iter()
            .zip(requests)
            .map(|(expanded, only)| {
                let (expanded, ediags) = expanded?;
                let mut errors: Vec<Diag> = ediags.iter().map(|d| locate(files, d)).collect();
                let (mut doc, mut provenance, pdiags) =
                    project::project_traced(&expanded, &self.vocabulary);
                errors.extend(pdiags.iter().map(|d| locate(files, d)));
                // the icon asked for, and none it badges
                let keep: Vec<bool> = doc.icons.iter().map(|i| i.name == only.icon).collect();
                let mut keeping = keep.iter();
                doc.icons.retain(|_| *keeping.next().expect("one each"));
                let mut keeping = keep.iter();
                provenance.retain(|_| *keeping.next().expect("one each"));
                written_in(&self.lexicon, &mut provenance);
                Ok(Compilation {
                    doc,
                    definitions: self.outcome.definitions,
                    errors,
                    warnings: Vec::new(),
                    palette: None,
                    provenance,
                })
            })
            .collect()
    }
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
