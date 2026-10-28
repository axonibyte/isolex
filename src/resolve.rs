//! Inheritance resolution: extends chains, part merging, param
//! layering, axis/grant unions. Everything stays symbolic (Num may be an
//! expression) — evaluation happens at expansion, which is what makes
//! parent geometry see child parameter values (late binding).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::ast::*;

/// A diagnostic carrying a span; stages after the parser do not hold
/// source text, so the CLI renders these to file:line:col at the edge.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpanDiag {
    pub span: Span,
    pub message: String,
}

/// A definition after its extends chain has been merged. Parts are in
/// final render order; values are still unevaluated.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedDef {
    pub name: String,
    pub kind: DefKind,
    pub category: Option<String>,
    pub footprint: Option<[Num; 3]>,
    pub canvas: Option<[Num; 2]>,
    pub describe: Vec<String>,
    /// Declaration-ordered; later entries with the same name never occur
    /// (overrides replace in place).
    pub params: Vec<(String, Param)>,
    pub axes: Vec<String>,
    pub grants: Vec<Grant>,
    pub parts: Vec<(String, Vec<Node>)>,
    /// Limits set along the chain, child over parent, each with the name
    /// of the definition (or variant) that set it.
    pub limits: BTreeMap<LimitKind, (Limit, String)>,
    /// The corner it is drawn from, and what its transforms do past its
    /// frame: child over parent.
    pub view: Option<Num>,
    pub transform_bounds: Option<Bounds>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct AxisDef {
    pub name: String,
    pub emit_base: bool,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Default)]
pub struct Resolved {
    /// Concrete icons only, in definition order.
    pub icons: Vec<ResolvedDef>,
    /// Bases too, by name — the expander does not need them, but `docs`
    /// and consumers inspecting the tree do.
    pub bases: BTreeMap<String, ResolvedDef>,
    pub fragments: BTreeMap<String, Definition>,
    pub fonts: BTreeMap<String, Definition>,
    pub axes: BTreeMap<String, AxisDef>,
    /// Limits the lexicon block sets for every icon.
    pub lexicon_limits: Limits,
    pub diags: Vec<SpanDiag>,
    /// Diagnostics that do not fail the build (SPEC §8).
    pub warnings: Vec<SpanDiag>,
}

struct Ctx {
    diags: Vec<SpanDiag>,
}

impl Ctx {
    fn diag(&mut self, span: Span, message: impl Into<String>) {
        self.diags.push(SpanDiag {
            span,
            message: message.into(),
        });
    }
}

pub(crate) fn suggest<'x>(want: &str, have: impl Iterator<Item = &'x str>) -> String {
    // small duplicate of the parser's helper to keep module boundaries
    // clean; the tie-break (first minimal) is deterministic
    fn lev(a: &str, b: &str) -> usize {
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
    let mut best: Option<(usize, &str)> = None;
    for h in have {
        let d = lev(want, h);
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, h));
        }
    }
    match best {
        Some((d, name)) if d <= 2 => format!(" (did you mean '{name}'?)"),
        _ => String::new(),
    }
}

