//! Projection: expanded shapes → shape IR. The isometric constants,
//! face construction order, and primitive defaults here are normative
//! language semantics (SPEC.md §4/§7) — a second implementation must
//! reproduce them to the byte.

use std::collections::{BTreeMap, BTreeSet};

use crate::affine::{self, Extent, Mat, IDENTITY};
use crate::ast::{Bounds, Face, LexiconConfig, MirrorAxis, Span, Strokes};
use crate::expand::{EvNum, EvStep, EvStyle, EvTransform, ExpandedIcon, PathEv, Shape, Shape2};
use crate::expr;
use crate::ir::{format_number as fmt, IrDocument, IrIcon, IrShape, IR_VERSION};
use crate::resolve::{suggest, SpanDiag};

/// One tile unit along an isometric axis, in output units.
pub const S: f64 = 40.0;
/// cos(30°), pinned as a literal so no libm variance can leak into
/// output bytes across platforms.
pub const COS: f64 = 0.866_025_403_784_438_7;
pub const SIN: f64 = 0.5;
/// The pack-wide outline weight default.
pub const OUTLINE_W: f64 = 1.2;
/// viewBox padding around the footprint volume.
pub const PAD: f64 = 8.0;
/// Default badge scale when none is given.
pub const BADGE_SCALE: f64 = 0.5;
/// Badge inset from the viewBox corner.
pub const BADGE_INSET: f64 = 2.0;

/// The projection: x runs right-down, y left-down, z up.
pub fn p(x: f64, y: f64, z: f64) -> (f64, f64) {
    ((x - y) * COS * S, (x + y) * SIN * S - z * S)
}

fn style_or(
    style: &EvStyle,
    fill: &str,
    stroke: &str,
    width: f64,
) -> (String, f64, String, f64, Option<String>) {
    (
        style.fill.clone().unwrap_or_else(|| fill.to_string()),
        style.opacity.unwrap_or(1.0),
        style.stroke.clone().unwrap_or_else(|| stroke.to_string()),
        style.width.unwrap_or(width),
        style.dash.clone(),
    )
}

struct Ctx<'v> {
    diags: Vec<SpanDiag>,
    vocabulary: &'v Vocabulary,
    /// The icon being projected, for errors.
    icon: String,
    /// The tokens it grants, through its definition chain.
    granted: Vec<String>,
    /// The transforms met in it so far, and those open around what is
    /// being projected now, innermost last.
    occs: Vec<Occ>,
    open: Vec<usize>,
    /// Each shape projected so far, the transform it stands in.
    tags: Vec<Option<usize>>,
}

impl Ctx<'_> {
    /// The shapes projected since the last call stand in the innermost
    /// transform open now.
    fn tag_up_to(&mut self, len: usize) {
        let occ = self.open.last().copied();
        while self.tags.len() < len {
            self.tags.push(occ);
        }
    }
}

/// A transform met in an icon: its map as written, the transform around
/// it, and what it does past the icon's frame and with strokes.
#[derive(Debug, Clone)]
struct Occ {
    parent: Option<usize>,
    local: Mat,
    bounds: Option<Bounds>,
    strokes: Option<Strokes>,
    /// A text's fill, for the glyphs drawn in it.
    paint: Option<String>,
    span: Span,
}

/// What a lexicon block declares the lexicon paints with (SPEC §2): its
/// tokens and stroke widths, each checked only when declared.
#[derive(Debug, Clone, Default)]
pub struct Vocabulary {
    tokens: Option<Vec<String>>,
    /// As formatted for output (SPEC §7), the precision a renderer sees.
    widths: Option<Vec<String>>,
    /// Tokens painted only under a grant.
    reserved: Vec<String>,
}

impl Vocabulary {
    pub fn of(config: Option<&LexiconConfig>) -> Vocabulary {
        let Some(config) = config else {
            return Vocabulary::default();
        };
        Vocabulary {
            tokens: (!config.tokens.is_empty()).then(|| config.tokens.clone()),
            widths: (!config.stroke_widths.is_empty())
                .then(|| config.stroke_widths.iter().map(|w| fmt(*w)).collect()),
            reserved: config.reserved.clone(),
        }
    }
}

