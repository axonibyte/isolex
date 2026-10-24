//! Expansion: axis products, parameter evaluation, repeat/when/insert
//! flattening. Output is a shape tree with every value evaluated except
//! those that reference an `at` anchor's screen coordinates (`x`, `y`),
//! which projection supplies; such values carry a captured environment
//! snapshot so scoping is decided here, once.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use serde::Serialize;

use crate::ast::{Face, Limit, LimitKind, Limits, Node, Num, PartDecl, PathPiece, Span, Style};
use crate::expr::{self, Expr};
use crate::resolve::{apply_variant, Resolved, ResolvedDef, SpanDiag};

/// A limit in force (SPEC §9): its ceiling, `None` when switched off,
/// and where that came from, for the error that names it.
#[derive(Debug, Clone)]
struct InForce {
    limit: Option<u64>,
    from: String,
}

/// The definition chain's value wins, then the lexicon block's, then the
/// built-in default.
fn in_force(
    kind: LimitKind,
    chain: &BTreeMap<LimitKind, (Limit, String)>,
    lexicon: &Limits,
) -> InForce {
    let limit = |l: &Limit| match l {
        Limit::At(n) => Some(*n),
        Limit::Unlimited => None,
    };
    if let Some((l, whose)) = chain.get(&kind) {
        InForce {
            limit: limit(l),
            from: format!("set by '{whose}'"),
        }
    } else if let Some(l) = lexicon.get(&kind) {
        InForce {
            limit: limit(l),
            from: "set in the lexicon block".to_string(),
        }
    } else {
        InForce {
            limit: Some(kind.default_limit()),
            from: "the default".to_string(),
        }
    }
}

/// The error for going past a limit: what happened, the limit and where
/// it came from, and how to raise it or switch it off.
fn past_limit(what: String, kind: LimitKind, limit: u64, from: &str) -> String {
    let d = kind.directive();
    let place = if kind.lexicon_wide() {
        "in the lexicon block"
    } else {
        "in this icon's definition or the lexicon block"
    };
    format!(
        "{what} past its {d} of {limit} ({from}); if that is intended, raise it {place}, \
         or set `{d} #null` for no limit"
    )
}

