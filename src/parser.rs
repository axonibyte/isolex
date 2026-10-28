//! KDL → Isolex AST. Schema enforcement happens here: node names, arg
//! counts and types, property names (with nearest-name suggestions),
//! context validity (2-D nodes only inside `at`, `pt` only inside
//! polygons), and expression parsing — so every later stage works on a
//! structurally valid tree.

use crate::ast::*;
use crate::expr;
use crate::kdl;

/// A diagnostic with a resolved location. Rendering is intentionally
/// plain (`file:line:col: message`) so error output is stable enough to
/// golden-test.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Diag {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for Diag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.col, self.message
        )
    }
}

pub struct ParseOutcome {
    pub lexicon: Lexicon,
    pub diags: Vec<Diag>,
}

struct FileCtx<'a> {
    idx: usize,
    name: &'a str,
    text: &'a str,
    diags: Vec<Diag>,
}

impl<'a> FileCtx<'a> {
    fn line_col(&self, offset: usize) -> (usize, usize) {
        let clamped = offset.min(self.text.len());
        let before = &self.text[..clamped];
        let line = before.matches('\n').count() + 1;
        let col = before.chars().rev().take_while(|&c| c != '\n').count() + 1;
        (line, col)
    }

    fn diag(&mut self, offset: usize, message: impl Into<String>) {
        let (line, col) = self.line_col(offset);
        self.diags.push(Diag {
            file: self.name.to_string(),
            line,
            col,
            message: message.into(),
        });
    }

    fn span(&self, s: kdl::Span) -> Span {
        Span {
            file: self.idx,
            start: s.offset(),
            len: s.len(),
        }
    }
}

fn suggest<'x>(want: &str, have: impl Iterator<Item = &'x str>) -> String {
    let mut best: Option<(usize, &str)> = None;
    for h in have {
        let d = levenshtein(want, h);
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, h));
        }
    }
    match best {
        Some((d, name)) if d <= 2 => format!(" (did you mean '{name}'?)"),
        _ => String::new(),
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Parse a whole lexicon from (path, text) pairs. Paths must already be
/// sorted by the caller; this function preserves the given order.
pub fn parse_lexicon(files: &[(String, String)]) -> ParseOutcome {
    let mut lexicon = Lexicon {
        files: files.iter().map(|(p, _)| p.clone()).collect(),
        ..Default::default()
    };
    let mut diags = Vec::new();

    for (idx, (path, text)) in files.iter().enumerate() {
        let mut ctx = FileCtx {
            idx,
            name: path,
            text,
            diags: Vec::new(),
        };
        match kdl::Document::parse(text) {
            Ok(doc) => {
                for node in doc.nodes() {
                    parse_top(&mut ctx, node, &mut lexicon);
                }
                map_sources(&ctx, &doc, &mut lexicon.sources);
            }
            // the reader stops at the first syntax error, and says what it is
            Err(e) => ctx.diag(e.offset, format!("KDL syntax error: {}", e.message)),
        }
        diags.append(&mut ctx.diags);
    }
    ParseOutcome { lexicon, diags }
}

/// Every node of a file into `map`, without recursion (#28): a lexicon
/// may nest its blocks to any depth.
fn map_sources(ctx: &FileCtx, doc: &kdl::Document, map: &mut SourceMap) {
    let mut todo: Vec<kdl::NodeRef> = doc.nodes().collect();
    while let Some(node) = todo.pop() {
        let mut args = Vec::new();
        let mut props = Vec::new();
        for e in node.entries() {
            let at = ctx.span(e.value_span());
            match e.name() {
                Some(key) => props.push((key.value().to_string(), at)),
                None => args.push(at),
            }
        }
        let kids: Vec<kdl::NodeRef> = node
            .children()
            .map(|c| c.nodes().collect())
            .unwrap_or_default();
        map.insert(NodeSource {
            span: ctx.span(node.span()),
            name: node.name().value().to_string(),
            name_span: ctx.span(node.name().span()),
            args,
            props,
            body: node.body().map(|b| ctx.span(b)),
            children: kids.iter().map(|k| ctx.span(k.span())).collect(),
        });
        todo.extend(kids);
    }
}

const TOP_NODES: &[&str] = &[
    "lexicon", "palette", "base", "icon", "fragment", "axis", "font",
];

fn parse_top(ctx: &mut FileCtx, node: kdl::NodeRef, out: &mut Lexicon) {
    let name = node.name().value();
    match name {
        "lexicon" => {
            only_props(ctx, node, &[]);
            let mut cfg = LexiconConfig::default();
            // each reserved token with where it was written, checked against
            // the tokens once the whole block is read (either may come first)
            let mut reserved_at: Vec<(String, usize)> = Vec::new();
            if let Some(children) = node.children() {
                for ch in children.nodes() {
                    let setting = ch.name().value();
                    if matches!(setting, "tokens" | "reserved" | "stroke-widths")
                        || limit_kind(setting).is_some()
                    {
                        only_props(ctx, ch, &[]);
                    }
                    match ch.name().value() {
                        "tokens" => {
                            cfg.tokens = string_args(ctx, ch);
                        }
                        "reserved" => {
                            for e in ch.entries().iter().filter(|e| e.name().is_none()) {
                                match e.value().as_string() {
                                    Some(s) => reserved_at.push((s.to_string(), e.span().offset())),
                                    None => ctx.diag(e.span().offset(), "expected a string here"),
                                }
                            }
                        }
                        "stroke-widths" => {
                            // checked against every stroke (SPEC §2), so an entry
                            // that is not a width is an error, not skipped
                            for e in ch.entries().iter().filter(|e| e.name().is_none()) {
                                match kdl_num(e.value()) {
                                    Some(w) if w.is_finite() && w > 0.0 => cfg.stroke_widths.push(w),
                                    _ => ctx.diag(
                                        e.span().offset(),
                                        "stroke-widths takes positive numbers: stroke-widths 1.2 1.4",
                                    ),
                                }
                            }
                        }
                        other if limit_kind(other).is_some() => {
                            let kind = limit_kind(other).expect("checked");
                            set_limit(ctx, ch, kind, &mut cfg.limits);
                        }
                        other => ctx.diag(
                            ch.span().offset(),
                            format!(
                                "unknown lexicon setting '{other}'{}",
                                suggest(
                                    other,
                                    ["tokens", "stroke-widths", "reserved"]
                                        .into_iter()
                                        .chain(LimitKind::ALL.iter().map(|k| k.directive()))
                                )
                            ),
                        ),
                    }
                }
            }
            for (token, at) in reserved_at {
                // a reserved token outside a declared vocabulary could never
                // be painted at all
                if !cfg.tokens.is_empty() && !cfg.tokens.contains(&token) {
                    ctx.diag(
                        at,
                        format!(
                            "'{token}' is reserved but is not one of the lexicon's tokens{}; \
                             declare it in tokens too",
                            suggest(&token, cfg.tokens.iter().map(|t| t.as_str()))
                        ),
                    );
                } else if !cfg.reserved.contains(&token) {
                    cfg.reserved.push(token);
                }
            }
            if out.config.is_some() {
                ctx.diag(
                    node.span().offset(),
                    "duplicate 'lexicon' node; one per lexicon",
                );
            } else {
                out.config = Some(cfg);
            }
        }
        "palette" => {
            let block = parse_palette(ctx, node);
            if out.palette.is_some() {
                ctx.diag(
                    node.span().offset(),
                    "duplicate 'palette' node; one per lexicon",
                );
            } else {
                out.palette = Some(block);
            }
        }
        "base" | "icon" | "fragment" | "axis" | "font" => {
            if let Some(def) = parse_definition(ctx, node) {
                out.defs.push(def);
            }
        }
        other => ctx.diag(
            node.span().offset(),
            format!(
                "unknown top-level node '{other}'{}",
                suggest(other, TOP_NODES.iter().copied())
            ),
        ),
    }
}

/// `palette { <token> light="#…" dark="#…" … }` (SPEC §2): whether each
/// token is the lexicon's is checked once every file is read.
fn parse_palette(ctx: &mut FileCtx, node: kdl::NodeRef) -> PaletteBlock {
    only_props(ctx, node, &[]);
    if let Some(e) = node.entries().iter().find(|e| e.name().is_none()) {
        ctx.diag(
            e.span().offset(),
            "palette takes no arguments: palette { ink light=\"#1f2933\" dark=\"#e4e7eb\" }",
        );
    }
    let mut colors: Vec<PaletteColor> = Vec::new();
    for ch in node.children().into_iter().flat_map(|c| c.nodes()) {
        let token = ch.name().value();
        let off = ch.span().offset();
        only_props(ctx, ch, &["light", "dark"]);
        if ch.entries().iter().any(|e| e.name().is_none()) || ch.children().is_some() {
            ctx.diag(
                off,
                format!("a palette color is written {token} light=\"#…\" dark=\"#…\""),
            );
            continue;
        }
        let mut color = |which: &str| -> Option<String> {
            let Some(e) = ch
                .entries()
                .iter()
                .find(|e| e.name().map(|n| n.value()) == Some(which))
            else {
                ctx.diag(
                    off,
                    format!("'{token}' needs a {which} color: {which}=\"#…\""),
                );
                return None;
            };
            match e.value().as_string() {
                Some(c) if is_hex_color(c) => Some(c.to_string()),
                Some(c) => {
                    ctx.diag(
                        e.span().offset(),
                        format!("'{c}' is not a color; write #rgb or #rrggbb"),
                    );
                    None
                }
                None => {
                    ctx.diag(e.span().offset(), format!("{which} takes a color string"));
                    None
                }
            }
        };
        let (light, dark) = (color("light"), color("dark"));
        if colors.iter().any(|c| c.token == token) {
            ctx.diag(off, format!("'{token}' is colored twice in the palette"));
            continue;
        }
        if let (Some(light), Some(dark)) = (light, dark) {
            colors.push(PaletteColor {
                token: token.to_string(),
                light,
                dark,
                span: ctx.span(ch.span()),
            });
        }
    }
    PaletteBlock {
        colors,
        span: ctx.span(node.span()),
    }
}

fn is_hex_color(c: &str) -> bool {
    c.strip_prefix('#')
        .is_some_and(|hex| matches!(hex.len(), 3 | 6) && hex.chars().all(|d| d.is_ascii_hexdigit()))
}

fn parse_definition(ctx: &mut FileCtx, node: kdl::NodeRef) -> Option<Definition> {
    let kind = match node.name().value() {
        "base" => DefKind::Base,
        "icon" => DefKind::Icon,
        "fragment" => DefKind::Fragment,
        "font" => DefKind::Font,
        _ => DefKind::Axis,
    };
    let Some(name) = first_string_arg(ctx, node) else {
        ctx.diag(
            node.span().offset(),
            format!(
                "a {} needs a name: {} \"name\" {{ … }}",
                kindword(kind),
                node.name().value()
            ),
        );
        return None;
    };

    let mut extends = None;
    for entry in node.entries() {
        match entry.name().map(|n| n.value()) {
            None => {} // the name arg
            Some("extends") if kind == DefKind::Axis => ctx.diag(
                entry.span().offset(),
                "an axis cannot extend: its variants overlay the icons that opt into it",
            ),
            Some("extends") if kind == DefKind::Font => {
                ctx.diag(entry.span().offset(), FONT_EXTENDS)
            }
            Some("extends") => match entry.value().as_string() {
                Some(s) => extends = Some(s.to_string()),
                None => ctx.diag(
                    entry.span().offset(),
                    "extends takes a definition name string",
                ),
            },
            Some(other) => ctx.diag(
                entry.span().offset(),
                format!(
                    "unknown property '{other}' on a {}{}",
                    kindword(kind),
                    suggest(other, ["extends"].into_iter())
                ),
            ),
        }
    }

    let mut def = Definition {
        kind,
        name,
        extends,
        body: DefBody::default(),
        variants: Vec::new(),
        emit_base: true,
        span: ctx.span(node.span()),
    };

    if let Some(children) = node.children() {
        for ch in children.nodes() {
            parse_body_child(ctx, ch, kind, false, &mut def);
        }
    }
    Some(def)
}

const FONT_EXTENDS: &str = "a font cannot extend: its weights and styles are variants of its axes";

fn kindword(kind: DefKind) -> &'static str {
    match kind {
        DefKind::Base => "base",
        DefKind::Icon => "icon",
        DefKind::Fragment => "fragment",
        DefKind::Axis => "axis",
        DefKind::Font => "font",
    }
}

