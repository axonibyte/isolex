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
#[derive(Debug, Clone, PartialEq)]
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
            }
            // the reader stops at the first syntax error, and says what it is
            Err(e) => ctx.diag(e.offset, format!("KDL syntax error: {}", e.message)),
        }
        diags.append(&mut ctx.diags);
    }
    ParseOutcome { lexicon, diags }
}

const TOP_NODES: &[&str] = &["lexicon", "base", "icon", "fragment", "axis"];

fn parse_top(ctx: &mut FileCtx, node: kdl::NodeRef, out: &mut Lexicon) {
    let name = node.name().value();
    match name {
        "lexicon" => {
            let mut cfg = LexiconConfig::default();
            // each reserved token with where it was written, checked against
            // the tokens once the whole block is read (either may come first)
            let mut reserved_at: Vec<(String, usize)> = Vec::new();
            if let Some(children) = node.children() {
                for ch in children.nodes() {
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
        "base" | "icon" | "fragment" | "axis" => {
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

fn parse_definition(ctx: &mut FileCtx, node: kdl::NodeRef) -> Option<Definition> {
    let kind = match node.name().value() {
        "base" => DefKind::Base,
        "icon" => DefKind::Icon,
        "fragment" => DefKind::Fragment,
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
            parse_body_child(ctx, ch, kind, &mut def);
        }
    }
    Some(def)
}

fn kindword(kind: DefKind) -> &'static str {
    match kind {
        DefKind::Base => "base",
        DefKind::Icon => "icon",
        DefKind::Fragment => "fragment",
        DefKind::Axis => "axis",
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

fn parse_body_child(ctx: &mut FileCtx, ch: kdl::NodeRef, kind: DefKind, def: &mut Definition) {
    let off = ch.span().offset();
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
                    if let Some(num) = parse_num_entry(ctx, v) {
                        def.body
                            .params
                            .push((n.to_string(), num, ctx.span(ch.span())));
                    }
                }
                _ => ctx.diag(off, "param takes a name and a default: param bays 2"),
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
                    parse_body_child(ctx, gch, DefKind::Icon, &mut sub);
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
        other if limit_kind(other).is_some() => {
            let limit = limit_kind(other).expect("checked");
            if limit.lexicon_wide() {
                ctx.diag(
                    off,
                    format!("{other} counts the whole lexicon; set it in the lexicon block"),
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

fn parse_geometry_children(ctx: &mut FileCtx, node: kdl::NodeRef, gctx: Ctx) -> Vec<Node> {
    let mut out = Vec::new();
    if let Some(children) = node.children() {
        for ch in children.nodes() {
            if let Some(n) = parse_geometry(ctx, ch, gctx) {
                out.push(n);
            }
        }
    }
    out
}

const NODES_3D: &[&str] = &[
    "box", "tile", "cylinder", "facerect", "poly3d", "shadow", "seg", "at", "repeat", "insert",
    "badge",
];
const NODES_2D: &[&str] = &[
    "line", "circle", "ellipse", "rect", "arc", "path", "poly", "repeat", "insert",
];

fn parse_geometry(ctx: &mut FileCtx, ch: kdl::NodeRef, gctx: Ctx) -> Option<Node> {
    let off = ch.span().offset();
    let name = ch.name().value();
    let span = ctx.span(ch.span());

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
            children: parse_geometry_children(ctx, ch, gctx),
            when: when_prop(ctx, ch),
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
                if let Some(v) = parse_num_entry(ctx, e) {
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
            children: parse_geometry_children(ctx, ch, Ctx::PolyPts3),
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
            children: parse_geometry_children(ctx, ch, Ctx::TwoD),
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
            children: parse_geometry_children(ctx, ch, Ctx::PolyPts2),
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

fn string_prop(_ctx: &mut FileCtx, node: kdl::NodeRef, name: &str) -> Option<String> {
    node.entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(name))
        .and_then(|e| e.value().as_string().map(|s| s.to_string()))
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