/// Hold a node's projected shapes to the declared vocabulary. This runs
/// on the IR, after projection has filled in every default, because a
/// program default appears in the IR exactly as if the author had written
/// it and is held to the same declarations (SPEC §2). `none` is no paint,
/// not a token, and a width is only checked where there is a stroke.
fn check_vocabulary(ctx: &mut Ctx, shapes: &[IrShape], span: Span) {
    let mut tokens: Vec<&str> = Vec::new();
    let mut widths: Vec<String> = Vec::new();
    for shape in shapes {
        let (fill, stroke, width) = match shape {
            IrShape::Polygon {
                fill,
                stroke,
                stroke_width,
                ..
            }
            | IrShape::Path {
                fill,
                stroke,
                stroke_width,
                ..
            }
            | IrShape::Ellipse {
                fill,
                stroke,
                stroke_width,
                ..
            }
            | IrShape::Circle {
                fill,
                stroke,
                stroke_width,
                ..
            }
            | IrShape::Rect {
                fill,
                stroke,
                stroke_width,
                ..
            } => (fill, stroke, *stroke_width),
            // a badge's shapes are its target's, held to this when it was
            // projected as an icon of its own; a transform's shapes are
            // checked at their nodes before they are grouped
            IrShape::Group { .. } => continue,
        };
        for token in [fill.as_str(), stroke.as_str()] {
            if token != "none" && !tokens.contains(&token) {
                tokens.push(token);
            }
        }
        let width = fmt(width);
        if stroke != "none" && !widths.contains(&width) {
            widths.push(width);
        }
    }

    let icon = ctx.icon.clone();
    // a reserved token, defaulted or written, needs a grant in the icon's
    // definition chain; a badge's shapes are its target's, granted there
    for token in &tokens {
        if ctx.vocabulary.reserved.iter().any(|r| r == token)
            && !ctx.granted.iter().any(|g| g == token)
        {
            let message = format!(
                "icon '{icon}' paints with '{token}', which the lexicon reserves; grant it in \
                 this icon's definition chain with `grant \"{token}\" reason=\"…\"`, or paint \
                 this node with another token"
            );
            report_once(ctx, span, message);
        }
    }
    if let Some(declared) = &ctx.vocabulary.tokens {
        for token in tokens {
            if !declared.iter().any(|d| d == token) {
                let message = format!(
                    "icon '{icon}' paints with '{token}', which is not one of the lexicon's \
                     tokens{}; declare it in the lexicon block, or paint this node with one \
                     that is",
                    suggest(token, declared.iter().map(|d| d.as_str()))
                );
                report_once(ctx, span, message);
            }
        }
    }
    if let Some(declared) = &ctx.vocabulary.widths {
        for width in widths {
            if !declared.contains(&width) {
                let message = format!(
                    "icon '{icon}' strokes at width {width}, which is not one of the lexicon's \
                     stroke-widths ({}); declare it in the lexicon block, or use one that is",
                    declared.join(", ")
                );
                report_once(ctx, span, message);
            }
        }
    }
}

/// A badge waiting for every icon to be projected: `at` indexes the empty
/// group standing in for it in its icon's shapes.
struct PendingBadge {
    at: usize,
    target: String,
    corner: String,
    scale: f64,
    span: Span,
}

/// An icon projected, before its badges are filled and its transforms
/// held to its frame: its shapes in one flat list, each with the
/// transform it stands in.
struct Draft {
    ir: IrIcon,
    tags: Vec<Option<usize>>,
    occs: Vec<Occ>,
    badges: Vec<PendingBadge>,
    default_bounds: Option<Bounds>,
}

pub fn project(icons: &[ExpandedIcon], vocabulary: &Vocabulary) -> (IrDocument, Vec<SpanDiag>) {
    let mut ctx = Ctx {
        diags: Vec::new(),
        vocabulary,
        icon: String::new(),
        granted: Vec::new(),
        occs: Vec::new(),
        open: Vec::new(),
        tags: Vec::new(),
    };

    // First pass: project every icon, with badges left as empty groups.
    let drafts: Vec<Draft> = icons
        .iter()
        .map(|icon| project_icon(&mut ctx, icon))
        .collect();

    // Second pass: finish the icons, those with no badge first, since a
    // badge draws one of them finished (an icon used as a badge must
    // itself contain no badge: one level, so no cycles are possible).
    let badged: BTreeSet<String> = drafts
        .iter()
        .filter(|d| !d.badges.is_empty())
        .map(|d| d.ir.name.clone())
        .collect();
    let mut finished: Vec<Option<IrIcon>> = vec![None; drafts.len()];
    let none = BTreeMap::new();
    for (i, draft) in drafts.iter().enumerate() {
        if draft.badges.is_empty() {
            finished[i] = Some(finish(&mut ctx, draft, &none, &badged));
        }
    }
    let targets: BTreeMap<String, IrIcon> = finished
        .iter()
        .flatten()
        .map(|i| (i.name.clone(), i.clone()))
        .collect();
    for (i, draft) in drafts.iter().enumerate() {
        if !draft.badges.is_empty() {
            finished[i] = Some(finish(&mut ctx, draft, &targets, &badged));
        }
    }

    (
        IrDocument {
            ir_version: IR_VERSION,
            icons: finished
                .into_iter()
                .map(|i| i.expect("every icon finished"))
                .collect(),
        },
        ctx.diags,
    )
}

/// Finite inputs can still overflow once projected to output units, and
/// JSON would write the result as null (SPEC §7).
fn too_large(icon: &str, v: f64) -> String {
    format!("icon '{icon}' projects to {v}, not a finite number: its geometry is too large to draw")
}

fn non_finite(vs: &[f64]) -> Option<f64> {
    vs.iter().copied().find(|v| !v.is_finite())
}

/// The first value in projected shapes that is not finite, if any. Path
/// data holds numbers already formatted, where Rust writes them as
/// `inf`, `-inf` and `NaN`: none of their letters is an SVG path
/// command, so finding them there is unambiguous.
fn first_non_finite(shapes: &[IrShape]) -> Option<f64> {
    fn in_d(d: &str) -> Option<f64> {
        if d.contains("NaN") {
            Some(f64::NAN)
        } else if d.contains("-inf") {
            Some(f64::NEG_INFINITY)
        } else if d.contains("inf") {
            Some(f64::INFINITY)
        } else {
            None
        }
    }
    shapes.iter().find_map(|s| match s {
        IrShape::Polygon {
            points,
            fill_opacity,
            stroke_width,
            ..
        } => non_finite(&points.concat()).or_else(|| non_finite(&[*fill_opacity, *stroke_width])),
        IrShape::Path {
            d,
            fill_opacity,
            stroke_width,
            ..
        } => in_d(d).or_else(|| non_finite(&[*fill_opacity, *stroke_width])),
        IrShape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            fill_opacity,
            stroke_width,
            ..
        } => non_finite(&[*cx, *cy, *rx, *ry, *fill_opacity, *stroke_width]),
        IrShape::Circle {
            cx,
            cy,
            r,
            fill_opacity,
            stroke_width,
            ..
        } => non_finite(&[*cx, *cy, *r, *fill_opacity, *stroke_width]),
        IrShape::Rect {
            x,
            y,
            w,
            h,
            rx,
            fill_opacity,
            stroke_width,
            ..
        } => non_finite(&[*x, *y, *w, *h, *rx, *fill_opacity, *stroke_width]),
        IrShape::Group {
            matrix,
            clip,
            shapes,
            ..
        } => non_finite(matrix)
            .or_else(|| clip.as_ref().and_then(|c| non_finite(&c.concat())))
            .or_else(|| first_non_finite(shapes)),
    })
}