const BODY_NODES: &[&str] = &[
    "extends",
    "category",
    "footprint",
    "canvas",
    "describe",
    "param",
    "part",
    "replace",
    "remove",
    "axes",
    "grant",
    "variant",
    "emit-base",
    "shape-limit",
    "repeat-limit",
    "insert-depth",
    "expression-depth",
    "transform-chain",
    "text-length",
    "text-limit",
    "glyph-limit",
    "view",
    "transform-bounds",
    "em",
    "ascent",
    "descent",
    "slant",
    "kind",
    "glyph",
    "notdef",
    "kern",
];

/// What a font holds; everything else in one belongs in an icon.
const FONT_NODES: &[&str] = &[
    "describe",
    "param",
    "axes",
    "em",
    "ascent",
    "descent",
    "slant",
    "kind",
    "glyph",
    "notdef",
    "kern",
    "glyph-limit",
];

/// What only a font holds, of the nodes above.
const FONT_ONLY: &[&str] = &[
    "em",
    "ascent",
    "descent",
    "slant",
    "kind",
    "glyph",
    "notdef",
    "kern",
    "glyph-limit",
];

fn limit_kind(name: &str) -> Option<LimitKind> {
    LimitKind::ALL.into_iter().find(|k| k.directive() == name)
}

/// `shape-limit 250000`, or `shape-limit #null` for none.
fn set_limit(ctx: &mut FileCtx, node: kdl::NodeRef, kind: LimitKind, limits: &mut Limits) {
    let off = node.span().offset();
    let name = kind.directive();
    let args: Vec<&kdl::Entry> = node
        .entries()
        .iter()
        .filter(|e| e.name().is_none())
        .collect();
    let limit = match args.as_slice() {
        [e] => match e.value() {
            kdl::Value::Integer(i) => u64::try_from(*i).ok().filter(|n| *n > 0).map(Limit::At),
            kdl::Value::Null => Some(Limit::Unlimited),
            _ => None,
        },
        _ => None,
    };
    let Some(limit) = limit else {
        ctx.diag(
            off,
            format!("{name} takes a positive whole number, or #null for no limit"),
        );
        return;
    };
    if limits.insert(kind, limit).is_some() {
        ctx.diag(off, format!("{name} is set twice here"));
    }
}