/// A number that may still need the anchor's screen coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum EvNum {
    Lit(f64),
    NeedsAnchor {
        src: String,
        ast: Expr,
        env: BTreeMap<String, f64>,
        /// The node it belongs to, for errors only projection can find.
        span: Span,
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
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Shape2 {
    Line {
        args: [EvNum; 4],
        style: EvStyle,
        span: Span,
    },
    Circle {
        args: [EvNum; 3],
        style: EvStyle,
        span: Span,
    },
    Ellipse {
        args: [EvNum; 4],
        style: EvStyle,
        span: Span,
    },
    Rect {
        args: [EvNum; 4],
        rx: Option<EvNum>,
        style: EvStyle,
        span: Span,
    },
    Arc {
        args: [EvNum; 5],
        style: EvStyle,
        span: Span,
    },
    Path {
        template: Vec<PathEv>,
        style: EvStyle,
        span: Span,
    },
    Poly {
        pts: Vec<[EvNum; 2]>,
        style: EvStyle,
        span: Span,
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
        span: Span,
    },
    Tile {
        x: f64,
        y: f64,
        z: f64,
        w: f64,
        d: f64,
        style: EvStyle,
        span: Span,
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
        span: Span,
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
        span: Span,
    },
    Poly3d {
        pts: Vec<[f64; 3]>,
        style: EvStyle,
        span: Span,
    },
    Shadow {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        span: Span,
    },
    Seg {
        a: [f64; 3],
        b: [f64; 3],
        style: EvStyle,
        span: Span,
    },
    At {
        x: f64,
        y: f64,
        z: f64,
        children: Vec<Shape2>,
        span: Span,
    },
    Badge {
        icon: String,
        corner: String,
        scale: Option<f64>,
        span: Span,
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
    /// The icon's definition, for errors projection finds.
    pub span: Span,
}

struct Ctx<'a> {
    resolved: &'a Resolved,
    diags: Vec<SpanDiag>,
    insert_stack: Vec<String>,
    /// The icon being expanded, and its limits and use of them.
    icon: String,
    shapes: (InForce, u64),
    repeats: (InForce, u64),
    insert_depth: InForce,
    expression_depth: InForce,
    /// A limit was passed: expansion of this icon stops everywhere.
    exhausted: bool,
}

impl<'a> Ctx<'a> {
    fn diag(&mut self, span: Span, message: impl Into<String>) {
        self.diags.push(SpanDiag {
            span,
            message: message.into(),
        });
    }

    /// Start an icon: its limits, nothing used.
    fn begin(&mut self, name: &str, def: &ResolvedDef) {
        let lexicon = &self.resolved.lexicon_limits;
        self.icon = name.to_string();
        self.shapes = (in_force(LimitKind::Shapes, &def.limits, lexicon), 0);
        self.repeats = (in_force(LimitKind::Repeats, &def.limits, lexicon), 0);
        self.insert_depth = in_force(LimitKind::InsertDepth, &def.limits, lexicon);
        self.expression_depth = in_force(LimitKind::ExpressionDepth, &def.limits, lexicon);
        self.exhausted = false;
    }

    /// Count one shape or point (`Shapes`) or one repeat iteration
    /// (`Repeats`) at `span`. False once a limit is passed: the caller
    /// stops, and so does everything else in this icon.
    fn spend(&mut self, kind: LimitKind, span: Span) -> bool {
        if self.exhausted {
            return false;
        }
        let ((in_force, used), what) = match kind {
            LimitKind::Shapes => (&mut self.shapes, "shapes and points"),
            LimitKind::Repeats => (&mut self.repeats, "repeat iterations"),
            _ => unreachable!("only shapes and repeats are spent"),
        };
        *used += 1;
        match in_force.limit {
            Some(limit) if *used > limit => {
                let message = past_limit(
                    format!("icon '{}' goes here", self.icon),
                    kind,
                    limit,
                    &format!("{what}, {}", in_force.from),
                );
                self.exhausted = true;
                self.diag(span, message);
                false
            }
            _ => true,
        }
    }
}

pub fn expand(resolved: &Resolved) -> (Vec<ExpandedIcon>, Vec<SpanDiag>) {
    let unset = InForce {
        limit: None,
        from: String::new(),
    };
    let mut ctx = Ctx {
        resolved,
        diags: Vec::new(),
        insert_stack: Vec::new(),
        icon: String::new(),
        shapes: (unset.clone(), 0),
        repeats: (unset.clone(), 0),
        insert_depth: unset.clone(),
        expression_depth: unset,
        exhausted: false,
    };
    let mut out = Vec::new();

    // Axes multiply the way nested repeats do, so the icon count is
    // checked by multiplying their sizes, before any product is built.
    let icon_limit = in_force(LimitKind::Icons, &BTreeMap::new(), &resolved.lexicon_limits);
    let mut icon_count: u128 = 0;
    let mut within_icon_limit = Vec::new();
    for icon in &resolved.icons {
        icon_count = icon_count.saturating_add(axis_product(&ctx, icon));
        match icon_limit.limit {
            Some(limit) if icon_count > u128::from(limit) => {
                let message = past_limit(
                    format!("'{}' takes the lexicon", icon.name),
                    LimitKind::Icons,
                    limit,
                    &format!("icons, {}", icon_limit.from),
                );
                ctx.diag(icon.span, message);
                break;
            }
            _ => within_icon_limit.push(icon),
        }
    }
    // Each emitted name, with what emitted it: an icon and the variants
    // applied to it. Names are decided here, not by evaluation, so a
    // collision is one whether or not the icons expand cleanly.
    let mut emitted: BTreeMap<String, (String, Vec<(String, String)>)> = BTreeMap::new();

    for icon in within_icon_limit {
        for combo in axis_combos(&mut ctx, icon) {
            let mut def = icon.clone();
            let mut suffix = String::new();
            let mut applied = Vec::new();
            for (axis_name, variant_idx) in &combo {
                if let Some(vi) = variant_idx {
                    let axis = &resolved.axes[axis_name];
                    let variant = &axis.variants[*vi];
                    suffix.push('-');
                    suffix.push_str(&variant.name);
                    applied.push((axis_name.clone(), variant.name.clone()));
                    apply_variant(&mut def, variant, &mut ctx.diags);
                }
            }
            let name = format!("{}{}", icon.name, suffix);
            let source = (icon.name.clone(), applied);
            match emitted.get(&name) {
                // the same definition twice: the resolver has already
                // reported it as a duplicate
                Some(first) if *first == source => {}
                Some(first) => {
                    let message = format!(
                        "'{name}' is emitted twice: by {}, and by {}",
                        describe_source(first),
                        describe_source(&source)
                    );
                    ctx.diag(icon.span, message);
                }
                None => {
                    emitted.insert(name.clone(), source);
                }
            }
            if let Some(exp) = expand_one(&mut ctx, &def, name) {
                out.push(exp);
            }
        }
    }
    (out, ctx.diags)
}

/// "icon 'srv' with variant 'down' of axis 'state'", or just "icon 'srv'".
fn describe_source((icon, applied): &(String, Vec<(String, String)>)) -> String {
    let mut s = format!("icon '{icon}'");
    for (i, (axis, variant)) in applied.iter().enumerate() {
        s.push_str(if i == 0 { " with " } else { " and " });
        s.push_str(&format!("variant '{variant}' of axis '{axis}'"));
    }
    s
}

/// How many icons an icon's axes expand into, without building them.
fn axis_product(ctx: &Ctx, icon: &ResolvedDef) -> u128 {
    icon.axes
        .iter()
        .filter_map(|a| ctx.resolved.axes.get(a))
        .map(|axis| axis.variants.len() as u128 + u128::from(axis.emit_base))
        .fold(1, u128::saturating_mul)
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
    ctx.begin(&name, def);
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
    let env: Env = Rc::new(env);
    for (_pname, nodes) in &def.parts {
        run(ctx, nodes, env.clone(), &mut shapes);
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
        span: def.span,
    })
}