/// What a shape is drawn with once its icon is finished: the transform it
/// stands in, and within that, a badge's map, the clip and fixed strokes
/// its target's group gave it, if it came from one.
struct Item {
    occ: Option<usize>,
    /// Which run within that transform: 0 for its own shapes, and one
    /// number for each badge, and each group of a badge's target.
    run: usize,
    pre: Mat,
    /// In the coordinates `pre` maps into.
    clip: Option<Vec<[f64; 2]>>,
    fixed: bool,
    shape: IrShape,
}

/// The map a transform's steps make, each but translate and matrix about
/// its origin, applied in order, and laid on a face last.
fn matrix_of(ctx: &mut Ctx, tf: &EvTransform, env: &BTreeMap<String, f64>, span: Span) -> Mat {
    let o = [ev(ctx, &tf.origin[0], env), ev(ctx, &tf.origin[1], env)];
    let mut chain = IDENTITY;
    for step in &tf.steps {
        let op = match step {
            EvStep::Translate([x, y]) => affine::translate(ev(ctx, x, env), ev(ctx, y, env)),
            EvStep::Rotate(a) => affine::about(o, &affine::rotate(ev(ctx, a, env))),
            EvStep::Scale([x, y]) => {
                affine::about(o, &affine::scale(ev(ctx, x, env), ev(ctx, y, env)))
            }
            EvStep::SkewX(a) => affine::about(o, &affine::skew_x(ev(ctx, a, env))),
            EvStep::SkewY(a) => affine::about(o, &affine::skew_y(ev(ctx, a, env))),
            EvStep::Mirror(MirrorAxis::Across) => affine::about(o, &affine::scale(-1.0, 1.0)),
            EvStep::Mirror(MirrorAxis::Down) => affine::about(o, &affine::scale(1.0, -1.0)),
            EvStep::Matrix(m) => {
                let mut out = [0.0; 6];
                for (i, v) in m.iter().enumerate() {
                    out[i] = ev(ctx, v, env);
                }
                out
            }
        };
        chain = affine::mul(&op, &chain);
    }
    if let Some(face) = &tf.onto {
        chain = affine::mul(face, &chain);
    }
    if let Some(v) = non_finite(&chain) {
        let icon = ctx.icon.clone();
        report_once(
            ctx,
            span,
            format!(
                "icon '{icon}' transforms by a matrix that is not finite ({v}): a skew of 90°, \
                 or a map too large to draw"
            ),
        );
    }
    chain
}

/// A transform's map with every transform around it: the matrix its
/// shapes are drawn with.
fn full_matrix(occs: &[Occ], occ: usize) -> Mat {
    let mut m = occs[occ].local;
    let mut at = occs[occ].parent;
    while let Some(p) = at {
        m = affine::mul(&occs[p].local, &m);
        at = occs[p].parent;
    }
    m
}

fn root(occs: &[Occ], mut occ: usize) -> usize {
    while let Some(p) = occs[occ].parent {
        occ = p;
    }
    occ
}

/// Strokes keep their width under the nearest transform that says so.
fn fixed_strokes(occs: &[Occ], occ: Option<usize>) -> bool {
    let mut at = occ;
    while let Some(o) = at {
        if let Some(s) = occs[o].strokes {
            return s == Strokes::Fixed;
        }
        at = occs[o].parent;
    }
    false
}

/// A uniform scale about the middle of what is drawn, and the least shift
/// that brings it inside the frame.
fn fit(e: &Extent, frame: &Extent) -> Mat {
    let size = |x: &Extent, k: usize| x.max[k] - x.min[k];
    let mut k: f64 = 1.0;
    for axis in 0..2 {
        if size(e, axis) > 0.0 {
            k = k.min(size(frame, axis) / size(e, axis));
        }
    }
    let c = [(e.min[0] + e.max[0]) / 2.0, (e.min[1] + e.max[1]) / 2.0];
    let mut shift = [0.0; 2];
    for axis in 0..2 {
        let half = size(e, axis) * k / 2.0;
        let (lo, hi) = (c[axis] - half, c[axis] + half);
        if lo < frame.min[axis] {
            shift[axis] = frame.min[axis] - lo;
        } else if hi > frame.max[axis] {
            shift[axis] = frame.max[axis] - hi;
        }
    }
    affine::mul(
        &affine::translate(shift[0], shift[1]),
        &affine::about(c, &affine::scale(k, k)),
    )
}

fn describe_extent(e: &Extent) -> String {
    format!(
        "{} {} to {} {}",
        fmt(e.min[0]),
        fmt(e.min[1]),
        fmt(e.max[0]),
        fmt(e.max[1])
    )
}