/// One node of a definition body. A variant's body (`in_variant`) may
/// hold what an icon holds or what a font does, since an axis can
/// overlay either; what does not fit is an error where it is applied.
fn parse_body_child(
    ctx: &mut FileCtx,
    ch: kdl::NodeRef,
    kind: DefKind,
    in_variant: bool,
    def: &mut Definition,
) {
    let off = ch.span().offset();
    let node_name = ch.name().value();
    if kind == DefKind::Font && !FONT_NODES.contains(&node_name) {
        ctx.diag(
            off,
            if node_name == "extends" {
                FONT_EXTENDS.to_string()
            } else if BODY_NODES.contains(&node_name) || node_name == "variant" {
                format!(
                    "a font holds em, ascent, descent, slant, kind, glyphs, notdef, kerns, \
                     params, axes and describe; '{node_name}' belongs in an icon"
                )
            } else {
                format!(
                    "unknown node '{node_name}' in a font{}",
                    suggest(node_name, FONT_NODES.iter().copied())
                )
            },
        );
        return;
    }
    if kind != DefKind::Font && !in_variant && FONT_ONLY.contains(&node_name) {
        ctx.diag(
            off,
            format!(
                "'{node_name}' belongs in a font, not in {} {}",
                if kind == DefKind::Icon { "an" } else { "a" },
                kindword(kind)
            ),
        );
        return;
    }
    // An axis is a set of overlays; anything else in it would be dropped.
    if kind == DefKind::Axis && !matches!(ch.name().value(), "variant" | "emit-base") {
        let name = ch.name().value();
        ctx.diag(
            off,
            if name == "extends" {
                "an axis cannot extend: its variants overlay the icons that opt into it".to_string()
            } else {
                format!(
                    "an axis holds only variant and emit-base; '{name}' belongs inside a variant"
                )
            },
        );
        return;
    }
    if BODY_NODES.contains(&ch.name().value()) {
        only_props(
            ctx,
            ch,
            match ch.name().value() {
                "grant" => &["reason"],
                "glyph" | "notdef" => &["advance"],
                _ => &[],
            },
        );
    }
    match ch.name().value() {
        "extends" => match (first_string_arg(ctx, ch), &def.extends) {
            (Some(parent), None) => def.extends = Some(parent),
            (Some(_), Some(_)) => ctx.diag(
                off,
                "extends declared twice (property and node, or two nodes)",
            ),
            (None, _) => ctx.diag(off, "extends takes a definition name string"),
        },
        "category" => {
            def.body.category = first_string_arg(ctx, ch);
        }
        "describe" => {
            if let Some(s) = first_string_arg(ctx, ch) {
                def.body.describe.push(s);
            }
        }
        "footprint" => {
            if let Some(v) = num_args::<3>(ctx, ch) {
                def.body.footprint = Some(v);
            }
        }
        "canvas" => {
            if let Some(v) = num_args::<2>(ctx, ch) {
                def.body.canvas = Some(v);
            }
        }
        "param" => {
            let entries = ch.entries();
            let name = entries
                .first()
                .filter(|e| e.name().is_none())
                .and_then(|e| e.value().as_string());
            let value = entries.get(1).filter(|e| e.name().is_none());
            match (name, value) {
                (Some(n), Some(v)) => {
                    if anchor_name(n) {
                        ctx.diag(off, reserved(n, "param"));
                        return;
                    }
                    if let Some(param) = parse_param_entry(ctx, v) {
                        def.body
                            .params
                            .push((n.to_string(), param, ctx.span(ch.span())));
                    }
                }
                _ => ctx.diag(
                    off,
                    "param takes a name and a default: param bays 2, or param label (text)\"U1\"",
                ),
            }
        }
        "axes" => {
            def.body.axes = string_args(ctx, ch);
        }
        "grant" if kind == DefKind::Fragment => ctx.diag(
            off,
            "a fragment cannot grant: its shapes are painted as part of the icon it is \
             inserted into, so grant on that icon",
        ),
        "grant" => {
            let token = first_string_arg(ctx, ch);
            let reason = ch
                .entries()
                .iter()
                .find(|e| e.name().map(|n| n.value()) == Some("reason"))
                .and_then(|e| e.value().as_string());
            match (token, reason) {
                (Some(token), Some(reason)) => def.body.grants.push(Grant {
                    token,
                    reason: reason.to_string(),
                    span: ctx.span(ch.span()),
                }),
                _ => ctx.diag(
                    off,
                    "grant takes a token and a reason: grant \"accent\" reason=\"…\"",
                ),
            }
        }
        "part" | "replace" => {
            let Some(name) = first_string_arg(ctx, ch) else {
                ctx.diag(off, "a part needs a name");
                return;
            };
            let body_ctx = if kind == DefKind::Fragment {
                Ctx::AnyD
            } else {
                Ctx::ThreeD
            };
            let nodes = parse_geometry_children(ctx, ch, body_ctx);
            def.body.parts.push(PartDecl {
                op: if ch.name().value() == "part" {
                    PartOp::Add
                } else {
                    PartOp::Replace
                },
                name,
                nodes,
                span: ctx.span(ch.span()),
            });
        }
        "remove" => {
            let Some(name) = first_string_arg(ctx, ch) else {
                ctx.diag(off, "remove takes the name of an inherited part");
                return;
            };
            def.body.parts.push(PartDecl {
                op: PartOp::Remove,
                name,
                nodes: Vec::new(),
                span: ctx.span(ch.span()),
            });
        }
        "variant" if kind == DefKind::Axis => {
            let Some(name) = first_string_arg(ctx, ch) else {
                ctx.diag(off, "a variant needs a name");
                return;
            };
            let mut sub = Definition {
                kind: DefKind::Axis,
                name: name.clone(),
                extends: None,
                body: DefBody::default(),
                variants: Vec::new(),
                emit_base: true,
                span: ctx.span(ch.span()),
            };
            if let Some(children) = ch.children() {
                for gch in children.nodes() {
                    if gch.name().value() == "extends" {
                        ctx.diag(
                            gch.span().offset(),
                            "a variant cannot extend: it overlays the icon that opts into its axis",
                        );
                        continue;
                    }
                    parse_body_child(ctx, gch, DefKind::Icon, true, &mut sub);
                }
            }
            def.variants.push(Variant {
                name,
                body: sub.body,
                span: ctx.span(ch.span()),
            });
        }
        "emit-base" if kind == DefKind::Axis => match ch.entries().first().map(|e| e.value()) {
            Some(kdl::Value::Bool(b)) => def.emit_base = *b,
            _ => ctx.diag(off, "emit-base takes #true or #false"),
        },
        "view" | "transform-bounds" if kind == DefKind::Fragment => ctx.diag(
            off,
            format!(
                "a fragment is drawn as part of the icon it is inserted into; set {} on that icon",
                ch.name().value()
            ),
        ),
        "view" => {
            let args: Vec<&kdl::Entry> =
                ch.entries().iter().filter(|e| e.name().is_none()).collect();
            match args.as_slice() {
                [e] => def.body.view = parse_num_entry(ctx, e),
                _ => ctx.diag(off, "view takes one angle, a quarter-turn: view 90"),
            }
        }
        "transform-bounds" => match first_string_arg(ctx, ch).as_deref() {
            Some(name) => match Bounds::named(name) {
                Some(b) => def.body.transform_bounds = Some(b),
                None => ctx.diag(
                    off,
                    format!(
                        "transform-bounds is \"error\", \"grow\", \"scale\" or \"clip\"{}",
                        suggest(name, Bounds::NAMES.into_iter())
                    ),
                ),
            },
            None => ctx.diag(
                off,
                "transform-bounds takes \"error\", \"grow\", \"scale\" or \"clip\"",
            ),
        },
        "em" | "ascent" | "descent" | "slant" => {
            let args: Vec<&kdl::Entry> =
                ch.entries().iter().filter(|e| e.name().is_none()).collect();
            let [e] = args.as_slice() else {
                ctx.diag(
                    off,
                    format!("{node_name} takes one number, in the font's em grid: {node_name} 8"),
                );
                return;
            };
            let value = parse_num_entry(ctx, e);
            let font = &mut def.body.font;
            match node_name {
                "em" => font.em = value,
                "ascent" => font.ascent = value,
                "descent" => font.descent = value,
                _ => font.slant = value,
            }
        }
        "kind" => match first_string_arg(ctx, ch).as_deref() {
            Some("stroke") => def.body.font.kind = Some(FontKind::Stroke),
            Some("outline") => def.body.font.kind = Some(FontKind::Outline),
            other => ctx.diag(
                off,
                format!(
                    "kind is \"stroke\" (glyphs drawn along their middles) or \"outline\" \
                     (filled outlines){}",
                    other
                        .map(|s| suggest(s, ["stroke", "outline"].into_iter()))
                        .unwrap_or_default()
                ),
            ),
        },
        "glyph" | "notdef" => {
            let name = if node_name == "glyph" {
                match first_string_arg(ctx, ch) {
                    Some(name) if !name.is_empty() => name,
                    _ => {
                        ctx.diag(
                            off,
                            "a glyph needs the text it draws: glyph \"A\" advance=6 { … }",
                        );
                        return;
                    }
                }
            } else {
                if ch.entries().iter().any(|e| e.name().is_none()) {
                    ctx.diag(off, "notdef takes no name: it draws what no glyph does");
                    return;
                }
                String::new()
            };
            let Some(advance) = num_prop(ctx, ch, "advance") else {
                if !ch
                    .entries()
                    .iter()
                    .any(|e| e.name().map(|n| n.value()) == Some("advance"))
                {
                    ctx.diag(
                        off,
                        format!("{node_name} needs advance=, how far it moves the pen"),
                    );
                }
                return;
            };
            let glyph = Glyph {
                name,
                advance,
                nodes: parse_geometry_children(ctx, ch, Ctx::TwoD),
                span: ctx.span(ch.span()),
            };
            let font = &mut def.body.font;
            if node_name == "notdef" {
                if font.notdef.is_some() {
                    ctx.diag(off, "notdef is drawn twice here; one is enough");
                    return;
                }
                font.notdef = Some(glyph);
            } else if font.glyphs.iter().any(|g| g.name == glyph.name) {
                ctx.diag(
                    off,
                    format!("glyph \"{}\" is drawn twice in this font", glyph.name),
                );
            } else {
                font.glyphs.push(glyph);
            }
        }
        "kern" => {
            let args: Vec<&kdl::Entry> =
                ch.entries().iter().filter(|e| e.name().is_none()).collect();
            let usage = "kern takes two glyphs and the space between them: kern \"A\" \"V\" -0.6";
            let [l, r, by] = args.as_slice() else {
                ctx.diag(off, usage);
                return;
            };
            let (Some(left), Some(right)) = (l.value().as_string(), r.value().as_string()) else {
                ctx.diag(off, usage);
                return;
            };
            let Some(by) = parse_num_entry(ctx, by) else {
                return;
            };
            let font = &mut def.body.font;
            if font
                .kerns
                .iter()
                .any(|k| k.left == left && k.right == right)
            {
                ctx.diag(
                    off,
                    format!("\"{left}\" \"{right}\" is kerned twice in this font"),
                );
                return;
            }
            font.kerns.push(Kern {
                left: left.to_string(),
                right: right.to_string(),
                by,
                span: ctx.span(ch.span()),
            });
        }
        other if limit_kind(other).is_some() => {
            let limit = limit_kind(other).expect("checked");
            if limit.lexicon_wide() {
                ctx.diag(
                    off,
                    format!("{other} counts the whole lexicon; set it in the lexicon block"),
                );
            } else if kind == DefKind::Font && !limit.of_fonts() {
                ctx.diag(
                    off,
                    format!(
                        "{other} counts what an icon draws; set it on the icon or in the \
                         lexicon block"
                    ),
                );
            } else if kind == DefKind::Fragment {
                ctx.diag(
                    off,
                    format!(
                        "a fragment's work counts toward the icon it is inserted into; \
                         set {other} on that icon or in the lexicon block"
                    ),
                );
            } else {
                set_limit(ctx, ch, limit, &mut def.body.limits);
            }
        }
        other => ctx.diag(
            off,
            format!(
                "unknown node '{other}' in a {}{}",
                kindword(kind),
                suggest(other, BODY_NODES.iter().copied())
            ),
        ),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Ctx {
    ThreeD,
    TwoD,
    /// Fragment part bodies: either dimension parses; the expander
    /// validates against the actual insertion site.
    AnyD,
    PolyPts3,
    PolyPts2,
}

/// The geometry in `node`'s children block, built with an explicit stack
/// rather than recursion, since a lexicon may nest blocks to any depth
/// (#28). Nodes are read in document order, so diagnostics come in the
/// order a recursive reading gave them: a node's own, then its
/// children's, then (for a repeat) its `when`.
fn parse_geometry_children(ctx: &mut FileCtx, node: kdl::NodeRef, gctx: Ctx) -> Vec<Node> {
    /// A block being read: its nodes, how far through, in which context,
    /// what has been built, and the node it belongs to, waiting for it.
    struct Frame<'d> {
        kids: Vec<kdl::NodeRef<'d>>,
        next: usize,
        gctx: Ctx,
        built: Vec<Node>,
        owner: Option<(Node, kdl::NodeRef<'d>)>,
    }
    fn kids(node: kdl::NodeRef<'_>) -> Vec<kdl::NodeRef<'_>> {
        node.children()
            .map(|c| c.nodes().collect())
            .unwrap_or_default()
    }
    let mut stack = vec![Frame {
        kids: kids(node),
        next: 0,
        gctx,
        built: Vec::new(),
        owner: None,
    }];
    loop {
        let top = stack.last_mut().expect("the outermost block");
        if let Some(&ch) = top.kids.get(top.next) {
            top.next += 1;
            let gctx = top.gctx;
            let Some(n) = parse_geometry(ctx, ch, gctx) else {
                continue;
            };
            // the context its own children are read in, if it has any
            let inner = match n {
                Node::Repeat { .. } | Node::Transform { .. } => Some(gctx),
                Node::Turn { .. } if gctx == Ctx::AnyD => Some(Ctx::AnyD),
                Node::Turn { .. } => Some(Ctx::ThreeD),
                Node::Poly3d { .. } => Some(Ctx::PolyPts3),
                Node::At { .. } => Some(Ctx::TwoD),
                Node::Poly2d { .. } => Some(Ctx::PolyPts2),
                _ => None,
            };
            // a transform's steps block is read with the transform
            let mut children = kids(ch);
            if matches!(n, Node::Transform { .. }) {
                children.retain(|k| k.name().value() != "steps");
            }
            match inner {
                Some(inner) => stack.push(Frame {
                    kids: children,
                    next: 0,
                    gctx: inner,
                    built: Vec::new(),
                    owner: Some((n, ch)),
                }),
                None => stack.last_mut().expect("a block").built.push(n),
            }
        } else {
            let frame = stack.pop().expect("a block");
            let Some((mut n, ch)) = frame.owner else {
                return frame.built;
            };
            if let Some(children) = n.children_mut() {
                *children = frame.built;
            }
            if let Node::Repeat { when, .. } = &mut n {
                *when = when_prop(ctx, ch);
            }
            stack.last_mut().expect("its parent's block").built.push(n);
        }
    }
}

const NODES_3D: &[&str] = &[
    "box",
    "tile",
    "cylinder",
    "facerect",
    "poly3d",
    "shadow",
    "seg",
    "at",
    "repeat",
    "insert",
    "badge",
    "transform",
    "turn",
    "mirror-diagonal",
];
const NODES_2D: &[&str] = &[
    "line",
    "circle",
    "ellipse",
    "rect",
    "arc",
    "path",
    "poly",
    "repeat",
    "insert",
    "transform",
    "text",
];

fn parse_geometry(ctx: &mut FileCtx, ch: kdl::NodeRef, gctx: Ctx) -> Option<Node> {
    let off = ch.span().offset();
    let name = ch.name().value();
    let span = ctx.span(ch.span());

    // an unknown node is reported as one; insert's properties are params
    let known = name == "pt" || NODES_3D.contains(&name) || NODES_2D.contains(&name);
    if known && name != "insert" {
        only_props(ctx, ch, geometry_props(name));
    }

    // pt is special: only valid inside poly bodies
    if name == "pt" {
        return match gctx {
            Ctx::PolyPts3 => num_args::<3>(ctx, ch).map(|args| Node::Pt3 { args, span }),
            Ctx::PolyPts2 => num_args::<2>(ctx, ch).map(|args| Node::Pt2 { args, span }),
            _ => {
                ctx.diag(off, "'pt' is only valid inside poly or poly3d");
                None
            }
        };
    }

    // repeat and insert are valid in every context and inherit it
    if name == "repeat" {
        let entries = ch.entries();
        let count = entries
            .first()
            .filter(|e| e.name().is_none())
            .and_then(|e| parse_num_entry(ctx, e));
        let var = entries
            .get(1)
            .filter(|e| e.name().is_none())
            .and_then(|e| e.value().as_string());
        let (Some(count), Some(var)) = (count, var) else {
            ctx.diag(
                off,
                "repeat takes a count and a variable name: repeat 12 \"i\" { … }",
            );
            return None;
        };
        if anchor_name(var) {
            ctx.diag(off, reserved(var, "repeat variable"));
            return None;
        }
        return Some(Node::Repeat {
            count,
            var: var.to_string(),
            // children, and then its `when`, are read by parse_geometry_children
            children: Vec::new(),
            when: None,
            span,
        });
    }
    if name == "insert" {
        let fragment = first_string_arg(ctx, ch)?;
        let mut args = Vec::new();
        for e in ch.entries() {
            if let Some(pname) = e.name().map(|n| n.value()) {
                if pname == "when" {
                    continue;
                }
                if let Some(v) = parse_param_entry(ctx, e) {
                    args.push((pname.to_string(), v));
                }
            }
        }
        return Some(Node::Insert {
            fragment,
            args,
            when: when_prop(ctx, ch),
            span,
        });
    }

    let in_2d = matches!(gctx, Ctx::TwoD);
    let valid: &[&str] = match gctx {
        Ctx::TwoD => NODES_2D,
        Ctx::AnyD => {
            // union of both worlds; expand validates the insertion site
            if NODES_2D.contains(&name) || NODES_3D.contains(&name) {
                if NODES_2D.contains(&name) && !NODES_3D.contains(&name) {
                    NODES_2D
                } else {
                    NODES_3D
                }
            } else {
                NODES_3D
            }
        }
        _ => NODES_3D,
    };
    if !valid.contains(&name) {
        let wrong_ctx = if in_2d {
            NODES_3D.contains(&name)
        } else {
            NODES_2D.contains(&name)
        };
        if wrong_ctx {
            ctx.diag(
                off,
                if in_2d {
                    format!("'{name}' is a 3-D node and cannot appear inside 'at'")
                } else {
                    format!("'{name}' is a 2-D node and only appears inside an 'at' anchor group")
                },
            );
        } else {
            ctx.diag(
                off,
                format!(
                    "unknown geometry node '{name}'{}",
                    suggest(name, valid.iter().copied())
                ),
            );
        }
        return None;
    }

    let style = style_props(ctx, ch);
    let when = when_prop(ctx, ch);
    match name {
        "box" => Some(Node::Box3 {
            args: num_args::<6>(ctx, ch)?,
            fills: fills_prop(ctx, ch)?,
            opacity: num_prop(ctx, ch, "opacity"),
            when,
            span,
        }),
        "tile" => Some(Node::Tile {
            args: num_args::<5>(ctx, ch)?,
            style,
            when,
            span,
        }),
        "cylinder" => Some(Node::Cylinder {
            args: num_args::<5>(ctx, ch)?,
            top: string_prop(ctx, ch, "top"),
            side: string_prop(ctx, ch, "side"),
            opacity: num_prop(ctx, ch, "opacity"),
            when,
            span,
        }),
        "facerect" => {
            let entries = ch.entries();
            let face = match entries
                .first()
                .filter(|e| e.name().is_none())
                .and_then(|e| e.value().as_string())
            {
                Some("front") => Face::Front,
                Some("right") => Face::Right,
                _ => {
                    ctx.diag(off, "facerect's first argument is \"front\" or \"right\"");
                    return None;
                }
            };
            let args = num_args_from(ctx, ch, 1, off, 5, "facerect")?;
            Some(Node::FaceRect {
                face,
                args: args.try_into().ok()?,
                fill: string_prop(ctx, ch, "fill"),
                opacity: num_prop(ctx, ch, "opacity"),
                when,
                span,
            })
        }
        "poly3d" => Some(Node::Poly3d {
            style,
            children: Vec::new(),
            when,
            span,
        }),
        "shadow" => Some(Node::Shadow {
            args: num_args::<4>(ctx, ch)?,
            when,
            span,
        }),
        "seg" => Some(Node::Seg {
            args: num_args::<6>(ctx, ch)?,
            style,
            when,
            span,
        }),
        "at" => Some(Node::At {
            args: num_args::<3>(ctx, ch)?,
            children: Vec::new(),
            when,
            span,
        }),
        "line" => Some(Node::Line {
            args: num_args::<4>(ctx, ch)?,
            style,
            when,
            span,
        }),
        "circle" => Some(Node::Circle {
            args: num_args::<3>(ctx, ch)?,
            style,
            when,
            span,
        }),
        "ellipse" => Some(Node::Ellipse {
            args: num_args::<4>(ctx, ch)?,
            style,
            when,
            span,
        }),
        "rect" => Some(Node::Rect2d {
            args: num_args::<4>(ctx, ch)?,
            rx: num_prop(ctx, ch, "rx"),
            style,
            when,
            span,
        }),
        "arc" => Some(Node::Arc {
            args: num_args::<5>(ctx, ch)?,
            style,
            when,
            span,
        }),
        "path" => {
            let Some(d) = ch
                .entries()
                .iter()
                .find(|e| e.name().map(|n| n.value()) == Some("d"))
                .and_then(|e| e.value().as_string().map(|s| (e.span().offset(), s)))
            else {
                ctx.diag(off, "path needs d=\"…\"");
                return None;
            };
            let template = parse_path_template(ctx, d.0, d.1)?;
            Some(Node::Path {
                template,
                style,
                when,
                span,
            })
        }
        "poly" => Some(Node::Poly2d {
            style,
            children: Vec::new(),
            when,
            span,
        }),
        "badge" => {
            let icon = first_string_arg(ctx, ch)?;
            Some(Node::Badge {
                icon,
                corner: string_prop(ctx, ch, "corner").unwrap_or_else(|| "ne".to_string()),
                scale: num_prop(ctx, ch, "scale"),
                when,
                span,
            })
        }
        "transform" => Some(Node::Transform {
            spec: transform_spec(ctx, ch)?,
            children: Vec::new(),
            when,
            span,
        }),
        "turn" => {
            let [angle] = num_args::<1>(ctx, ch)?;
            Some(Node::Turn {
                by: TurnBy::Angle(angle),
                origin: list_prop(ctx, ch, "origin", &[2])?.map(pair),
                children: Vec::new(),
                when,
                span,
            })
        }
        "mirror-diagonal" => {
            num_args::<0>(ctx, ch)?;
            Some(Node::Turn {
                by: TurnBy::MirrorDiagonal,
                origin: list_prop(ctx, ch, "origin", &[2])?.map(pair),
                children: Vec::new(),
                when,
                span,
            })
        }
        "text" => {
            let args: Vec<&kdl::Entry> =
                ch.entries().iter().filter(|e| e.name().is_none()).collect();
            let [x, y, content] = args.as_slice() else {
                ctx.diag(
                    off,
                    format!(
                        "text takes where it starts and what it says, found {} argument(s): \
                         text \"x\" \"y\" \"U{{u}}\" font=\"…\" size=6",
                        args.len()
                    ),
                );
                return None;
            };
            let (x, y) = (parse_num_entry(ctx, x), parse_num_entry(ctx, y));
            let Some(words) = content.value().as_string() else {
                ctx.diag(
                    content.span().offset(),
                    "text's third argument is what it says, a string",
                );
                return None;
            };
            let content = parse_text_template(ctx, args[2].span().offset(), words, "text")?;
            let spec = text_spec(ctx, ch, style.fill)?;
            Some(Node::Text {
                args: [x?, y?],
                content,
                spec,
                when,
                span,
            })
        }
        _ => unreachable!("validated against the context's node list"),
    }
}

fn parse_path_template(ctx: &mut FileCtx, off: usize, d: &str) -> Option<Vec<PathPiece>> {
    let mut pieces = Vec::new();
    let mut lit = String::new();
    let mut chars = d.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '{' {
            let rest = &d[i + 1..];
            let Some(end) = rest.find('}') else {
                ctx.diag(off, "unclosed '{' in path template");
                return None;
            };
            let src = &rest[..end];
            match expr::parse_num(src) {
                Ok(ast) => {
                    if !lit.is_empty() {
                        pieces.push(PathPiece::Lit(std::mem::take(&mut lit)));
                    }
                    pieces.push(PathPiece::Expr {
                        src: src.to_string(),
                        ast,
                    });
                }
                Err(e) => {
                    ctx.diag(
                        off,
                        format!("in path template expression '{src}': {}", e.message),
                    );
                    return None;
                }
            }
            for _ in 0..end + 1 {
                chars.next();
            }
        } else {
            lit.push(c);
        }
    }
    if !lit.is_empty() {
        pieces.push(PathPiece::Lit(lit));
    }
    Some(pieces)
}

/// Text with `{…}` interpolations, as text content and text params write
/// it: each `{…}` an expression, or a text param's name; `{{` and `}}`
/// are a brace of their own.
fn parse_text_template(
    ctx: &mut FileCtx,
    off: usize,
    text: &str,
    what: &str,
) -> Option<Vec<PathPiece>> {
    let mut pieces = Vec::new();
    let mut lit = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if rest.starts_with("{{") {
            lit.push('{');
            rest = &rest[2..];
        } else if rest.starts_with("}}") {
            lit.push('}');
            rest = &rest[2..];
        } else if c == '{' {
            let Some(end) = rest.find('}') else {
                ctx.diag(
                    off,
                    format!("unclosed '{{' in {what}; write {{{{ for a brace of its own"),
                );
                return None;
            };
            let src = &rest[1..end];
            match expr::parse_num(src) {
                Ok(ast) => {
                    if !lit.is_empty() {
                        pieces.push(PathPiece::Lit(std::mem::take(&mut lit)));
                    }
                    pieces.push(PathPiece::Expr {
                        src: src.to_string(),
                        ast,
                    });
                }
                Err(e) => {
                    ctx.diag(off, format!("in {what} expression '{src}': {}", e.message));
                    return None;
                }
            }
            rest = &rest[end + 1..];
        } else {
            lit.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    if !lit.is_empty() {
        pieces.push(PathPiece::Lit(lit));
    }
    Some(pieces)
}

/// A param's value, a default or an insert's argument: a number or an
/// expression, or text, annotated `(text)` since a string is otherwise an
/// expression.
fn parse_param_entry(ctx: &mut FileCtx, e: &kdl::Entry) -> Option<Param> {
    match e.ty().map(|t| t.value()) {
        None => parse_num_entry(ctx, e).map(Param::Num),
        Some("text") => match e.value().as_string() {
            Some(s) => parse_text_template(ctx, e.span().offset(), s, "text").map(Param::Text),
            None => {
                ctx.diag(e.span().offset(), "(text) takes a string: (text)\"U{u}\"");
                None
            }
        },
        Some(other) => {
            ctx.diag(
                e.span().offset(),
                format!(
                    "unknown type ({other}); a value is a number, an expression, or \
                     (text)\"…\"{}",
                    suggest(other, ["text"].into_iter())
                ),
            );
            None
        }
    }
}

/// A `text` node's properties.
fn text_spec(ctx: &mut FileCtx, ch: kdl::NodeRef, fill: Option<String>) -> Option<TextSpec> {
    let off = ch.span().offset();
    let Some(font) = string_prop(ctx, ch, "font") else {
        ctx.diag(off, "text needs font=\"…\", the font it is set in");
        return None;
    };
    let Some(size) = num_prop(ctx, ch, "size") else {
        ctx.diag(off, "text needs size=, its font's em in output units");
        return None;
    };
    let styles = string_prop(ctx, ch, "style")
        .map(|s| s.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();
    let anchor = match string_prop(ctx, ch, "anchor").as_deref() {
        None | Some("start") => TextAnchor::Start,
        Some("middle") => TextAnchor::Middle,
        Some("end") => TextAnchor::End,
        Some(other) => {
            ctx.diag(
                off,
                format!(
                    "anchor is \"start\", \"middle\" or \"end\"{}",
                    suggest(other, ["start", "middle", "end"].into_iter())
                ),
            );
            return None;
        }
    };
    const BASELINES: [&str; 4] = ["alphabetic", "top", "middle", "bottom"];
    let baseline = match string_prop(ctx, ch, "baseline").as_deref() {
        None | Some("alphabetic") => Baseline::Alphabetic,
        Some("top") => Baseline::Top,
        Some("middle") => Baseline::Middle,
        Some("bottom") => Baseline::Bottom,
        Some(other) => {
            ctx.diag(
                off,
                format!(
                    "baseline is \"alphabetic\", \"top\", \"middle\" or \"bottom\"{}",
                    suggest(other, BASELINES.into_iter())
                ),
            );
            return None;
        }
    };
    let bounds = match ch
        .entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some("bounds"))
    {
        None => None,
        Some(e) => {
            let value = e.value().as_string();
            match value.and_then(Bounds::named) {
                Some(b) => Some(b),
                None => {
                    ctx.diag(
                        e.span().offset(),
                        format!(
                            "bounds is \"error\", \"grow\", \"scale\" or \"clip\"{}",
                            value
                                .map(|s| suggest(s, Bounds::NAMES.into_iter()))
                                .unwrap_or_default()
                        ),
                    );
                    return None;
                }
            }
        }
    };
    Some(TextSpec {
        font,
        styles,
        size,
        anchor,
        baseline,
        fill,
        max_width: num_prop(ctx, ch, "max-width"),
        bounds,
    })
}

// --- entry helpers -------------------------------------------------------

fn kdl_num(v: &kdl::Value) -> Option<f64> {
    match v {
        kdl::Value::Integer(i) => Some(*i as f64),
        kdl::Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn parse_num_entry(ctx: &mut FileCtx, e: &kdl::Entry) -> Option<Num> {
    if let Some(n) = kdl_num(e.value()) {
        // KDL 2.0 spells these #inf, #-inf and #nan; JSON has none of them
        if !n.is_finite() {
            let kdl = if n.is_nan() {
                "#nan"
            } else if n > 0.0 {
                "#inf"
            } else {
                "#-inf"
            };
            ctx.diag(
                e.span().offset(),
                format!("{kdl} is not a finite number; every number must be"),
            );
            return None;
        }
        return Some(Num::Lit(n));
    }
    if let Some(s) = e.value().as_string() {
        return match expr::parse_num(s) {
            Ok(ast) => Some(Num::Expr {
                src: s.to_string(),
                ast,
            }),
            Err(err) => {
                ctx.diag(
                    e.span().offset(),
                    format!("in expression '{s}': {}", err.message),
                );
                None
            }
        };
    }
    ctx.diag(
        e.span().offset(),
        "expected a number or an expression string here",
    );
    None
}

fn num_args<const N: usize>(ctx: &mut FileCtx, node: kdl::NodeRef) -> Option<[Num; N]> {
    let off = node.span().offset();
    let v = num_args_from(ctx, node, 0, off, N, node.name().value())?;
    v.try_into().ok()
}

fn num_args_from(
    ctx: &mut FileCtx,
    node: kdl::NodeRef,
    skip: usize,
    off: usize,
    want: usize,
    what: &str,
) -> Option<Vec<Num>> {
    let args: Vec<&kdl::Entry> = node
        .entries()
        .iter()
        .filter(|e| e.name().is_none())
        .skip(skip)
        .collect();
    if args.len() != want {
        ctx.diag(
            off,
            format!(
                "{what} takes {want} positional argument(s), found {}",
                args.len()
            ),
        );
        return None;
    }
    let mut out = Vec::with_capacity(want);
    for e in args {
        out.push(parse_num_entry(ctx, e)?);
    }
    Some(out)
}

fn first_string_arg(ctx: &mut FileCtx, node: kdl::NodeRef) -> Option<String> {
    let e = node.entries().iter().find(|e| e.name().is_none())?;
    match e.value().as_string() {
        Some(s) => Some(s.to_string()),
        None => {
            ctx.diag(e.span().offset(), "expected a string here");
            None
        }
    }
}

fn string_args(ctx: &mut FileCtx, node: kdl::NodeRef) -> Vec<String> {
    let mut out = Vec::new();
    for e in node.entries().iter().filter(|e| e.name().is_none()) {
        match e.value().as_string() {
            Some(s) => out.push(s.to_string()),
            None => ctx.diag(e.span().offset(), "expected a string here"),
        }
    }
    out
}

fn string_prop(ctx: &mut FileCtx, node: kdl::NodeRef, name: &str) -> Option<String> {
    let e = node
        .entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(name))?;
    match e.value().as_string() {
        Some(s) => Some(s.to_string()),
        // used to be dropped as if it were not there
        None => {
            ctx.diag(e.span().offset(), format!("{name} takes a string"));
            None
        }
    }
}

fn num_prop(ctx: &mut FileCtx, node: kdl::NodeRef, name: &str) -> Option<Num> {
    let e = node
        .entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(name))?;
    parse_num_entry(ctx, e)
}

fn when_prop(ctx: &mut FileCtx, node: kdl::NodeRef) -> Option<Num> {
    let e = node
        .entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some("when"))?;
    let Some(s) = e.value().as_string() else {
        ctx.diag(e.span().offset(), "when takes a boolean expression string");
        return None;
    };
    match expr::parse_bool(s) {
        Ok(ast) => Some(Num::Expr {
            src: s.to_string(),
            ast,
        }),
        Err(err) => {
            ctx.diag(
                e.span().offset(),
                format!("in when guard '{s}': {}", err.message),
            );
            None
        }
    }
}

fn style_props(ctx: &mut FileCtx, node: kdl::NodeRef) -> Style {
    Style {
        fill: string_prop(ctx, node, "fill"),
        stroke: string_prop(ctx, node, "stroke"),
        width: num_prop(ctx, node, "width"),
        dash: string_prop(ctx, node, "dash"),
        opacity: num_prop(ctx, node, "opacity"),
    }
}

/// The properties each geometry node reads. `insert` is not here: its
/// properties are the fragment's params.
fn geometry_props(name: &str) -> &'static [&'static str] {
    const STYLE: &[&str] = &["fill", "stroke", "width", "dash", "opacity", "when"];
    match name {
        "box" => &["fills", "opacity", "when"],
        "cylinder" => &["top", "side", "opacity", "when"],
        "facerect" => &["fill", "opacity", "when"],
        "rect" => &["fill", "stroke", "width", "dash", "opacity", "rx", "when"],
        "path" => &["fill", "stroke", "width", "dash", "opacity", "d", "when"],
        "badge" => &["corner", "scale", "when"],
        "transform" => &[
            "origin",
            "translate",
            "rotate",
            "scale",
            "skew-x",
            "skew-y",
            "mirror",
            "matrix",
            "onto",
            "of",
            "bounds",
            "strokes",
            "when",
        ],
        "turn" | "mirror-diagonal" => &["origin", "when"],
        "text" => &[
            "font",
            "style",
            "size",
            "anchor",
            "baseline",
            "fill",
            "max-width",
            "bounds",
            "when",
        ],
        "shadow" | "at" | "repeat" => &["when"],
        "pt" => &[],
        _ => STYLE,
    }
}

/// Every property is read by name, so one a node does not read would be
/// dropped without a word (#39): `box … fill="brand"` painted nothing and
/// said nothing. Each is an error instead, saying what the node takes.
fn only_props(ctx: &mut FileCtx, node: kdl::NodeRef, takes: &[&str]) {
    let what = node.name().value();
    for e in node.entries() {
        let Some(prop) = e.name().map(|n| n.value()) else {
            continue;
        };
        if takes.contains(&prop) {
            continue;
        }
        let list = match takes {
            [] => format!("{what} takes no properties"),
            [one] => format!("{what} takes only {one}"),
            [init @ .., last] => format!("{what} takes {} and {last}", init.join(", ")),
        };
        ctx.diag(
            e.span().offset(),
            format!(
                "unknown property '{prop}' on {what}; {list}{}",
                suggest(prop, takes.iter().copied())
            ),
        );
    }
}

/// Inside `at`, `x` and `y` are the anchor's screen position, so a param
/// or loop variable of either name would be overridden there.
fn anchor_name(name: &str) -> bool {
    name == "x" || name == "y"
}

fn reserved(name: &str, what: &str) -> String {
    format!(
        "'{name}' is reserved for the anchor's screen position inside `at`; \
         give this {what} another name"
    )
}

/// A box's `fills`: body (the default), virt or none. Anything else used
/// to fall back to the body fills without a word.
fn fills_prop(ctx: &mut FileCtx, node: kdl::NodeRef) -> Option<Option<String>> {
    let Some(e) = node
        .entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some("fills"))
    else {
        return Some(None);
    };
    const FILLS: [&str; 3] = ["body", "virt", "none"];
    match e.value().as_string() {
        Some(s) if FILLS.contains(&s) => Some(Some(s.to_string())),
        other => {
            ctx.diag(
                e.span().offset(),
                format!(
                    "fills is \"body\", \"virt\" or \"none\"{}",
                    other
                        .map(|s| suggest(s, FILLS.into_iter()))
                        .unwrap_or_default()
                ),
            );
            None
        }
    }
}