pub fn resolve(lexicon: &Lexicon) -> Resolved {
    let mut ctx = Ctx { diags: Vec::new() };
    let mut out = Resolved {
        lexicon_limits: lexicon
            .config
            .as_ref()
            .map(|c| c.limits.clone())
            .unwrap_or_default(),
        ..Resolved::default()
    };

    // --- the palette colors only the lexicon's tokens (SPEC §2) --------
    if let (Some(palette), Some(tokens)) = (
        &lexicon.palette,
        lexicon
            .config
            .as_ref()
            .map(|c| &c.tokens)
            .filter(|t| !t.is_empty()),
    ) {
        for color in &palette.colors {
            if !tokens.contains(&color.token) {
                ctx.diag(
                    color.span,
                    format!(
                        "the palette colors '{}', which is not one of the lexicon's tokens{}",
                        color.token,
                        suggest(&color.token, tokens.iter().map(|t| t.as_str()))
                    ),
                );
            }
        }
    }

    // --- index and duplicate detection --------------------------------
    let mut index: BTreeMap<&str, &Definition> = BTreeMap::new();
    for def in &lexicon.defs {
        if index.insert(def.name.as_str(), def).is_some() {
            ctx.diag(
                def.span,
                format!(
                    "duplicate definition '{}' — one global namespace, one name each",
                    def.name
                ),
            );
        }
        match def.kind {
            DefKind::Fragment => {
                if def.extends.is_some() {
                    ctx.diag(def.span, "fragments cannot extend anything");
                }
                out.fragments.insert(def.name.clone(), def.clone());
            }
            DefKind::Axis => {
                out.axes.insert(
                    def.name.clone(),
                    AxisDef {
                        name: def.name.clone(),
                        emit_base: def.emit_base,
                        variants: def.variants.clone(),
                    },
                );
            }
            DefKind::Font => {
                out.fonts.insert(def.name.clone(), def.clone());
            }
            _ => {}
        }
    }

    check_grants(&mut ctx, lexicon, &mut out.warnings);
    for def in lexicon.defs.iter().filter(|d| d.kind == DefKind::Font) {
        check_font(&mut ctx, def, &out.axes);
    }

    // --- resolve each base/icon ---------------------------------------
    for def in &lexicon.defs {
        if !matches!(def.kind, DefKind::Base | DefKind::Icon) {
            continue;
        }
        let Some(resolved) = resolve_one(&mut ctx, &index, def) else {
            continue;
        };
        match def.kind {
            DefKind::Base => {
                out.bases.insert(def.name.clone(), resolved);
            }
            DefKind::Icon => {
                // completeness check for emitted definitions
                let mut missing = Vec::new();
                if resolved.category.is_none() {
                    missing.push("category");
                }
                if resolved.footprint.is_none() && resolved.canvas.is_none() {
                    missing.push("footprint or canvas");
                }
                if !missing.is_empty() {
                    ctx.diag(
                        def.span,
                        format!(
                            "icon '{}' resolves without {} — every emitted icon needs them",
                            def.name,
                            missing.join(" and ")
                        ),
                    );
                    continue;
                }
                // axis references must exist
                let known: Vec<&str> = out.axes.keys().map(|s| s.as_str()).collect();
                let mut ok = true;
                for a in &resolved.axes {
                    if !out.axes.contains_key(a) {
                        ctx.diag(
                            def.span,
                            format!(
                                "icon '{}' opts into unknown axis '{a}'{}",
                                def.name,
                                suggest(a, known.iter().copied())
                            ),
                        );
                        ok = false;
                    }
                }
                if ok {
                    out.icons.push(resolved);
                }
            }
            _ => unreachable!(),
        }
    }

    out.diags = ctx.diags;
    out
}

fn resolve_one(
    ctx: &mut Ctx,
    index: &BTreeMap<&str, &Definition>,
    leaf: &Definition,
) -> Option<ResolvedDef> {
    // Walk the chain root-first.
    let mut chain: Vec<&Definition> = vec![leaf];
    let mut seen: Vec<&str> = vec![leaf.name.as_str()];
    let mut cur = leaf;
    while let Some(parent_name) = &cur.extends {
        let Some(parent) = index.get(parent_name.as_str()) else {
            let candidates = index
                .iter()
                .filter(|(_, d)| matches!(d.kind, DefKind::Base | DefKind::Icon))
                .map(|(n, _)| *n);
            ctx.diag(
                cur.span,
                format!(
                    "'{}' extends unknown definition '{parent_name}'{}",
                    cur.name,
                    suggest(parent_name, candidates)
                ),
            );
            return None;
        };
        if !matches!(parent.kind, DefKind::Base | DefKind::Icon) {
            ctx.diag(
                cur.span,
                format!(
                    "'{}' extends '{parent_name}', which is a {:?} — only base and icon definitions can be extended",
                    cur.name, parent.kind
                ),
            );
            return None;
        }
        if seen.contains(&parent_name.as_str()) {
            let mut cycle: Vec<&str> = seen.clone();
            cycle.push(parent_name);
            ctx.diag(leaf.span, format!("extends cycle: {}", cycle.join(" -> ")));
            return None;
        }
        seen.push(parent_name.as_str());
        chain.push(parent);
        cur = parent;
    }
    chain.reverse(); // root first

    let mut r = ResolvedDef {
        name: leaf.name.clone(),
        kind: leaf.kind,
        category: None,
        footprint: None,
        canvas: None,
        describe: Vec::new(),
        params: Vec::new(),
        axes: Vec::new(),
        grants: Vec::new(),
        parts: Vec::new(),
        limits: BTreeMap::new(),
        view: None,
        transform_bounds: None,
        span: leaf.span,
    };

    for def in chain {
        merge_body(ctx, &mut r, &def.body, &def.name);
    }
    Some(r)
}