// --- evaluation helpers --------------------------------------------------

/// An expression nesting past the icon's expression-depth (SPEC §9) is an
/// error where it is used. Evaluating one is a loop at any depth, so this
/// is a default against mistakes, not a guard against a crash.
fn within_expression_depth(ctx: &mut Ctx, ast: &Expr, span: Span) -> bool {
    let depth = ast.depth();
    match ctx.expression_depth.limit {
        Some(limit) if depth as u64 > limit => {
            let message = past_limit(
                format!("icon '{}' nests an expression {depth} deep here", ctx.icon),
                LimitKind::ExpressionDepth,
                limit,
                &format!("levels, {}", ctx.expression_depth.from),
            );
            ctx.diag(span, message);
            false
        }
        _ => true,
    }
}

fn eval_full(ctx: &mut Ctx, num: &Num, env: &BTreeMap<String, f64>, span: Span) -> Option<f64> {
    match num {
        Num::Lit(v) => Some(*v),
        Num::Expr { src, ast } => {
            if !within_expression_depth(ctx, ast, span) {
                return None;
            }
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
            if !within_expression_depth(ctx, ast, span) {
                return None;
            }
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
                    span,
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
    // No cap of its own: every iteration counts toward the icon's
    // repeat-limit, so a huge count stops there, at this repeat.
    Some(v.round() as usize)
}

// --- 3-D expansion -------------------------------------------------------

/// One node into 3-D shapes. A node with children is not expanded here:
/// what it opens comes back, for [`run`] to expand without recursion.
fn expand_node_3d<'a: 'n, 'n>(
    ctx: &mut Ctx<'a>,
    node: &'n Node,
    env: &BTreeMap<String, f64>,
    out: &mut Vec<Shape>,
) -> Option<Open<'n>> {
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
                return None;
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
                return None;
            }
            let v = evs!(args, *span, 6);
            let opacity = opacity.as_ref().and_then(|o| eval_full(ctx, o, env, *span));
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape::Box3 {
                x: v[0],
                y: v[1],
                z: v[2],
                w: v[3],
                d: v[4],
                h: v[5],
                fills: fills.clone(),
                opacity,
                span: *span,
            });
        }
        Node::Tile {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let v = evs!(args, *span, 5);
            let style = eval_style(ctx, style, env, *span)?;
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape::Tile {
                x: v[0],
                y: v[1],
                z: v[2],
                w: v[3],
                d: v[4],
                style,
                span: *span,
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
                return None;
            }
            let v = evs!(args, *span, 5);
            let opacity = opacity.as_ref().and_then(|o| eval_full(ctx, o, env, *span));
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape::Cylinder {
                cx: v[0],
                cy: v[1],
                z: v[2],
                r: v[3],
                h: v[4],
                top: top.clone(),
                side: side.clone(),
                opacity,
                span: *span,
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
                return None;
            }
            let v = evs!(args, *span, 5);
            let opacity = opacity.as_ref().and_then(|o| eval_full(ctx, o, env, *span));
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape::FaceRect {
                face: *face,
                a: v[0],
                b: v[1],
                z: v[2],
                extent: v[3],
                h: v[4],
                fill: fill.clone(),
                opacity,
                span: *span,
            });
        }
        Node::Poly3d {
            style,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let style = eval_style(ctx, style, env, *span)?;
            return Some(Open::Poly3d {
                style,
                children,
                span: *span,
            });
        }
        Node::Shadow { args, when, span } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let v = evs!(args, *span, 4);
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape::Shadow {
                cx: v[0],
                cy: v[1],
                rx: v[2],
                ry: v[3],
                span: *span,
            });
        }
        Node::Seg {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let v = evs!(args, *span, 6);
            let style = eval_style(ctx, style, env, *span)?;
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape::Seg {
                a: [v[0], v[1], v[2]],
                b: [v[3], v[4], v[5]],
                style,
                span: *span,
            });
        }
        Node::At {
            args,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let v = evs!(args, *span, 3);
            return Some(Open::At {
                at: v,
                children,
                span: *span,
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
                return None;
            }
            let n = repeat_count(ctx, count, env, *span)?;
            return Some(Open::Repeat {
                var,
                children,
                n,
                span: *span,
            });
        }
        Node::Insert {
            fragment,
            args,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            return open_insert(ctx, fragment, args, env, *span);
        }
        Node::Badge {
            icon,
            corner,
            scale,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            if !["ne", "nw", "se", "sw"].contains(&corner.as_str()) {
                ctx.diag(
                    *span,
                    format!("badge corner must be ne, nw, se or sw, got '{corner}'"),
                );
                return None;
            }
            let scale = scale.as_ref().and_then(|s| eval_full(ctx, s, env, *span));
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape::Badge {
                icon: icon.clone(),
                corner: corner.clone(),
                scale,
                span: *span,
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
    None
}

/// One node inside a poly3d: a point, or a repeat of more.
fn collect_pt3<'a: 'n, 'n>(
    ctx: &mut Ctx<'a>,
    node: &'n Node,
    env: &BTreeMap<String, f64>,
    out: &mut Vec<[f64; 3]>,
) -> Option<Open<'n>> {
    match node {
        Node::Pt3 { args, span } => {
            let mut v = [0.0; 3];
            let mut ok = true;
            for (i, a) in args.iter().enumerate() {
                match eval_full(ctx, a, env, *span) {
                    Some(val) => v[i] = val,
                    None => ok = false,
                }
            }
            if ok && ctx.spend(LimitKind::Shapes, *span) {
                out.push(v);
            }
            None
        }
        Node::Repeat {
            count,
            var,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let n = repeat_count(ctx, count, env, *span)?;
            Some(Open::Repeat {
                var,
                children,
                n,
                span: *span,
            })
        }
        other => {
            ctx.diag(node_span(other), "only pt and repeat belong inside poly3d");
            None
        }
    }
}