/// An icon finished: its badges filled from the finished `targets`, its
/// transforms held to its frame, and its shapes grouped, one group for
/// each run of shapes drawn with the same matrix, clip and strokes, never
/// nested (SPEC §7).
fn finish(
    ctx: &mut Ctx,
    draft: &Draft,
    targets: &BTreeMap<String, IrIcon>,
    badged: &BTreeSet<String>,
) -> IrIcon {
    let icon = draft.ir.name.clone();
    ctx.icon = icon.clone();
    let frame = Extent::of_viewbox(draft.ir.viewbox);

    // every shape with what it is drawn through, badges opened up
    let mut items: Vec<Item> = Vec::new();
    let mut runs = 0;
    let mut badges = draft.badges.iter().peekable();
    for (i, shape) in draft.ir.shapes.iter().enumerate() {
        let occ = draft.tags[i];
        let Some(badge) = badges.next_if(|b| b.at == i) else {
            items.push(Item {
                occ,
                run: 0,
                pre: IDENTITY,
                clip: None,
                fixed: false,
                shape: shape.clone(),
            });
            continue;
        };
        let name = &badge.target;
        let Some(target) = targets.get(name) else {
            let message = if badged.contains(name) {
                format!("badge target '{name}' itself contains a badge — one level only")
            } else {
                format!("badge references unknown icon '{name}'")
            };
            ctx.diags.push(SpanDiag {
                span: badge.span,
                message,
            });
            continue;
        };
        // at a corner of the icon's frame, as it is before any transform
        // grows it
        let s = badge.scale;
        let [minx, miny, w, h] = draft.ir.viewbox;
        let [bminx, bminy, bw, bh] = target.viewbox;
        let (px, py) = match badge.corner.as_str() {
            "nw" => (minx + BADGE_INSET, miny + BADGE_INSET),
            "se" => (
                minx + w - BADGE_INSET - bw * s,
                miny + h - BADGE_INSET - bh * s,
            ),
            "sw" => (minx + BADGE_INSET, miny + h - BADGE_INSET - bh * s),
            _ => (minx + w - BADGE_INSET - bw * s, miny + BADGE_INSET), // ne
        };
        let b = [s, 0.0, 0.0, s, px - bminx * s, py - bminy * s];
        if let Some(v) = non_finite(&b) {
            ctx.diags.push(SpanDiag {
                span: badge.span,
                message: too_large(&icon, v),
            });
            continue;
        }
        runs += 1;
        for t in &target.shapes {
            match t {
                IrShape::Group {
                    matrix,
                    clip,
                    strokes,
                    shapes,
                } => {
                    let clip: Option<Vec<[f64; 2]>> = clip
                        .as_ref()
                        .map(|c| c.iter().map(|p| affine::apply(&b, *p)).collect());
                    runs += 1;
                    for inner in shapes {
                        items.push(Item {
                            occ,
                            run: runs,
                            pre: affine::mul(&b, matrix),
                            clip: clip.clone(),
                            fixed: strokes.as_deref() == Some("fixed"),
                            shape: inner.clone(),
                        });
                    }
                }
                plain => items.push(Item {
                    occ,
                    run: runs,
                    pre: b,
                    clip: None,
                    fixed: false,
                    shape: plain.clone(),
                }),
            }
        }
    }

    // each outermost transform held to the frame, with everything in it
    let mut occs = draft.occs.clone();
    let mut grown = frame;
    let mut root_clip: BTreeMap<usize, Vec<[f64; 2]>> = BTreeMap::new();
    for r in 0..occs.len() {
        if occs[r].parent.is_some() {
            continue;
        }
        let mut reach: Option<Extent> = None;
        for item in &items {
            let Some(occ) = item.occ.filter(|o| root(&occs, *o) == r) else {
                continue;
            };
            let outer = full_matrix(&occs, occ);
            let m = affine::mul(&outer, &item.pre);
            let e = match affine::shape_extent(&item.shape, &m) {
                Ok(e) => e,
                Err(why) => {
                    report_once(
                        ctx,
                        occs[r].span,
                        format!("icon '{icon}' transforms path data it cannot read, so it cannot tell how far it reaches: {why}"),
                    );
                    None
                }
            };
            let e = match (e, &item.clip) {
                (Some(e), Some(clip)) => {
                    let mut cut: Option<Extent> = None;
                    for p in clip {
                        let q = affine::apply(&outer, *p);
                        match &mut cut {
                            Some(c) => c.add(q),
                            None => cut = Some(Extent::of_point(q)),
                        }
                    }
                    cut.and_then(|c| e.intersect(&c))
                }
                (e, _) => e,
            };
            if let Some(e) = e {
                reach = Some(match reach {
                    Some(r) => r.union(&e),
                    None => e,
                });
            }
        }
        let Some(reach) = reach else { continue };
        if !reach.is_finite() || reach.within(&frame) {
            continue;
        }
        match occs[r]
            .bounds
            .or(draft.default_bounds)
            .unwrap_or(Bounds::Error)
        {
            Bounds::Error => report_once(
                ctx,
                occs[r].span,
                format!(
                    "icon '{icon}' draws this transform out to {}, outside its frame, {}; \
                     set bounds=\"grow\", \"scale\" or \"clip\" on the transform, or \
                     transform-bounds on the icon, to say what to do",
                    describe_extent(&reach),
                    describe_extent(&frame)
                ),
            ),
            Bounds::Grow => grown = grown.union(&reach),
            Bounds::Scale => {
                let s = fit(&reach, &frame);
                occs[r].local = affine::mul(&s, &occs[r].local);
            }
            Bounds::Clip => {
                root_clip.insert(r, frame.corners());
            }
        }
    }

    // grouped, one group for each run: the shapes of one transform, or of
    // one badge, or one group of its target, side by side
    let mut shapes: Vec<IrShape> = Vec::new();
    let mut run_key: Option<(Option<usize>, usize)> = None;
    for item in items {
        let outer = item.occ.map(|o| full_matrix(&occs, o)).unwrap_or(IDENTITY);
        // and never -0.0, which JSON would write as such
        let matrix = affine::mul(&outer, &item.pre).map(|v| v + 0.0);
        let mut clip: Option<Vec<[f64; 2]>> = item
            .occ
            .and_then(|o| root_clip.get(&root(&occs, o)).cloned());
        if let Some(own) = &item.clip {
            let own: Vec<[f64; 2]> = own.iter().map(|p| affine::apply(&outer, *p)).collect();
            clip = Some(match clip {
                Some(c) => affine::clip_convex(&own, &c),
                None => own,
            });
        }
        if clip.as_ref().is_some_and(|c| c.len() < 3) {
            continue; // clipped away entirely
        }
        let fixed = item.fixed || fixed_strokes(&occs, item.occ);
        if let Some(v) =
            non_finite(&matrix).or_else(|| clip.as_ref().and_then(|c| non_finite(&c.concat())))
        {
            if let Some(o) = item.occ {
                report_once(ctx, occs[o].span, too_large(&icon, v));
            }
            continue;
        }
        if affine::is_identity(&matrix) && clip.is_none() && !fixed {
            shapes.push(item.shape);
            run_key = None;
            continue;
        }
        let key = (item.occ, item.run);
        if run_key == Some(key) {
            if let Some(IrShape::Group { shapes: inner, .. }) = shapes.last_mut() {
                inner.push(item.shape);
                continue;
            }
        }
        shapes.push(IrShape::Group {
            matrix,
            clip,
            strokes: fixed.then(|| "fixed".to_string()),
            shapes: vec![item.shape],
        });
        run_key = Some(key);
    }

    IrIcon {
        viewbox: grown.viewbox(),
        shapes,
        ..draft.ir.clone()
    }
}

