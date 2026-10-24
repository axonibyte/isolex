//! Projection: expanded shapes → shape IR. The isometric constants,
//! face construction order, and primitive defaults here are normative
//! language semantics (SPEC.md §4/§7) — a second implementation must
//! reproduce them to the byte.

use std::collections::BTreeMap;

use crate::ast::{Face, Span};
use crate::expand::{EvNum, EvStyle, ExpandedIcon, PathEv, Shape, Shape2};
use crate::expr;
use crate::ir::{IrDocument, IrIcon, IrShape, IR_VERSION};
use crate::resolve::SpanDiag;

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

fn fmt(v: f64) -> String {
    format!("{v:.2}")
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

struct Ctx {
    diags: Vec<SpanDiag>,
}

/// A badge waiting for every icon to be projected: `at` indexes the empty
/// group standing in for it in its icon's shapes.
struct PendingBadge {
    at: usize,
    target: String,
    corner: String,
    span: Span,
}

pub fn project(icons: &[ExpandedIcon]) -> (IrDocument, Vec<SpanDiag>) {
    let mut ctx = Ctx { diags: Vec::new() };

    // First pass: project every icon, with badges left as empty groups.
    let (mut projected, pending): (Vec<IrIcon>, Vec<Vec<PendingBadge>>) = icons
        .iter()
        .map(|icon| project_icon(&mut ctx, icon))
        .unzip();

    // Second pass: fill the groups from the projected set (an icon used
    // as a badge must itself contain no badge — one level, so no cycles
    // are possible).
    let by_name: BTreeMap<String, IrIcon> = projected
        .iter()
        .map(|i| (i.name.clone(), i.clone()))
        .collect();
    for (icon, badges) in projected.iter_mut().zip(&pending) {
        let viewbox = icon.viewbox;
        for badge in badges {
            let group = &mut icon.shapes[badge.at];
            resolve_badge(&mut ctx.diags, &by_name, badge, viewbox, group);
            if let IrShape::Group { tx, ty, .. } = group {
                if let Some(v) = non_finite(&[*tx, *ty]) {
                    ctx.diags.push(SpanDiag {
                        span: badge.span,
                        message: too_large(&icon.name, v),
                    });
                }
            }
        }
    }

    (
        IrDocument {
            ir_version: IR_VERSION,
            icons: projected,
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
            tx,
            ty,
            scale,
            shapes,
        } => non_finite(&[*tx, *ty, *scale]).or_else(|| first_non_finite(shapes)),
    })
}

fn resolve_badge(
    diags: &mut Vec<SpanDiag>,
    by_name: &BTreeMap<String, IrIcon>,
    badge: &PendingBadge,
    viewbox: [f64; 4],
    group: &mut IrShape,
) {
    let IrShape::Group {
        tx,
        ty,
        scale,
        shapes,
    } = group
    else {
        unreachable!("a pending badge indexes its placeholder group");
    };
    let s = *scale;
    let name = &badge.target;
    let Some(target) = by_name.get(name) else {
        diags.push(SpanDiag {
            span: badge.span,
            message: format!("badge references unknown icon '{name}'"),
        });
        return;
    };
    if target
        .shapes
        .iter()
        .any(|s| matches!(s, IrShape::Group { .. }))
    {
        diags.push(SpanDiag {
            span: badge.span,
            message: format!("badge target '{name}' itself contains a badge — one level only"),
        });
        return;
    }
    let [minx, miny, w, h] = viewbox;
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
    *tx = px - bminx * s;
    *ty = py - bminy * s;
    *shapes = target.shapes.clone();
}

fn project_icon(ctx: &mut Ctx, icon: &ExpandedIcon) -> (IrIcon, Vec<PendingBadge>) {
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
    for shape in &icon.shapes {
        if let Shape::Badge {
            icon: target,
            corner,
            scale,
            span,
        } = shape
        {
            badges.push(PendingBadge {
                at: shapes.len(),
                target: target.clone(),
                corner: corner.clone(),
                span: *span,
            });
            shapes.push(IrShape::Group {
                tx: 0.0,
                ty: 0.0,
                scale: scale.unwrap_or(BADGE_SCALE),
                shapes: Vec::new(),
            });
            continue;
        }
        let before = shapes.len();
        project_shape(ctx, shape, &mut shapes);
        if let Some(v) = first_non_finite(&shapes[before..]) {
            ctx.diags.push(SpanDiag {
                span: shape_span(shape),
                message: too_large(&icon.name, v),
            });
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
    (ir, badges)
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
        | Shape::Badge { span, .. } => *span,
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
                project_2d(ctx, child, &env, out);
            }
        }
        Shape::Badge { .. } => unreachable!("project_icon places badges itself"),
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
        Shape2::Line { args, style } => {
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
        Shape2::Circle { args, style } => {
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
        Shape2::Ellipse { args, style } => {
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
        Shape2::Rect { args, rx, style } => {
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
        Shape2::Arc { args, style } => {
            let cx = ev(ctx, &args[0], env);
            let cy = ev(ctx, &args[1], env);
            let r = ev(ctx, &args[2], env);
            let a0 = ev(ctx, &args[3], env);
            let a1 = ev(ctx, &args[4], env);
            let (x0, y0) = (
                cx + r * crate::expr::quantize(a0.to_radians().cos()),
                cy - r * crate::expr::quantize(a0.to_radians().sin()),
            );
            let (x1, y1) = (
                cx + r * crate::expr::quantize(a1.to_radians().cos()),
                cy - r * crate::expr::quantize(a1.to_radians().sin()),
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
        Shape2::Path { template, style } => {
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
        Shape2::Poly { pts, style } => {
            let (fill, op, stroke, width, dash) = style_or(style, "ink", "none", 0.0);
            let points = pts
                .iter()
                .map(|[a, b]| [ev(ctx, a, env), ev(ctx, b, env)])
                .collect();
            out.push(polygon(points, &fill, op, &stroke, width, dash));
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
        assert_eq!(COS, 30.0_f64.to_radians().cos());
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