// --- transforms ------------------------------------------------------------

/// A list of numbers in one string, as `origin="cx cy"` writes two:
/// values apart by spaces, each a number or an expression, one with
/// spaces of its own in parentheses (`"(w / 2) 0"`). A plain number is a
/// list of one.
fn parse_num_list(
    ctx: &mut FileCtx,
    e: &kdl::Entry,
    what: &str,
    counts: &[usize],
) -> Option<Vec<Num>> {
    let off = e.span().offset();
    let want = match counts {
        [n] => format!("{n}"),
        [a, b] => format!("{a} or {b}"),
        _ => unreachable!("one or two counts"),
    };
    let complaint = format!(
        "{what} takes {want} number(s) apart by spaces; write an expression with spaces of its own in parentheses: \"(w / 2) 0\""
    );
    let values = match e.value().as_string() {
        None => vec![parse_num_entry(ctx, e)?],
        Some(text) => {
            let mut pieces = Vec::new();
            let mut depth = 0i32;
            let mut piece = String::new();
            for c in text.chars() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if c.is_whitespace() && depth == 0 {
                    if !piece.is_empty() {
                        pieces.push(std::mem::take(&mut piece));
                    }
                } else {
                    piece.push(c);
                }
            }
            if !piece.is_empty() {
                pieces.push(piece);
            }
            // the count first: `w / 2 0` is four pieces, not a bad `/`
            if !counts.contains(&pieces.len()) {
                ctx.diag(off, complaint);
                return None;
            }
            let mut values = Vec::new();
            for piece in pieces {
                match expr::parse_num(&piece) {
                    Ok(ast) => values.push(Num::Expr { src: piece, ast }),
                    Err(err) => {
                        ctx.diag(off, format!("in {what} value '{piece}': {}", err.message));
                        return None;
                    }
                }
            }
            values
        }
    };
    if !counts.contains(&values.len()) {
        ctx.diag(off, complaint);
        return None;
    }
    Some(values)
}