fn project_icon(ctx: &mut Ctx, icon: &ExpandedIcon) -> Draft {
    ctx.icon = icon.name.clone();
    ctx.granted = icon.grants.iter().map(|(token, _)| token.clone()).collect();
    ctx.occs.clear();
    ctx.open.clear();
    ctx.tags.clear();
    let viewbox = match (&icon.footprint, &icon.canvas) {
        (Some([w, d, h]), _) => volume_viewbox(*w, *d, *h),
        (_, Some([w, h])) => [0.0, 0.0, *w, *h],
        _ => [0.0, 0.0, 0.0, 0.0],
    };
    if let Some(v) = non_finite(&viewbox) {
        ctx.diags.push(SpanDiag {
            span: icon.span,
            message: too_large(&icon.name, v),
        });
    }
    let mut shapes = Vec::new();
    let mut badges = Vec::new();
    let no_anchor = BTreeMap::new();
    for shape in &icon.shapes {
        match shape {
            Shape::Badge {
                icon: target,
                corner,
                scale,
                span,
            } => {
                badges.push(PendingBadge {
                    at: shapes.len(),
                    target: target.clone(),
                    corner: corner.clone(),
                    scale: scale.unwrap_or(BADGE_SCALE),
                    span: *span,
                });
                shapes.push(IrShape::Group {
                    matrix: IDENTITY,
                    clip: None,
                    strokes: None,
                    shapes: Vec::new(),
                });
                ctx.tag_up_to(shapes.len());
                continue;
            }
            Shape::TransformStart { tf, span } => {
                open_transform(ctx, tf, &no_anchor, *span);
                continue;
            }
            Shape::TransformEnd => {
                ctx.open.pop();
                continue;
            }
            _ => {}
        }
        let before = shapes.len();
        project_shape(ctx, shape, &mut shapes);
        ctx.tag_up_to(shapes.len());
        if let Some(v) = first_non_finite(&shapes[before..]) {
            ctx.diags.push(SpanDiag {
                span: shape_span(shape),
                message: too_large(&icon.name, v),
            });
        }
        // an anchor's 2-D children are each checked at their own node
        if !matches!(shape, Shape::At { .. }) {
            check_vocabulary(ctx, &shapes[before..], shape_span(shape));
        }
    }
    let ir = IrIcon {
        name: icon.name.clone(),
        category: icon.category.clone(),
        footprint: icon.footprint,
        canvas: icon.canvas,
        viewbox,
        describe: icon.describe.clone(),
        grants: icon.grants.clone(),
        shapes,
    };
    Draft {
        ir,
        tags: std::mem::take(&mut ctx.tags),
        occs: std::mem::take(&mut ctx.occs),
        badges,
        default_bounds: icon.transform_bounds,
    }
}

/// A transform begins: its map, inside whatever transform is open.
fn open_transform(ctx: &mut Ctx, tf: &EvTransform, env: &BTreeMap<String, f64>, span: Span) {
    let local = matrix_of(ctx, tf, env, span);
    ctx.occs.push(Occ {
        parent: ctx.open.last().copied(),
        local,
        bounds: tf.bounds,
        strokes: tf.strokes,
        paint: tf.paint.clone(),
        span,
    });
    ctx.open.push(ctx.occs.len() - 1);
}

/// The fill of the nearest text around what is projected now, if any.
fn text_paint(ctx: &Ctx) -> Option<String> {
    let mut at = ctx.open.last().copied();
    while let Some(o) = at {
        if let Some(p) = &ctx.occs[o].paint {
            return Some(p.clone());
        }
        at = ctx.occs[o].parent;
    }
    None
}

