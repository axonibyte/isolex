//! Expansion: axis products, parameter evaluation, repeat/when/insert
//! flattening. Output is a shape tree with every value evaluated except
//! those that reference an `at` anchor's screen coordinates (`x`, `y`),
//! which projection supplies; such values carry a captured environment
//! snapshot so scoping is decided here, once.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::ast::{Face, Node, Num, PathPiece, Span, Style};
use crate::expr::{self, Expr};
use crate::resolve::{apply_variant, Resolved, ResolvedDef, SpanDiag};

pub const REPEAT_CAP: usize = 10_000;
const INSERT_DEPTH_CAP: usize = 32;

/// A number that may still need the anchor's screen coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum EvNum {
    Lit(f64),
    NeedsAnchor {
        src: String,
        ast: Expr,
        env: BTreeMap<String, f64>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct EvStyle {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub width: Option<f64>,
    pub dash: Option<String>,
    pub opacity: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PathEv {
    Lit(String),
    Expr {
        src: String,
        ast: Expr,
        env: BTreeMap<String, f64>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Shape2 {
    Line {
        args: [EvNum; 4],
        style: EvStyle,
    },
    Circle {
        args: [EvNum; 3],
        style: EvStyle,
    },
    Ellipse {
        args: [EvNum; 4],
        style: EvStyle,
    },
    Rect {
        args: [EvNum; 4],
        rx: Option<EvNum>,
        style: EvStyle,
    },
    Arc {
        args: [EvNum; 5],
        style: EvStyle,
    },
    Path {
        template: Vec<PathEv>,
        style: EvStyle,
    },
    Poly {
        pts: Vec<[EvNum; 2]>,
        style: EvStyle,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Shape {
    Box3 {
        x: f64,
        y: f64,
        z: f64,
        w: f64,
        d: f64,
        h: f64,
        fills: Option<String>,
        opacity: Option<f64>,
    },
    Tile {
        x: f64,
        y: f64,
        z: f64,
        w: f64,
        d: f64,
        style: EvStyle,
    },
    Cylinder {
        cx: f64,
        cy: f64,
        z: f64,
        r: f64,
        h: f64,
        top: Option<String>,
        side: Option<String>,
        opacity: Option<f64>,
    },
    FaceRect {
        face: Face,
        a: f64,
        b: f64,
        z: f64,
        extent: f64,
        h: f64,
        fill: Option<String>,
        opacity: Option<f64>,
    },
    Poly3d {
        pts: Vec<[f64; 3]>,
        style: EvStyle,
    },
    Shadow {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
    },
    At {
        x: f64,
        y: f64,
        z: f64,
        children: Vec<Shape2>,
    },
    Badge {
        icon: String,
        corner: String,
        scale: Option<f64>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpandedIcon {
    pub name: String,
    pub category: String,
    pub footprint: Option<[f64; 3]>,
    pub canvas: Option<[f64; 2]>,
    pub describe: Vec<String>,
    pub grants: Vec<(String, String)>,
    pub shapes: Vec<Shape>,
}

struct Ctx<'a> {
    resolved: &'a Resolved,
    diags: Vec<SpanDiag>,
    insert_stack: Vec<String>,
}

impl<'a> Ctx<'a> {
    fn diag(&mut self, span: Span, message: impl Into<String>) {
        self.diags.push(SpanDiag {
            span,
            message: message.into(),
        });
    }
}

pub fn expand(resolved: &Resolved) -> (Vec<ExpandedIcon>, Vec<SpanDiag>) {
    let mut ctx = Ctx {
        resolved,
        diags: Vec::new(),
        insert_stack: Vec::new(),
    };
    let mut out = Vec::new();

    for icon in &resolved.icons {
        for combo in axis_combos(&mut ctx, icon) {
            let mut def = icon.clone();
            let mut suffix = String::new();
            for (axis_name, variant_idx) in &combo {
                if let Some(vi) = variant_idx {
                    let axis = &resolved.axes[axis_name];
                    let variant = &axis.variants[*vi];
                    suffix.push('-');
                    suffix.push_str(&variant.name);
                    apply_variant(&mut def, variant, &mut ctx.diags);
                }
            }
            let name = format!("{}{}", icon.name, suffix);
            if let Some(exp) = expand_one(&mut ctx, &def, name) {
                out.push(exp);
            }
        }
    }
    (out, ctx.diags)
}

/// The cartesian product of variant choices, in axis declaration order.
/// An axis with `emit-base #false` offers no "absent" option, so every
/// emitted combination carries one of its variants — which also excludes
/// the bare base.
fn axis_combos(ctx: &mut Ctx, icon: &ResolvedDef) -> Vec<Vec<(String, Option<usize>)>> {
    let mut combos: Vec<Vec<(String, Option<usize>)>> = vec![Vec::new()];
    for axis_name in &icon.axes {
        let Some(axis) = ctx.resolved.axes.get(axis_name) else {
            continue; // already diagnosed by the resolver
        };
        let mut options: Vec<Option<usize>> = Vec::new();
        if axis.emit_base {
            options.push(None);
        }
        options.extend((0..axis.variants.len()).map(Some));
        if axis.variants.is_empty() && !axis.emit_base {
            ctx.diag(
                icon.span,
                format!("axis '{axis_name}' has no variants and emit-base #false — nothing can be emitted"),
            );
        }
        combos = combos
            .into_iter()
            .flat_map(|c| {
                options.iter().map(move |o| {
                    let mut c2 = c.clone();
                    c2.push((axis_name.clone(), *o));
                    c2
                })
            })
            .collect();
    }
    combos
}

fn expand_one(ctx: &mut Ctx, def: &ResolvedDef, name: String) -> Option<ExpandedIcon> {
    // parameters evaluate in declaration order; earlier params are in
    // scope for later defaults
    let mut env: BTreeMap<String, f64> = BTreeMap::new();
    for (pname, pval) in &def.params {
        let v = eval_full(ctx, pval, &env, def.span)?;
        env.insert(pname.clone(), v);
    }

    let footprint = match &def.footprint {
        Some(fp) => {
            let mut out = [0.0; 3];
            for (i, v) in fp.iter().enumerate() {
                out[i] = eval_full(ctx, v, &env, def.span)?;
            }
            Some(out)
        }
        None => None,
    };
    let canvas = match &def.canvas {
        Some(cv) => {
            let mut out = [0.0; 2];
            for (i, v) in cv.iter().enumerate() {
                out[i] = eval_full(ctx, v, &env, def.span)?;
            }
            Some(out)
        }
        None => None,
    };

    let mut shapes = Vec::new();
    for (_pname, nodes) in &def.parts {
        expand_nodes_3d(ctx, nodes, &env, &mut shapes);
    }

    Some(ExpandedIcon {
        name,
        category: def.category.clone().unwrap_or_default(),
        footprint,
        canvas,
        describe: def.describe.clone(),
        grants: def
            .grants
            .iter()
            .map(|g| (g.token.clone(), g.reason.clone()))
            .collect(),
        shapes,
    })
}

// --- evaluation helpers --------------------------------------------------

fn eval_full(ctx: &mut Ctx, num: &Num, env: &BTreeMap<String, f64>, span: Span) -> Option<f64> {
    match num {
        Num::Lit(v) => Some(*v),
        Num::Expr { src, ast } => {
            let mut free = BTreeSet::new();
            expr::free_vars(ast, &mut free);
            for name in &free {
                if !env.contains_key(name) {
                    let scope: Vec<&str> = env.keys().map(|s| s.as_str()).collect();
                    ctx.diag(
                        span,
                        format!(
                            "in '{src}': unknown name '{name}' (in scope here: {})",
                            if scope.is_empty() {
                                "nothing".to_string()
                            } else {
                                scope.join(", ")
                            }
                        ),
                    );
                    return None;
                }
            }
            match expr::eval(ast, env) {
                Ok(v) => Some(v),
                Err(e) => {
                    ctx.diag(span, format!("in '{src}': {}", e.message));
                    None
                }
            }
        }
    }
}

/// Evaluate a 2-D value: full if possible, deferred if it needs the
/// anchor's `x`/`y`.
fn eval_2d(ctx: &mut Ctx, num: &Num, env: &BTreeMap<String, f64>, span: Span) -> Option<EvNum> {
    match num {
        Num::Lit(v) => Some(EvNum::Lit(*v)),
        Num::Expr { src, ast } => {
            let mut free = BTreeSet::new();
            expr::free_vars(ast, &mut free);
            let needs_anchor = free.iter().any(|n| n == "x" || n == "y");
            for name in &free {
                if name != "x" && name != "y" && !env.contains_key(name) {
                    let mut scope: Vec<&str> = env.keys().map(|s| s.as_str()).collect();
                    scope.push("x");
                    scope.push("y");
                    ctx.diag(
                        span,
                        format!(
                            "in '{src}': unknown name '{name}' (in scope here: {})",
                            scope.join(", ")
                        ),
                    );
                    return None;
                }
            }
            if needs_anchor {
                let captured: BTreeMap<String, f64> = free
                    .iter()
                    .filter(|n| n.as_str() != "x" && n.as_str() != "y")
                    .map(|n| (n.clone(), env[n]))
                    .collect();
                Some(EvNum::NeedsAnchor {
                    src: src.clone(),
                    ast: ast.clone(),
                    env: captured,
                })
            } else {
                match expr::eval(ast, env) {
                    Ok(v) => Some(EvNum::Lit(v)),
                    Err(e) => {
                        ctx.diag(span, format!("in '{src}': {}", e.message));
                        None
                    }
                }
            }
        }
    }
}

fn eval_style(
    ctx: &mut Ctx,
    style: &Style,
    env: &BTreeMap<String, f64>,
    span: Span,
) -> Option<EvStyle> {
    Some(EvStyle {
        fill: style.fill.clone(),
        stroke: style.stroke.clone(),
        width: match &style.width {
            Some(n) => Some(eval_full(ctx, n, env, span)?),
            None => None,
        },
        dash: style.dash.clone(),
        opacity: match &style.opacity {
            Some(n) => Some(eval_full(ctx, n, env, span)?),
            None => None,
        },
    })
}

fn when_passes(ctx: &mut Ctx, when: &Option<Num>, env: &BTreeMap<String, f64>, span: Span) -> bool {
    match when {
        None => true,
        Some(n) => match eval_full(ctx, n, env, span) {
            Some(v) => v != 0.0,
            None => false,
        },
    }
}

fn repeat_count(
    ctx: &mut Ctx,
    count: &Num,
    env: &BTreeMap<String, f64>,
    span: Span,
) -> Option<usize> {
    let v = eval_full(ctx, count, env, span)?;
    if v < 0.0 || (v - v.round()).abs() > 1e-9 {
        ctx.diag(
            span,
            format!("repeat count must be a non-negative integer, got {v}"),
        );
        return None;
    }
    let n = v.round() as usize;
    if n > REPEAT_CAP {
        ctx.diag(
            span,
            format!("repeat count {n} exceeds the cap of {REPEAT_CAP}"),
        );
        return None;
    }
    Some(n)
}

// --- 3-D expansion -------------------------------------------------------

fn expand_nodes_3d(
    ctx: &mut Ctx,
    nodes: &[Node],
    env: &BTreeMap<String, f64>,
    out: &mut Vec<Shape>,
) {
    for node in nodes {
        expand_node_3d(ctx, node, env, out);
    }
}

fn expand_node_3d(ctx: &mut Ctx, node: &Node, env: &BTreeMap<String, f64>, out: &mut Vec<Shape>) {
    macro_rules! evs {
        ($args:expr, $span:expr, $n:literal) => {{
            let mut vals = [0.0; $n];
            let mut ok = true;
            for (i, a) in $args.iter().enumerate() {
                match eval_full(ctx, a, env, $span) {
                    Some(v) => vals[i] = v,
                    None => ok = false,
                }
            }
            if !ok {
                return;
            }
            vals
        }};
    }

    match node {
        Node::Box3 {
            args,
            fills,
            opacity,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let v = evs!(args, *span, 6);
            let opacity = opacity.as_ref().and_then(|o| eval_full(ctx, o, env, *span));
            out.push(Shape::Box3 {
                x: v[0],
                y: v[1],
                z: v[2],
                w: v[3],
                d: v[4],
                h: v[5],
                fills: fills.clone(),
                opacity,
            });
        }
        Node::Tile {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let v = evs!(args, *span, 5);
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            out.push(Shape::Tile {
                x: v[0],
                y: v[1],
                z: v[2],
                w: v[3],
                d: v[4],
                style,
            });
        }
        Node::Cylinder {
            args,
            top,
            side,
            opacity,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let v = evs!(args, *span, 5);
            let opacity = opacity.as_ref().and_then(|o| eval_full(ctx, o, env, *span));
            out.push(Shape::Cylinder {
                cx: v[0],
                cy: v[1],
                z: v[2],
                r: v[3],
                h: v[4],
                top: top.clone(),
                side: side.clone(),
                opacity,
            });
        }
        Node::FaceRect {
            face,
            args,
            fill,
            opacity,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let v = evs!(args, *span, 5);
            let opacity = opacity.as_ref().and_then(|o| eval_full(ctx, o, env, *span));
            out.push(Shape::FaceRect {
                face: *face,
                a: v[0],
                b: v[1],
                z: v[2],
                extent: v[3],
                h: v[4],
                fill: fill.clone(),
                opacity,
            });
        }
        Node::Poly3d {
            style,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            let mut pts = Vec::new();
            collect_pts3(ctx, children, env, &mut pts);
            out.push(Shape::Poly3d { pts, style });
        }
        Node::Shadow { args, when, span } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let v = evs!(args, *span, 4);
            out.push(Shape::Shadow {
                cx: v[0],
                cy: v[1],
                rx: v[2],
                ry: v[3],
            });
        }
        Node::At {
            args,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let v = evs!(args, *span, 3);
            let mut kids = Vec::new();
            expand_nodes_2d(ctx, children, env, &mut kids);
            out.push(Shape::At {
                x: v[0],
                y: v[1],
                z: v[2],
                children: kids,
            });
        }
        Node::Repeat {
            count,
            var,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let Some(n) = repeat_count(ctx, count, env, *span) else {
                return;
            };
            for i in 0..n {
                let mut env2 = env.clone();
                env2.insert(var.clone(), i as f64);
                expand_nodes_3d(ctx, children, &env2, out);
            }
        }
        Node::Insert {
            fragment,
            args,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            expand_insert(ctx, fragment, args, env, *span, InsertCtx::ThreeD(out));
        }
        Node::Badge {
            icon,
            corner,
            scale,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            if !["ne", "nw", "se", "sw"].contains(&corner.as_str()) {
                ctx.diag(
                    *span,
                    format!("badge corner must be ne, nw, se or sw, got '{corner}'"),
                );
                return;
            }
            let scale = scale.as_ref().and_then(|s| eval_full(ctx, s, env, *span));
            out.push(Shape::Badge {
                icon: icon.clone(),
                corner: corner.clone(),
                scale,
            });
        }
        other => {
            // parser context validation should prevent this; defensive
            ctx.diag(
                node_span(other),
                "2-D node reached 3-D expansion (parser context bug or a fragment inserted in the wrong context)",
            );
        }
    }
}

fn collect_pts3(
    ctx: &mut Ctx,
    children: &[Node],
    env: &BTreeMap<String, f64>,
    out: &mut Vec<[f64; 3]>,
) {
    for ch in children {
        match ch {
            Node::Pt3 { args, span } => {
                let mut v = [0.0; 3];
                let mut ok = true;
                for (i, a) in args.iter().enumerate() {
                    match eval_full(ctx, a, env, *span) {
                        Some(val) => v[i] = val,
                        None => ok = false,
                    }
                }
                if ok {
                    out.push(v);
                }
            }
            Node::Repeat {
                count,
                var,
                children,
                when,
                span,
            } => {
                if !when_passes(ctx, when, env, *span) {
                    continue;
                }
                let Some(n) = repeat_count(ctx, count, env, *span) else {
                    continue;
                };
                for i in 0..n {
                    let mut env2 = env.clone();
                    env2.insert(var.clone(), i as f64);
                    collect_pts3(ctx, children, &env2, out);
                }
            }
            other => ctx.diag(node_span(other), "only pt and repeat belong inside poly3d"),
        }
    }
}

// --- 2-D expansion -------------------------------------------------------

fn expand_nodes_2d(
    ctx: &mut Ctx,
    nodes: &[Node],
    env: &BTreeMap<String, f64>,
    out: &mut Vec<Shape2>,
) {
    for node in nodes {
        expand_node_2d(ctx, node, env, out);
    }
}

fn expand_node_2d(ctx: &mut Ctx, node: &Node, env: &BTreeMap<String, f64>, out: &mut Vec<Shape2>) {
    macro_rules! evs2 {
        ($args:expr, $span:expr, $n:literal) => {{
            let mut vals: [EvNum; $n] = core::array::from_fn(|_| EvNum::Lit(0.0));
            let mut ok = true;
            for (i, a) in $args.iter().enumerate() {
                match eval_2d(ctx, a, env, $span) {
                    Some(v) => vals[i] = v,
                    None => ok = false,
                }
            }
            if !ok {
                return;
            }
            vals
        }};
    }

    match node {
        Node::Line {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let a = evs2!(args, *span, 4);
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            out.push(Shape2::Line { args: a, style });
        }
        Node::Circle {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let a = evs2!(args, *span, 3);
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            out.push(Shape2::Circle { args: a, style });
        }
        Node::Ellipse {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let a = evs2!(args, *span, 4);
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            out.push(Shape2::Ellipse { args: a, style });
        }
        Node::Rect2d {
            args,
            rx,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let a = evs2!(args, *span, 4);
            let rx = match rx {
                Some(r) => match eval_2d(ctx, r, env, *span) {
                    Some(v) => Some(v),
                    None => return,
                },
                None => None,
            };
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            out.push(Shape2::Rect { args: a, rx, style });
        }
        Node::Arc {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let a = evs2!(args, *span, 5);
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            out.push(Shape2::Arc { args: a, style });
        }
        Node::Path {
            template,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let mut pieces = Vec::new();
            for piece in template {
                match piece {
                    PathPiece::Lit(s) => pieces.push(PathEv::Lit(s.clone())),
                    PathPiece::Expr { src, ast } => {
                        let num = Num::Expr {
                            src: src.clone(),
                            ast: ast.clone(),
                        };
                        match eval_2d(ctx, &num, env, *span) {
                            Some(EvNum::Lit(v)) => pieces.push(PathEv::Lit(fmt_num(v))),
                            Some(EvNum::NeedsAnchor { src, ast, env }) => {
                                pieces.push(PathEv::Expr { src, ast, env })
                            }
                            None => return,
                        }
                    }
                }
            }
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            out.push(Shape2::Path {
                template: pieces,
                style,
            });
        }
        Node::Poly2d {
            style,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let Some(style) = eval_style(ctx, style, env, *span) else {
                return;
            };
            let mut pts = Vec::new();
            collect_pts2(ctx, children, env, &mut pts);
            out.push(Shape2::Poly { pts, style });
        }
        Node::Repeat {
            count,
            var,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            let Some(n) = repeat_count(ctx, count, env, *span) else {
                return;
            };
            for i in 0..n {
                let mut env2 = env.clone();
                env2.insert(var.clone(), i as f64);
                expand_nodes_2d(ctx, children, &env2, out);
            }
        }
        Node::Insert {
            fragment,
            args,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return;
            }
            expand_insert(ctx, fragment, args, env, *span, InsertCtx::TwoD(out));
        }
        other => ctx.diag(
            node_span(other),
            "3-D node reached 2-D expansion (parser context bug or a fragment inserted in the wrong context)",
        ),
    }
}

fn collect_pts2(
    ctx: &mut Ctx,
    children: &[Node],
    env: &BTreeMap<String, f64>,
    out: &mut Vec<[EvNum; 2]>,
) {
    for ch in children {
        match ch {
            Node::Pt2 { args, span } => {
                let a = eval_2d(ctx, &args[0], env, *span);
                let b = eval_2d(ctx, &args[1], env, *span);
                if let (Some(a), Some(b)) = (a, b) {
                    out.push([a, b]);
                }
            }
            Node::Repeat {
                count,
                var,
                children,
                when,
                span,
            } => {
                if !when_passes(ctx, when, env, *span) {
                    continue;
                }
                let Some(n) = repeat_count(ctx, count, env, *span) else {
                    continue;
                };
                for i in 0..n {
                    let mut env2 = env.clone();
                    env2.insert(var.clone(), i as f64);
                    collect_pts2(ctx, children, &env2, out);
                }
            }
            other => ctx.diag(node_span(other), "only pt and repeat belong inside poly"),
        }
    }
}

// --- fragments -----------------------------------------------------------

enum InsertCtx<'v, 'o> {
    ThreeD(&'v mut Vec<Shape>),
    TwoD(&'o mut Vec<Shape2>),
}

fn expand_insert(
    ctx: &mut Ctx,
    fragment: &str,
    args: &[(String, Num)],
    env: &BTreeMap<String, f64>,
    span: Span,
    target: InsertCtx,
) {
    let Some(frag) = ctx.resolved.fragments.get(fragment).cloned() else {
        let names: Vec<String> = ctx.resolved.fragments.keys().cloned().collect();
        ctx.diag(
            span,
            format!(
                "unknown fragment '{fragment}'{}",
                crate::resolve::suggest(fragment, names.iter().map(|s| s.as_str()))
            ),
        );
        return;
    };
    if ctx.insert_stack.iter().any(|f| f == fragment) {
        let mut chain = ctx.insert_stack.clone();
        chain.push(fragment.to_string());
        ctx.diag(
            span,
            format!("fragment insertion cycle: {}", chain.join(" -> ")),
        );
        return;
    }
    if ctx.insert_stack.len() >= INSERT_DEPTH_CAP {
        ctx.diag(
            span,
            format!("fragment insertion deeper than {INSERT_DEPTH_CAP}"),
        );
        return;
    }

    // hygienic environment: fragment params only, defaults evaluated in
    // fragment scope, overridden by caller args evaluated in caller scope
    let mut frag_env: BTreeMap<String, f64> = BTreeMap::new();
    for (pname, pval, pspan) in &frag.body.params {
        if let Some(v) = eval_full(ctx, pval, &frag_env, *pspan) {
            frag_env.insert(pname.clone(), v);
        }
    }
    for (aname, aval) in args {
        if !frag_env.contains_key(aname) {
            let known: Vec<&str> = frag_env.keys().map(|s| s.as_str()).collect();
            ctx.diag(
                span,
                format!(
                    "fragment '{fragment}' has no parameter '{aname}'{}",
                    crate::resolve::suggest(aname, known.into_iter())
                ),
            );
            return;
        }
        if let Some(v) = eval_full(ctx, aval, env, span) {
            frag_env.insert(aname.clone(), v);
        }
    }

    ctx.insert_stack.push(fragment.to_string());
    match target {
        InsertCtx::ThreeD(out) => {
            for part in &frag.body.parts {
                expand_nodes_3d(ctx, &part.nodes, &frag_env, out);
            }
        }
        InsertCtx::TwoD(out) => {
            for part in &frag.body.parts {
                expand_nodes_2d(ctx, &part.nodes, &frag_env, out);
            }
        }
    }
    ctx.insert_stack.pop();
}

fn node_span(node: &Node) -> Span {
    match node {
        Node::Box3 { span, .. }
        | Node::Tile { span, .. }
        | Node::Cylinder { span, .. }
        | Node::FaceRect { span, .. }
        | Node::Poly3d { span, .. }
        | Node::Shadow { span, .. }
        | Node::At { span, .. }
        | Node::Line { span, .. }
        | Node::Circle { span, .. }
        | Node::Ellipse { span, .. }
        | Node::Rect2d { span, .. }
        | Node::Arc { span, .. }
        | Node::Path { span, .. }
        | Node::Poly2d { span, .. }
        | Node::Pt3 { span, .. }
        | Node::Pt2 { span, .. }
        | Node::Repeat { span, .. }
        | Node::Insert { span, .. }
        | Node::Badge { span, .. } => *span,
    }
}

/// The normative numeric formatting rule: fixed two decimal places,
/// exactly as the shape IR emits coordinates. Byte parity with any other
/// conforming implementation depends on this being the only rule.
pub fn fmt_num(v: f64) -> String {
    format!("{v:.2}")
}