/// A property holding a list of numbers, if it is there; `None` within
/// for one that is not, and an outer `None` for one that is wrong.
fn list_prop(
    ctx: &mut FileCtx,
    node: kdl::NodeRef,
    name: &str,
    counts: &[usize],
) -> Option<Option<Vec<Num>>> {
    let Some(e) = node
        .entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(name))
    else {
        return Some(None);
    };
    parse_num_list(ctx, e, name, counts).map(Some)
}

fn pair(v: Vec<Num>) -> [Num; 2] {
    let mut it = v.into_iter();
    let a = it.next().expect("two");
    [a, it.next().expect("two")]
}

/// One value, or two: `scale=2` is `scale="2 2"`.
fn both(v: Vec<Num>) -> [Num; 2] {
    if v.len() == 1 {
        [v[0].clone(), v[0].clone()]
    } else {
        pair(v)
    }
}

fn mirror_axis(ctx: &mut FileCtx, off: usize, value: Option<&str>) -> Option<MirrorAxis> {
    match value {
        Some("across") => Some(MirrorAxis::Across),
        Some("down") => Some(MirrorAxis::Down),
        other => {
            ctx.diag(
                off,
                format!(
                    "mirror is \"across\" (left for right) or \"down\" (top for bottom){}",
                    other
                        .map(|s| suggest(s, ["across", "down"].into_iter()))
                        .unwrap_or_default()
                ),
            );
            None
        }
    }
}