// --- 2-D expansion -------------------------------------------------------

/// One node into 2-D shapes; see [`expand_node_3d`].
fn expand_node_2d<'a: 'n, 'n>(
    ctx: &mut Ctx<'a>,
    node: &'n Node,
    env: &BTreeMap<String, f64>,
    out: &mut Vec<Shape2>,
) -> Option<Open<'n>> {
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
                return None;
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
                return None;
            }
            let a = evs2!(args, *span, 4);
            let style = eval_style(ctx, style, env, *span)?;
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape2::Line {
                args: a, style,
                span: *span,
            });
        }
        Node::Circle {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let a = evs2!(args, *span, 3);
            let style = eval_style(ctx, style, env, *span)?;
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape2::Circle {
                args: a, style,
                span: *span,
            });
        }
        Node::Ellipse {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let a = evs2!(args, *span, 4);
            let style = eval_style(ctx, style, env, *span)?;
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape2::Ellipse {
                args: a, style,
                span: *span,
            });
        }
        Node::Rect2d {
            args,
            rx,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let a = evs2!(args, *span, 4);
            let rx = match rx {
                Some(r) => {
                    let v = eval_2d(ctx, r, env, *span)?;
                    Some(v)
                },
                None => None,
            };
            let style = eval_style(ctx, style, env, *span)?;
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape2::Rect {
                args: a, rx, style,
                span: *span,
            });
        }
        Node::Arc {
            args,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let a = evs2!(args, *span, 5);
            let style = eval_style(ctx, style, env, *span)?;
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape2::Arc {
                args: a, style,
                span: *span,
            });
        }
        Node::Path {
            template,
            style,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
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
                            Some(EvNum::Lit(v)) => pieces.push(PathEv::Lit(crate::ir::format_number(v))),
                            Some(EvNum::NeedsAnchor {
                                src,
                                ast,
                                env,
                                span,
                            }) => {
                                pieces.push(PathEv::Expr {
                                src,
                                ast,
                                env,
                                span,
                            })
                            }
                            None => return None,
                        }
                    }
                }
            }
            let style = eval_style(ctx, style, env, *span)?;
            if !ctx.spend(LimitKind::Shapes, *span) {
                return None;
            }
            out.push(Shape2::Path {
                template: pieces,
                style,
                span: *span,
            });
        }
        Node::Poly2d {
            style,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let style = eval_style(ctx, style, env, *span)?;
            return Some(Open::Poly2d {
                style,
                children,
                span: *span,
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
                return None;
            }
            let n = repeat_count(ctx, count, env, *span)?;
            return Some(Open::Repeat {
                var,
                children,
                n,
                span: *span,
            });
        }
        Node::Insert {
            fragment,
            args,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            return open_insert(ctx, fragment, args, env, *span);
        }
        other => ctx.diag(
            node_span(other),
            "3-D node reached 2-D expansion (parser context bug or a fragment inserted in the wrong context)",
        ),
    }
    None
}