/// Merge one definition body (or axis variant body) onto the resolved
/// accumulator, child-over-parent.
fn merge_body(ctx: &mut Ctx, r: &mut ResolvedDef, body: &DefBody, whose: &str) {
    if let Some(c) = &body.category {
        r.category = Some(c.clone());
    }
    if let Some(f) = &body.footprint {
        r.footprint = Some(f.clone());
    }
    if let Some(c) = &body.canvas {
        r.canvas = Some(c.clone());
    }
    if let Some(v) = &body.view {
        r.view = Some(v.clone());
    }
    if let Some(b) = body.transform_bounds {
        r.transform_bounds = Some(b);
    }
    r.describe.extend(body.describe.iter().cloned());
    merge_params(ctx, &mut r.params, &body.params, whose);
    for a in &body.axes {
        if !r.axes.contains(a) {
            r.axes.push(a.clone());
        }
    }
    // A union by token, in first-seen order; a child's reason replaces
    // its parent's, the way a child's value wins everywhere else.
    for grant in &body.grants {
        match r.grants.iter_mut().find(|g| g.token == grant.token) {
            Some(slot) => *slot = grant.clone(),
            None => r.grants.push(grant.clone()),
        }
    }
    for (kind, limit) in &body.limits {
        r.limits.insert(*kind, (*limit, whose.to_string()));
    }

    for decl in &body.parts {
        let existing = r.parts.iter().position(|(n, _)| n == &decl.name);
        match (decl.op, existing) {
            (PartOp::Add, None) => r.parts.push((decl.name.clone(), decl.nodes.clone())),
            (PartOp::Add, Some(_)) => ctx.diag(
                decl.span,
                format!(
                    "'{whose}' declares part \"{}\" which already exists on an ancestor — \
                     use replace \"{0}\" to override it, or a new name to add alongside \
                     (silent shadowing is how packs rot)",
                    decl.name
                ),
            ),
            (PartOp::Replace, Some(i)) => r.parts[i].1 = decl.nodes.clone(),
            (PartOp::Remove, Some(i)) => {
                r.parts.remove(i);
            }
            (PartOp::Replace | PartOp::Remove, None) => {
                let names: Vec<&str> = r.parts.iter().map(|(n, _)| n.as_str()).collect();
                ctx.diag(
                    decl.span,
                    format!(
                        "'{whose}' tries to {} part \"{}\", but no ancestor defines it{}",
                        if decl.op == PartOp::Replace {
                            "replace"
                        } else {
                            "remove"
                        },
                        decl.name,
                        suggest(&decl.name, names.into_iter())
                    ),
                );
            }
        }
    }
}

/// Params override in place, keeping their slot; a param keeps its type,
/// number or text, down the chain.
fn merge_params(
    ctx: &mut Ctx,
    params: &mut Vec<(String, Param)>,
    over: &[(String, Param, Span)],
    whose: &str,
) {
    fn kind(p: &Param) -> &'static str {
        match p {
            Param::Num(_) => "a number",
            Param::Text(_) => "text",
        }
    }
    for (name, value, span) in over {
        match params.iter_mut().find(|(n, _)| n == name) {
            Some(slot) if kind(&slot.1) != kind(value) => ctx.diag(
                *span,
                format!(
                    "'{whose}' sets param '{name}' to {}, but it is {} where it was declared; \
                     a param keeps its type",
                    kind(value),
                    kind(&slot.1)
                ),
            ),
            Some(slot) => slot.1 = value.clone(),
            None => params.push((name.clone(), value.clone())),
        }
    }
}

