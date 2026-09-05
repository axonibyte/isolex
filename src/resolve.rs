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
    pub params: Vec<(String, Num)>,
    pub axes: Vec<String>,
    pub grants: Vec<Grant>,
    pub parts: Vec<(String, Vec<Node>)>,
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
    pub axes: BTreeMap<String, AxisDef>,
    pub diags: Vec<SpanDiag>,
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
    let mut out = Resolved::default();

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
            _ => {}
        }
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
    r.describe.extend(body.describe.iter().cloned());
    for (name, value, _) in &body.params {
        if let Some(slot) = r.params.iter_mut().find(|(n, _)| n == name) {
            slot.1 = value.clone();
        } else {
            r.params.push((name.clone(), value.clone()));
        }
    }
    for a in &body.axes {
        if !r.axes.contains(a) {
            r.axes.push(a.clone());
        }
    }
    r.grants.extend(body.grants.iter().cloned());

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

/// Public entry the expander uses to overlay an axis variant.
pub fn apply_variant(r: &mut ResolvedDef, variant: &Variant, diags: &mut Vec<SpanDiag>) {
    let mut ctx = Ctx { diags: Vec::new() };
    merge_body(
        &mut ctx,
        r,
        &variant.body,
        &format!("variant {}", variant.name),
    );
    diags.append(&mut ctx.diags);
}