/// One node inside a 2-D poly: a point, or a repeat of more.
fn collect_pt2<'a: 'n, 'n>(
    ctx: &mut Ctx<'a>,
    node: &'n Node,
    env: &BTreeMap<String, f64>,
    out: &mut Vec<[EvNum; 2]>,
) -> Option<Open<'n>> {
    match node {
        Node::Pt2 { args, span } => {
            let a = eval_2d(ctx, &args[0], env, *span);
            let b = eval_2d(ctx, &args[1], env, *span);
            if let (Some(a), Some(b)) = (a, b) {
                if ctx.spend(LimitKind::Shapes, *span) {
                    out.push([a, b]);
                }
            }
            None
        }
        Node::Repeat {
            count,
            var,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let n = repeat_count(ctx, count, env, *span)?;
            Some(Open::Repeat {
                var,
                children,
                n,
                span: *span,
            })
        }
        other => {
            ctx.diag(node_span(other), "only pt and repeat belong inside poly");
            None
        }
    }
}

// --- fragments -----------------------------------------------------------

/// An insert, checked and its environment built; its parts are expanded
/// by [`run`], in whichever dimension the insert stands in.
fn open_insert<'a: 'n, 'n>(
    ctx: &mut Ctx<'a>,
    fragment: &str,
    args: &[(String, Num)],
    env: &BTreeMap<String, f64>,
    span: Span,
) -> Option<Open<'n>> {
    let resolved: &'a Resolved = ctx.resolved;
    let Some(frag) = resolved.fragments.get(fragment) else {
        let names: Vec<String> = resolved.fragments.keys().cloned().collect();
        ctx.diag(
            span,
            format!(
                "unknown fragment '{fragment}'{}",
                crate::resolve::suggest(fragment, names.iter().map(|s| s.as_str()))
            ),
        );
        return None;
    };
    if ctx.insert_stack.iter().any(|f| f == fragment) {
        let mut chain = ctx.insert_stack.clone();
        chain.push(fragment.to_string());
        ctx.diag(
            span,
            format!("fragment insertion cycle: {}", chain.join(" -> ")),
        );
        return None;
    }
    if let Some(limit) = ctx.insert_depth.limit {
        if ctx.insert_stack.len() as u64 >= limit {
            let message = past_limit(
                format!("icon '{}' nests fragment insertions here", ctx.icon),
                LimitKind::InsertDepth,
                limit,
                &format!("levels, {}", ctx.insert_depth.from),
            );
            ctx.diag(span, message);
            return None;
        }
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
            return None;
        }
        if let Some(v) = eval_full(ctx, aval, env, span) {
            frag_env.insert(aname.clone(), v);
        }
    }
    Some(Open::Insert {
        fragment: &frag.name,
        parts: &frag.body.parts,
        env: Rc::new(frag_env),
    })
}