/// A glyph's shape in its text's fill: every paint but none, written or
/// defaulted, is drawn in it (SPEC §4), and held to the vocabulary so.
fn repaint(shape: &mut IrShape, paint: &str) {
    match shape {
        IrShape::Polygon { fill, stroke, .. }
        | IrShape::Path { fill, stroke, .. }
        | IrShape::Ellipse { fill, stroke, .. }
        | IrShape::Circle { fill, stroke, .. }
        | IrShape::Rect { fill, stroke, .. } => {
            for p in [fill, stroke] {
                if p != "none" {
                    *p = paint.to_string();
                }
            }
        }
        IrShape::Group { .. } => {}
    }
}

fn shape_span(shape: &Shape) -> Span {
    match shape {
        Shape::Box3 { span, .. }
        | Shape::Tile { span, .. }
        | Shape::Cylinder { span, .. }
        | Shape::FaceRect { span, .. }
        | Shape::Poly3d { span, .. }
        | Shape::Shadow { span, .. }
        | Shape::Seg { span, .. }
        | Shape::At { span, .. }
        | Shape::Badge { span, .. }
        | Shape::TransformStart { span, .. } => *span,
        Shape::TransformEnd => unreachable!("an end is never projected as a shape"),
    }
}

/// A node expanded more than once (in a repeat, or a fragment inserted
/// twice) would otherwise report the same thing at the same place again.
fn report_once(ctx: &mut Ctx, span: Span, message: String) {
    let diag = SpanDiag { span, message };
    if !ctx.diags.contains(&diag) {
        ctx.diags.push(diag);
    }
}

fn shape2_span(shape: &Shape2) -> Span {
    match shape {
        Shape2::Line { span, .. }
        | Shape2::Circle { span, .. }
        | Shape2::Ellipse { span, .. }
        | Shape2::Rect { span, .. }
        | Shape2::Arc { span, .. }
        | Shape2::Path { span, .. }
        | Shape2::Poly { span, .. }
        | Shape2::TransformStart { span, .. } => *span,
        Shape2::TransformEnd => unreachable!("an end is never projected as a shape"),
    }
}

/// The viewBox of the (w, d, h) footprint volume with PAD padding —
/// byte-compatible with the consumer's historical computation.
pub fn volume_viewbox(fw: f64, fd: f64, fh: f64) -> [f64; 4] {
    let corners = [
        (0.0, 0.0, 0.0),
        (fw, 0.0, 0.0),
        (0.0, fd, 0.0),
        (fw, fd, 0.0),
        (0.0, 0.0, fh),
        (fw, 0.0, fh),
        (0.0, fd, fh),
        (fw, fd, fh),
    ];
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (x, y, z) in corners {
        let (px, py) = p(x, y, z);
        xs.push(px);
        ys.push(py);
    }
    let minx = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let miny = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let maxx = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let maxy = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    [
        minx - PAD,
        miny - PAD,
        maxx - minx + 2.0 * PAD,
        maxy - miny + 2.0 * PAD,
    ]
}

fn polygon(
    points: Vec<[f64; 2]>,
    fill: &str,
    opacity: f64,
    stroke: &str,
    width: f64,
    dash: Option<String>,
) -> IrShape {
    IrShape::Polygon {
        points,
        fill: fill.to_string(),
        fill_opacity: opacity,
        stroke: stroke.to_string(),
        stroke_width: width,
        dash,
    }
}