/// What a font is drawn with: its own body, then each style's variant
/// laid over it, in its axes' order.
#[derive(Debug, Clone)]
pub struct FontStyled<'a> {
    pub name: String,
    pub params: Vec<(String, Param)>,
    pub em: Option<&'a Num>,
    pub ascent: Option<&'a Num>,
    pub descent: Option<&'a Num>,
    pub slant: Option<&'a Num>,
    pub kind: Option<FontKind>,
    /// In first-seen order; a style's glyph replaces the font's in place.
    pub glyphs: Vec<&'a Glyph>,
    pub notdef: Option<&'a Glyph>,
    pub kerns: Vec<&'a Kern>,
    /// The glyph-limit set along the way, and by whom.
    pub glyph_limit: Option<(Limit, String)>,
}

/// A font with these variants laid over it, or what is wrong with them:
/// a variant that draws what only an icon has.
pub fn style_font<'a>(
    font: &'a Definition,
    variants: &[&'a Variant],
    diags: &mut Vec<SpanDiag>,
) -> Option<FontStyled<'a>> {
    let mut ctx = Ctx { diags: Vec::new() };
    let mut out = FontStyled {
        name: font.name.clone(),
        params: Vec::new(),
        em: None,
        ascent: None,
        descent: None,
        slant: None,
        kind: None,
        glyphs: Vec::new(),
        notdef: None,
        kerns: Vec::new(),
        glyph_limit: None,
    };
    let bodies = std::iter::once((font.name.clone(), &font.body, font.span)).chain(
        variants
            .iter()
            .map(|v| (format!("variant {}", v.name), &v.body, v.span)),
    );
    for (whose, body, span) in bodies {
        if let Some(what) = icon_only(body) {
            ctx.diag(
                span,
                format!(
                    "{whose} sets {what}, which only an icon has; it styles font '{}' here",
                    font.name
                ),
            );
            continue;
        }
        merge_params(&mut ctx, &mut out.params, &body.params, &whose);
        let f = &body.font;
        out.em = f.em.as_ref().or(out.em);
        out.ascent = f.ascent.as_ref().or(out.ascent);
        out.descent = f.descent.as_ref().or(out.descent);
        out.slant = f.slant.as_ref().or(out.slant);
        out.kind = f.kind.or(out.kind);
        for g in &f.glyphs {
            match out.glyphs.iter_mut().find(|h| h.name == g.name) {
                Some(slot) => *slot = g,
                None => out.glyphs.push(g),
            }
        }
        out.notdef = f.notdef.as_ref().or(out.notdef);
        for k in &f.kerns {
            match out
                .kerns
                .iter_mut()
                .find(|j| j.left == k.left && j.right == k.right)
            {
                Some(slot) => *slot = k,
                None => out.kerns.push(k),
            }
        }
        if let Some(l) = body.limits.get(&LimitKind::GlyphLimit) {
            out.glyph_limit = Some((*l, whose.trim_start_matches("variant ").to_string()));
        }
    }
    let ok = ctx.diags.is_empty();
    diags.append(&mut ctx.diags);
    ok.then_some(out)
}

/// The first thing a body sets that a font cannot have, if any.
fn icon_only(body: &DefBody) -> Option<&'static str> {
    if body.category.is_some() {
        Some("a category")
    } else if body.footprint.is_some() {
        Some("a footprint")
    } else if body.canvas.is_some() {
        Some("a canvas")
    } else if !body.parts.is_empty() {
        Some("parts")
    } else if !body.grants.is_empty() {
        Some("grants")
    } else if body.view.is_some() {
        Some("a view")
    } else if body.transform_bounds.is_some() {
        Some("transform-bounds")
    } else if body.limits.keys().any(|k| !k.of_fonts()) {
        Some("an icon's limits")
    } else {
        None
    }
}