// --- the expansion loop ----------------------------------------------------

/// The parameters and loop variables a node sees, shared by the tasks
/// that see them.
type Env = Rc<BTreeMap<String, f64>>;

/// What a node with children opens, for [`run`] to expand.
enum Open<'n> {
    At {
        at: [f64; 3],
        children: &'n [Node],
        span: Span,
    },
    Poly3d {
        style: EvStyle,
        children: &'n [Node],
        span: Span,
    },
    Poly2d {
        style: EvStyle,
        children: &'n [Node],
        span: Span,
    },
    Repeat {
        var: &'n str,
        children: &'n [Node],
        n: usize,
        span: Span,
    },
    Insert {
        fragment: &'n str,
        parts: &'n [PartDecl],
        env: Env,
    },
}

/// Where what is expanded goes: shapes in 3-D or 2-D, or a polygon's
/// points.
enum Sink {
    Shapes(Vec<Shape>),
    Shapes2(Vec<Shape2>),
    Pts3(Vec<[f64; 3]>),
    Pts2(Vec<[EvNum; 2]>),
}

/// A step of expansion.
enum Task<'n> {
    /// Expand a node, into whatever sink is on top.
    Node(&'n Node, Env),
    /// Iteration `i` of a repeat's `n`.
    Repeat {
        var: &'n str,
        children: &'n [Node],
        i: usize,
        n: usize,
        env: Env,
        span: Span,
    },
    /// The children of an `at` are done: its shape, from the sink on top.
    CloseAt {
        at: [f64; 3],
        span: Span,
    },
    /// A polygon's points are done: its shape, from the sink on top.
    ClosePoly3d {
        style: EvStyle,
        span: Span,
    },
    ClosePoly2d {
        style: EvStyle,
        span: Span,
    },
    /// A fragment's parts are done.
    PopInsert,
}