fn project_shape(ctx: &mut Ctx, shape: &Shape, out: &mut Vec<IrShape>) {
    match shape {
        Shape::Box3 {
            x,
            y,
            z,
            w,
            d,
            h,
            fills,
            opacity,
            ..
        } => {
            let (ft, fr, fl) = match fills.as_deref() {
                Some("virt") => ("virt", "virt", "virt"),
                Some("none") => ("none", "none", "none"),
                _ => ("body-top", "body-right", "body-left"),
            };
            let op = opacity.unwrap_or(1.0);
            let top = vec![
                pt(*x, *y, z + h),
                pt(x + w, *y, z + h),
                pt(x + w, y + d, z + h),
                pt(*x, y + d, z + h),
            ];
            let right = vec![
                pt(x + w, *y, *z),
                pt(x + w, y + d, *z),
                pt(x + w, y + d, z + h),
                pt(x + w, *y, z + h),
            ];
            let left = vec![
                pt(*x, y + d, *z),
                pt(x + w, y + d, *z),
                pt(x + w, y + d, z + h),
                pt(*x, y + d, z + h),
            ];
            out.push(polygon(left, fl, op, "outline", OUTLINE_W, None));
            out.push(polygon(right, fr, op, "outline", OUTLINE_W, None));
            out.push(polygon(top, ft, op, "outline", OUTLINE_W, None));
        }
        Shape::Tile {
            x,
            y,
            z,
            w,
            d,
            style,
            ..
        } => {
            let (fill, op, stroke, width, dash) = style_or(style, "body-top", "outline", OUTLINE_W);
            let pts = vec![
                pt(*x, *y, *z),
                pt(x + w, *y, *z),
                pt(x + w, y + d, *z),
                pt(*x, y + d, *z),
            ];
            out.push(polygon(pts, &fill, op, &stroke, width, dash));
        }
        Shape::Cylinder {
            cx,
            cy,
            z,
            r,
            h,
            top,
            side,
            opacity,
            ..
        } => {
            let (tcx, tcy) = p(*cx, *cy, z + h);
            let (bcx, bcy) = p(*cx, *cy, *z);
            let rx = std::f64::consts::SQRT_2 * r * COS * S;
            let ry = std::f64::consts::SQRT_2 * r * SIN * S;
            let body_d = format!(
                "M {},{} L {},{} A {} {} 0 0 0 {},{} L {},{} Z",
                fmt(tcx - rx),
                fmt(tcy),
                fmt(bcx - rx),
                fmt(bcy),
                fmt(rx),
                fmt(ry),
                fmt(bcx + rx),
                fmt(bcy),
                fmt(tcx + rx),
                fmt(tcy)
            );
            let op = opacity.unwrap_or(1.0);
            out.push(IrShape::Path {
                d: body_d,
                fill: side.clone().unwrap_or_else(|| "body-right".to_string()),
                fill_opacity: op,
                stroke: "outline".to_string(),
                stroke_width: OUTLINE_W,
                dash: None,
            });
            out.push(IrShape::Ellipse {
                cx: tcx,
                cy: tcy,
                rx,
                ry,
                fill: top.clone().unwrap_or_else(|| "body-top".to_string()),
                fill_opacity: op,
                stroke: "outline".to_string(),
                stroke_width: OUTLINE_W,
                dash: None,
            });
        }
        Shape::FaceRect {
            face,
            a,
            b,
            z,
            extent,
            h,
            fill,
            opacity,
            ..
        } => {
            let pts = match face {
                Face::Front => vec![
                    pt(*a, *b, *z),
                    pt(a + extent, *b, *z),
                    pt(a + extent, *b, z + h),
                    pt(*a, *b, z + h),
                ],
                Face::Right => vec![
                    pt(*a, *b, *z),
                    pt(*a, b + extent, *z),
                    pt(*a, b + extent, z + h),
                    pt(*a, *b, z + h),
                ],
            };
            out.push(polygon(
                pts,
                fill.as_deref().unwrap_or("ink"),
                opacity.unwrap_or(1.0),
                "none",
                0.0,
                None,
            ));
        }
        Shape::Poly3d {
            pts: pts3, style, ..
        } => {
            let (fill, op, stroke, width, dash) = style_or(style, "body-top", "outline", OUTLINE_W);
            let pts = pts3.iter().map(|[x, y, z]| pt(*x, *y, *z)).collect();
            out.push(polygon(pts, &fill, op, &stroke, width, dash));
        }
        Shape::Shadow { cx, cy, rx, ry, .. } => {
            let (px, py) = p(*cx, *cy, 0.0);
            out.push(IrShape::Ellipse {
                cx: px,
                cy: py,
                rx: rx * COS * S,
                ry: ry * SIN * S,
                fill: "shadow".to_string(),
                fill_opacity: 0.35,
                stroke: "none".to_string(),
                stroke_width: 0.0,
                dash: None,
            });
        }
        Shape::Seg { a, b, style, .. } => {
            let (x0, y0) = p(a[0], a[1], a[2]);
            let (x1, y1) = p(b[0], b[1], b[2]);
            let (fill, op, stroke, width, dash) = style_or(style, "none", "ink", 1.4);
            out.push(IrShape::Path {
                d: format!("M{},{} L{},{}", fmt(x0), fmt(y0), fmt(x1), fmt(y1)),
                fill,
                fill_opacity: op,
                stroke,
                stroke_width: width,
                dash,
            });
        }
        Shape::At {
            x, y, z, children, ..
        } => {
            let (sx, sy) = p(*x, *y, *z);
            let mut env = BTreeMap::new();
            env.insert("x".to_string(), sx);
            env.insert("y".to_string(), sy);
            for child in children {
                match child {
                    Shape2::TransformStart { tf, span } => {
                        ctx.tag_up_to(out.len());
                        open_transform(ctx, tf, &env, *span);
                        continue;
                    }
                    Shape2::TransformEnd => {
                        ctx.tag_up_to(out.len());
                        ctx.open.pop();
                        continue;
                    }
                    _ => {}
                }
                let before = out.len();
                project_2d(ctx, child, &env, out);
                if let Some(paint) = text_paint(ctx) {
                    for shape in &mut out[before..] {
                        repaint(shape, &paint);
                    }
                }
                ctx.tag_up_to(out.len());
                check_vocabulary(ctx, &out[before..], shape2_span(child));
            }
        }
        Shape::Badge { .. } | Shape::TransformStart { .. } | Shape::TransformEnd => {
            unreachable!("project_icon handles badges and transforms itself")
        }
    }
}

fn pt(x: f64, y: f64, z: f64) -> [f64; 2] {
    let (px, py) = p(x, y, z);
    [px, py]
}

fn ev(ctx: &mut Ctx, num: &EvNum, env: &BTreeMap<String, f64>) -> f64 {
    match num {
        EvNum::Lit(v) => *v,
        EvNum::NeedsAnchor {
            src,
            ast,
            env: captured,
            span,
        } => {
            let mut full = captured.clone();
            full.extend(env.iter().map(|(k, v)| (k.clone(), *v)));
            match expr::eval(ast, &full) {
                Ok(v) => v,
                Err(e) => {
                    ctx.diags.push(SpanDiag {
                        span: *span,
                        message: format!("in '{src}': {}", e.message),
                    });
                    0.0
                }
            }
        }
    }
}