const STEP_NAMES: [&str; 7] = [
    "translate",
    "rotate",
    "scale",
    "skew-x",
    "skew-y",
    "mirror",
    "matrix",
];

/// One step of a `steps` block: its name and positional arguments.
fn parse_step(ctx: &mut FileCtx, node: kdl::NodeRef) -> Option<Step> {
    let off = node.span().offset();
    let name = node.name().value();
    only_props(ctx, node, &[]);
    let args: Vec<&kdl::Entry> = node
        .entries()
        .iter()
        .filter(|e| e.name().is_none())
        .collect();
    let count = |ctx: &mut FileCtx, counts: &[usize], usage: &str| -> bool {
        if counts.contains(&args.len()) {
            true
        } else {
            ctx.diag(
                off,
                format!("{name} takes {usage}, found {} argument(s)", args.len()),
            );
            false
        }
    };
    let nums = |ctx: &mut FileCtx| -> Option<Vec<Num>> {
        let mut out = Vec::new();
        for e in &args {
            out.push(parse_num_entry(ctx, e)?);
        }
        Some(out)
    };
    match name {
        "translate" if count(ctx, &[2], "two numbers: translate 2 0") => {
            Some(Step::Translate(pair(nums(ctx)?)))
        }
        "rotate" if count(ctx, &[1], "an angle in degrees: rotate 90") => {
            Some(Step::Rotate(nums(ctx)?.remove(0)))
        }
        "scale" if count(ctx, &[1, 2], "one number or two: scale 2, scale 2 1") => {
            Some(Step::Scale(both(nums(ctx)?)))
        }
        "skew-x" if count(ctx, &[1], "an angle in degrees: skew-x -12") => {
            Some(Step::SkewX(nums(ctx)?.remove(0)))
        }
        "skew-y" if count(ctx, &[1], "an angle in degrees: skew-y 12") => {
            Some(Step::SkewY(nums(ctx)?.remove(0)))
        }
        "mirror" if count(ctx, &[1], "\"across\" or \"down\"") => {
            let value = args[0].value().as_string();
            Some(Step::Mirror(mirror_axis(
                ctx,
                args[0].span().offset(),
                value,
            )?))
        }
        "matrix" if count(ctx, &[6], "six numbers: matrix 1 0 0 1 0 0") => {
            let v = nums(ctx)?;
            Some(Step::Matrix(Box::new(v.try_into().ok()?)))
        }
        "translate" | "rotate" | "scale" | "skew-x" | "skew-y" | "mirror" | "matrix" => None,
        other => {
            ctx.diag(
                off,
                format!(
                    "unknown step '{other}'; a step is translate, rotate, scale, skew-x, skew-y, mirror or matrix{}",
                    suggest(other, STEP_NAMES.into_iter())
                ),
            );
            None
        }
    }
}