/// Expand `nodes` into `out` with an explicit stack of tasks and sinks
/// rather than recursion, since repeats, `at`, polygons and fragment
/// inserts may nest to any depth (#28). Tasks run in the order a
/// recursive expansion visited nodes, so diagnostics, limits and the
/// shapes themselves come out in the same order; a node with children is
/// counted toward the shape-limit after them, as it was.
fn run<'a: 'n, 'n>(ctx: &mut Ctx<'a>, nodes: &'n [Node], env: Env, out: &mut Vec<Shape>) {
    let mut tasks: Vec<Task<'n>> = nodes
        .iter()
        .rev()
        .map(|node| Task::Node(node, env.clone()))
        .collect();
    let mut sinks = vec![Sink::Shapes(std::mem::take(out))];
    while let Some(task) = tasks.pop() {
        match task {
            Task::Node(node, env) => {
                let opened = match sinks.last_mut().expect("a sink") {
                    // once a limit is passed, no more shapes are expanded
                    Sink::Shapes(_) | Sink::Shapes2(_) if ctx.exhausted => None,
                    Sink::Shapes(out) => expand_node_3d(ctx, node, &env, out),
                    Sink::Shapes2(out) => expand_node_2d(ctx, node, &env, out),
                    Sink::Pts3(out) => collect_pt3(ctx, node, &env, out),
                    Sink::Pts2(out) => collect_pt2(ctx, node, &env, out),
                };
                if let Some(open) = opened {
                    open_children(ctx, open, env, &mut tasks, &mut sinks);
                }
            }
            Task::Repeat {
                var,
                children,
                i,
                n,
                env,
                span,
            } => {
                if i >= n || !ctx.spend(LimitKind::Repeats, span) {
                    continue;
                }
                tasks.push(Task::Repeat {
                    var,
                    children,
                    i: i + 1,
                    n,
                    env: env.clone(),
                    span,
                });
                let mut env2 = (*env).clone();
                env2.insert(var.to_string(), i as f64);
                push_nodes(&mut tasks, children, Rc::new(env2));
            }
            Task::CloseAt { at, span } => {
                let Some(Sink::Shapes2(children)) = sinks.pop() else {
                    unreachable!("an at's children are 2-D shapes");
                };
                if ctx.spend(LimitKind::Shapes, span) {
                    let Some(Sink::Shapes(out)) = sinks.last_mut() else {
                        unreachable!("an at stands among 3-D shapes");
                    };
                    out.push(Shape::At {
                        x: at[0],
                        y: at[1],
                        z: at[2],
                        children,
                        span,
                    });
                }
            }
            Task::ClosePoly3d { style, span } => {
                let Some(Sink::Pts3(pts)) = sinks.pop() else {
                    unreachable!("a poly3d's children are 3-D points");
                };
                if ctx.spend(LimitKind::Shapes, span) {
                    let Some(Sink::Shapes(out)) = sinks.last_mut() else {
                        unreachable!("a poly3d stands among 3-D shapes");
                    };
                    out.push(Shape::Poly3d { pts, style, span });
                }
            }
            Task::ClosePoly2d { style, span } => {
                let Some(Sink::Pts2(pts)) = sinks.pop() else {
                    unreachable!("a poly's children are 2-D points");
                };
                if ctx.spend(LimitKind::Shapes, span) {
                    let Some(Sink::Shapes2(out)) = sinks.last_mut() else {
                        unreachable!("a poly stands among 2-D shapes");
                    };
                    out.push(Shape2::Poly { pts, style, span });
                }
            }
            Task::PopInsert => {
                ctx.insert_stack.pop();
            }
        }
    }
    let Some(Sink::Shapes(shapes)) = sinks.pop() else {
        unreachable!("the icon's own shapes, at the bottom");
    };
    *out = shapes;
}

fn push_nodes<'n>(tasks: &mut Vec<Task<'n>>, nodes: &'n [Node], env: Env) {
    tasks.extend(nodes.iter().rev().map(|node| Task::Node(node, env.clone())));
}

/// Set up what a node opened: a sink for its children where they are
/// collected apart, and the tasks that expand them and then close it.
fn open_children<'n>(
    ctx: &mut Ctx,
    open: Open<'n>,
    env: Env,
    tasks: &mut Vec<Task<'n>>,
    sinks: &mut Vec<Sink>,
) {
    match open {
        Open::At { at, children, span } => {
            tasks.push(Task::CloseAt { at, span });
            sinks.push(Sink::Shapes2(Vec::new()));
            push_nodes(tasks, children, env);
        }
        Open::Poly3d {
            style,
            children,
            span,
        } => {
            tasks.push(Task::ClosePoly3d { style, span });
            sinks.push(Sink::Pts3(Vec::new()));
            push_nodes(tasks, children, env);
        }
        Open::Poly2d {
            style,
            children,
            span,
        } => {
            tasks.push(Task::ClosePoly2d { style, span });
            sinks.push(Sink::Pts2(Vec::new()));
            push_nodes(tasks, children, env);
        }
        Open::Repeat {
            var,
            children,
            n,
            span,
        } => tasks.push(Task::Repeat {
            var,
            children,
            i: 0,
            n,
            env,
            span,
        }),
        Open::Insert {
            fragment,
            parts,
            env,
        } => {
            // on the stack exactly while its parts expand, in the
            // dimension the insert stands in
            ctx.insert_stack.push(fragment.to_string());
            tasks.push(Task::PopInsert);
            for part in parts.iter().rev() {
                push_nodes(tasks, &part.nodes, env.clone());
            }
        }
    }
}

fn node_span(node: &Node) -> Span {
    match node {
        Node::Box3 { span, .. }
        | Node::Tile { span, .. }
        | Node::Cylinder { span, .. }
        | Node::FaceRect { span, .. }
        | Node::Poly3d { span, .. }
        | Node::Shadow { span, .. }
        | Node::Seg { span, .. }
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