/// What a font must say for text to be set in it, and that its kerning
/// names its glyphs and its axes exist.
fn check_font(ctx: &mut Ctx, def: &Definition, axes: &BTreeMap<String, AxisDef>) {
    let f = &def.body.font;
    let mut missing = Vec::new();
    if f.em.is_none() {
        missing.push("em");
    }
    if f.ascent.is_none() {
        missing.push("ascent");
    }
    if f.descent.is_none() {
        missing.push("descent");
    }
    if f.kind.is_none() {
        missing.push("kind");
    }
    if !missing.is_empty() {
        let list = match missing.as_slice() {
            [one] => one.to_string(),
            [init @ .., last] => format!("{} and {last}", init.join(", ")),
            [] => unreachable!(),
        };
        ctx.diag(
            def.span,
            format!("font '{}' needs {list}: every font says them", def.name),
        );
    }
    // a style can add glyphs, so kerning is checked against the font's own
    // only when no axis could
    if def.body.axes.is_empty() {
        for k in &f.kerns {
            for side in [&k.left, &k.right] {
                if !f.glyphs.iter().any(|g| &g.name == side) {
                    ctx.diag(
                        k.span,
                        format!(
                            "font '{}' kerns \"{side}\", which it has no glyph for{}",
                            def.name,
                            suggest(side, f.glyphs.iter().map(|g| g.name.as_str()))
                        ),
                    );
                }
            }
        }
    }
    for a in &def.body.axes {
        if !axes.contains_key(a) {
            ctx.diag(
                def.span,
                format!(
                    "font '{}' opts into unknown axis '{a}'{}",
                    def.name,
                    suggest(a, axes.keys().map(|k| k.as_str()))
                ),
            );
        }
    }
}

/// Public entry the expander uses to overlay an axis variant.
pub fn apply_variant(r: &mut ResolvedDef, variant: &Variant, diags: &mut Vec<SpanDiag>) {
    let mut ctx = Ctx { diags: Vec::new() };
    if !variant.body.font.is_empty() {
        ctx.diag(
            variant.span,
            format!(
                "variant {} sets a font's metrics, glyphs or kerning, which only a font has; it \
                 overlays icon '{}' here",
                variant.name, r.name
            ),
        );
    }
    merge_body(
        &mut ctx,
        r,
        &variant.body,
        &format!("variant {}", variant.name),
    );
    diags.append(&mut ctx.diags);
}

/// Each `grant`, once, where it is written (SPEC §2): with tokens reserved,
/// granting one that is not is an error; with none reserved, a grant has
/// no effect, which is a warning.
fn check_grants(ctx: &mut Ctx, lexicon: &Lexicon, warnings: &mut Vec<SpanDiag>) {
    let reserved: &[String] = lexicon
        .config
        .as_ref()
        .map(|c| c.reserved.as_slice())
        .unwrap_or_default();
    for def in &lexicon.defs {
        let bodies = std::iter::once((format!("'{}'", def.name), &def.body)).chain(
            def.variants.iter().map(|v| {
                (
                    format!("variant '{}' of axis '{}'", v.name, def.name),
                    &v.body,
                )
            }),
        );
        for (whose, body) in bodies {
            for grant in &body.grants {
                let token = &grant.token;
                if reserved.is_empty() {
                    warnings.push(SpanDiag {
                        span: grant.span,
                        message: format!(
                            "{whose} grants '{token}', but the lexicon reserves no tokens, so \
                             the grant has no effect; reserve it with `reserved \"{token}\"` \
                             in the lexicon block, or remove the grant"
                        ),
                    });
                } else if !reserved.contains(token) {
                    ctx.diag(
                        grant.span,
                        format!(
                            "{whose} grants '{token}', which the lexicon does not reserve{}; \
                             reserve it in the lexicon block, or remove the grant",
                            suggest(token, reserved.iter().map(|r| r.as_str()))
                        ),
                    );
                }
            }
        }
    }
}