/// A `transform`'s properties and steps block. Steps written as
/// properties apply in one order: mirror, scale, skew-y, skew-x, rotate,
/// then translate.
fn transform_spec(ctx: &mut FileCtx, ch: kdl::NodeRef) -> Option<TransformSpec> {
    let off = ch.span().offset();
    let prop = |name: &str| {
        ch.entries()
            .iter()
            .find(|e| e.name().map(|n| n.value()) == Some(name))
    };
    if ch.entries().iter().any(|e| e.name().is_none()) {
        ctx.diag(
            off,
            "transform takes properties, not arguments: transform rotate=90 { … }",
        );
        return None;
    }
    let mut spec = TransformSpec {
        origin: list_prop(ctx, ch, "origin", &[2])?.map(pair),
        ..TransformSpec::default()
    };

    // as properties, in their fixed order
    if let Some(e) = prop("mirror") {
        let value = e.value().as_string();
        spec.steps
            .push(Step::Mirror(mirror_axis(ctx, e.span().offset(), value)?));
    }
    if let Some(v) = list_prop(ctx, ch, "scale", &[1, 2])? {
        spec.steps.push(Step::Scale(both(v)));
    }
    if let Some(v) = num_prop(ctx, ch, "skew-y") {
        spec.steps.push(Step::SkewY(v));
    }
    if let Some(v) = num_prop(ctx, ch, "skew-x") {
        spec.steps.push(Step::SkewX(v));
    }
    if let Some(v) = num_prop(ctx, ch, "rotate") {
        spec.steps.push(Step::Rotate(v));
    }
    if let Some(v) = list_prop(ctx, ch, "translate", &[2])? {
        spec.steps.push(Step::Translate(pair(v)));
    }
    let matrix = list_prop(ctx, ch, "matrix", &[6])?;

    // or as a steps block, in its own order
    let blocks: Vec<kdl::NodeRef> = ch
        .children()
        .map(|c| c.nodes().filter(|n| n.name().value() == "steps").collect())
        .unwrap_or_default();
    if let Some(extra) = blocks.get(1) {
        ctx.diag(extra.span().offset(), "a transform has one steps block");
        return None;
    }
    if let Some(block) = blocks.first() {
        only_props(ctx, *block, &[]);
        if block.entries().iter().any(|e| e.name().is_none()) {
            ctx.diag(
                block.span().offset(),
                "steps takes no arguments, only a block of steps",
            );
            return None;
        }
        if !spec.steps.is_empty() || matrix.is_some() {
            ctx.diag(
                block.span().offset(),
                "a transform's steps are its properties or a steps block, not both",
            );
            return None;
        }
        let nodes: Vec<kdl::NodeRef> = block
            .children()
            .map(|c| c.nodes().collect())
            .unwrap_or_default();
        if nodes.is_empty() {
            ctx.diag(
                block.span().offset(),
                "an empty steps block; give it a step, or remove it",
            );
            return None;
        }
        for node in nodes {
            spec.steps.push(parse_step(ctx, node)?);
        }
    }
    if let Some(m) = matrix {
        if !spec.steps.is_empty() {
            ctx.diag(
                off,
                "matrix stands alone: it is the whole map, so give it no other steps",
            );
            return None;
        }
        if spec.origin.is_some() {
            ctx.diag(
                off,
                "matrix is applied as written, so origin has nothing to do; remove it",
            );
            return None;
        }
        spec.steps.push(Step::Matrix(Box::new(m.try_into().ok()?)));
    }

    // laid on a face
    match (prop("onto"), prop("of")) {
        (Some(onto), Some(of)) => {
            let face = match onto.value().as_string() {
                Some("top") => OntoFace::Top,
                Some("front") => OntoFace::Front,
                Some("right") => OntoFace::Right,
                other => {
                    ctx.diag(
                        onto.span().offset(),
                        format!(
                            "onto is \"top\", \"front\" or \"right\", a face of the box `of` names{}",
                            other
                                .map(|s| suggest(s, ["top", "front", "right"].into_iter()))
                                .unwrap_or_default()
                        ),
                    );
                    return None;
                }
            };
            let usage = "of names a box as box does: of=\"box 0 0 0 1 1 1\"";
            let Some(text) = of.value().as_string() else {
                ctx.diag(of.span().offset(), usage);
                return None;
            };
            let Some(rest) = text.trim_start().strip_prefix("box ") else {
                ctx.diag(of.span().offset(), usage);
                return None;
            };
            // the six numbers after `box`, read as a list
            let mut values = Vec::new();
            let mut depth = 0i32;
            let mut piece = String::new();
            for c in rest.chars().chain(std::iter::once(' ')) {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if c.is_whitespace() && depth == 0 {
                    if !piece.is_empty() {
                        match expr::parse_num(&piece) {
                            Ok(ast) => values.push(Num::Expr {
                                src: std::mem::take(&mut piece),
                                ast,
                            }),
                            Err(err) => {
                                ctx.diag(
                                    of.span().offset(),
                                    format!("in of value '{piece}': {}", err.message),
                                );
                                return None;
                            }
                        }
                    }
                } else {
                    piece.push(c);
                }
            }
            let Ok(values) = <[Num; 6]>::try_from(values) else {
                ctx.diag(of.span().offset(), format!("{usage}: x y z w d h"));
                return None;
            };
            spec.onto = Some((face, values));
        }
        (Some(e), None) => {
            ctx.diag(
                e.span().offset(),
                "onto needs of, the box whose face it is: of=\"box 0 0 0 1 1 1\"",
            );
            return None;
        }
        (None, Some(e)) => {
            ctx.diag(
                e.span().offset(),
                "of names the box for onto; give onto too, or remove it",
            );
            return None;
        }
        (None, None) => {}
    }

    if let Some(e) = prop("bounds") {
        let value = e.value().as_string();
        match value.and_then(Bounds::named) {
            Some(b) => spec.bounds = Some(b),
            None => {
                ctx.diag(
                    e.span().offset(),
                    format!(
                        "bounds is \"error\", \"grow\", \"scale\" or \"clip\"{}",
                        value
                            .map(|s| suggest(s, Bounds::NAMES.into_iter()))
                            .unwrap_or_default()
                    ),
                );
                return None;
            }
        }
    }
    if let Some(e) = prop("strokes") {
        match e.value().as_string() {
            Some("scale") => spec.strokes = Some(Strokes::Scale),
            Some("fixed") => spec.strokes = Some(Strokes::Fixed),
            other => {
                ctx.diag(
                    e.span().offset(),
                    format!(
                        "strokes is \"scale\" (the default) or \"fixed\"{}",
                        other
                            .map(|s| suggest(s, ["scale", "fixed"].into_iter()))
                            .unwrap_or_default()
                    ),
                );
                return None;
            }
        }
    }
    if spec.steps.is_empty() && spec.onto.is_none() {
        ctx.diag(
            off,
            "transform needs something to do: translate, rotate, scale, skew-x, skew-y, mirror or matrix, \
             a steps block, or onto",
        );
        return None;
    }
    Some(spec)
}