fn project_2d(ctx: &mut Ctx, shape: &Shape2, env: &BTreeMap<String, f64>, out: &mut Vec<IrShape>) {
    match shape {
        Shape2::Line { args, style, .. } => {
            let (x1, y1, x2, y2) = (
                ev(ctx, &args[0], env),
                ev(ctx, &args[1], env),
                ev(ctx, &args[2], env),
                ev(ctx, &args[3], env),
            );
            let (fill, op, stroke, width, dash) = style_or(style, "none", "ink", 1.4);
            out.push(IrShape::Path {
                d: format!("M{},{} L{},{}", fmt(x1), fmt(y1), fmt(x2), fmt(y2)),
                fill,
                fill_opacity: op,
                stroke,
                stroke_width: width,
                dash,
            });
        }
        Shape2::Circle { args, style, .. } => {
            let (fill, op, stroke, width, dash) = style_or(style, "none", "outline", OUTLINE_W);
            out.push(IrShape::Circle {
                cx: ev(ctx, &args[0], env),
                cy: ev(ctx, &args[1], env),
                r: ev(ctx, &args[2], env),
                fill,
                fill_opacity: op,
                stroke,
                stroke_width: width,
                dash,
            });
        }
        Shape2::Ellipse { args, style, .. } => {
            let (fill, op, stroke, width, dash) = style_or(style, "none", "outline", OUTLINE_W);
            out.push(IrShape::Ellipse {
                cx: ev(ctx, &args[0], env),
                cy: ev(ctx, &args[1], env),
                rx: ev(ctx, &args[2], env),
                ry: ev(ctx, &args[3], env),
                fill,
                fill_opacity: op,
                stroke,
                stroke_width: width,
                dash,
            });
        }
        Shape2::Rect {
            args, rx, style, ..
        } => {
            let (fill, op, stroke, width, dash) = style_or(style, "none", "none", 1.4);
            out.push(IrShape::Rect {
                x: ev(ctx, &args[0], env),
                y: ev(ctx, &args[1], env),
                w: ev(ctx, &args[2], env),
                h: ev(ctx, &args[3], env),
                rx: rx.as_ref().map(|r| ev(ctx, r, env)).unwrap_or(0.0),
                fill,
                fill_opacity: op,
                stroke,
                stroke_width: width,
                dash,
            });
        }
        Shape2::Arc { args, style, .. } => {
            let cx = ev(ctx, &args[0], env);
            let cy = ev(ctx, &args[1], env);
            let r = ev(ctx, &args[2], env);
            let a0 = ev(ctx, &args[3], env);
            let a1 = ev(ctx, &args[4], env);
            let (x0, y0) = (
                cx + r * crate::expr::quantize(crate::expr::cos(a0.to_radians())),
                cy - r * crate::expr::quantize(crate::expr::sin(a0.to_radians())),
            );
            let (x1, y1) = (
                cx + r * crate::expr::quantize(crate::expr::cos(a1.to_radians())),
                cy - r * crate::expr::quantize(crate::expr::sin(a1.to_radians())),
            );
            let large = if (a1 - a0).abs() > 180.0 { 1 } else { 0 };
            let (fill, op, stroke, width, dash) = style_or(style, "none", "ink", 1.4);
            out.push(IrShape::Path {
                d: format!(
                    "M{},{} A{} {} 0 {} 0 {},{}",
                    fmt(x0),
                    fmt(y0),
                    fmt(r),
                    fmt(r),
                    large,
                    fmt(x1),
                    fmt(y1)
                ),
                fill,
                fill_opacity: op,
                stroke,
                stroke_width: width,
                dash,
            });
        }
        Shape2::Path {
            template, style, ..
        } => {
            let mut d = String::new();
            for piece in template {
                match piece {
                    PathEv::Lit(s) => d.push_str(s),
                    PathEv::Expr {
                        src,
                        ast,
                        env: captured,
                        span,
                    } => {
                        let mut full = captured.clone();
                        full.extend(env.iter().map(|(k, v)| (k.clone(), *v)));
                        match expr::eval(ast, &full) {
                            Ok(v) => d.push_str(&fmt(v)),
                            Err(e) => {
                                ctx.diags.push(SpanDiag {
                                    span: *span,
                                    message: format!("in '{src}': {}", e.message),
                                });
                            }
                        }
                    }
                }
            }
            let (fill, op, stroke, width, dash) = style_or(style, "none", "ink", 1.4);
            out.push(IrShape::Path {
                d,
                fill,
                fill_opacity: op,
                stroke,
                stroke_width: width,
                dash,
            });
        }
        Shape2::Poly { pts, style, .. } => {
            let (fill, op, stroke, width, dash) = style_or(style, "ink", "none", 0.0);
            let points = pts
                .iter()
                .map(|[a, b]| [ev(ctx, a, env), ev(ctx, b, env)])
                .collect();
            out.push(polygon(points, &fill, op, &stroke, width, dash));
        }
        Shape2::TransformStart { .. } | Shape2::TransformEnd => {
            unreachable!("an anchor's transforms are opened as its children are projected")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_constants_match_the_reference_values() {
        // the same values Python's math produces for the pack today
        let (x, y) = p(1.0, 1.0, 0.0);
        assert_eq!(x, 0.0);
        assert_eq!(y, 40.0);
        let (x, y) = p(2.0, 1.0, 0.28);
        assert!((x - 34.641_016_151_377_55).abs() < 1e-12);
        assert!((y - 48.8).abs() < 1e-12);
        assert_eq!(COS, crate::expr::cos(30.0_f64.to_radians()));
    }

    #[test]
    fn volume_viewbox_matches_the_reference_shape() {
        // footprint (2,1,0.28): matches the consumer's historical numbers
        // reference values from the consumer's Python implementation:
        // volume_viewbox(2, 1, 0.28) ==
        //   (-42.64101615137755, -19.200000000000003, 119.92304845413264, 87.2)
        let [minx, miny, w, h] = volume_viewbox(2.0, 1.0, 0.28);
        assert_eq!(minx, -42.641_016_151_377_55);
        assert_eq!(miny, -19.200_000_000_000_003);
        assert_eq!(w, 119.923_048_454_132_64);
        assert_eq!(h, 87.2);
    }
}
