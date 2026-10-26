//! Expansion: axis products, parameter evaluation, repeat/when/insert
//! flattening. Output is a shape tree with every value evaluated except
//! those that reference an `at` anchor's screen coordinates (`x`, `y`),
//! which projection supplies; such values carry a captured environment
//! snapshot so scoping is decided here, once.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use serde::Serialize;

use crate::ast::{
    Baseline, Bounds, Face, FontKind, Limit, LimitKind, Limits, MirrorAxis, Node, Num, OntoFace,
    Param, PartDecl, PathPiece, Span, Step, Strokes, Style, TextAnchor, TextSpec, TransformSpec,
    TurnBy, Variant,
};
use crate::expr::{self, Expr};
use crate::resolve::{apply_variant, style_font, suggest, Resolved, ResolvedDef, SpanDiag};

/// What a node sees: the numbers of its params and loop variables, and
/// its text params' text. It reads as its numbers, which are all an
/// expression sees.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    nums: BTreeMap<String, f64>,
    texts: BTreeMap<String, String>,
}

impl std::ops::Deref for Scope {
    type Target = BTreeMap<String, f64>;
    fn deref(&self) -> &Self::Target {
        &self.nums
    }
}

impl std::ops::DerefMut for Scope {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.nums
    }
}

/// A param's value as a host gives it, compiling one icon (SPEC §10):
/// taken as it is, never read as an expression or a template.
#[derive(Debug, Clone, PartialEq)]
pub enum HostValue {
    Num(f64),
    Text(String),
}

/// One icon to compile, by the name the lexicon emits it under, with
/// params a host sets on it.
#[derive(Debug, Clone, Default)]
pub struct Only {
    pub icon: String,
    pub params: BTreeMap<String, HostValue>,
}

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
    } else if kind.of_fonts() {
        "in the font or the lexicon block"
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
    /// The shapes up to the matching end are drawn through this
    /// transform. Transforms nest as markers in one flat list, never as a
    /// tree, so how deeply they nest costs no call stack.
    TransformStart {
        tf: EvTransform,
        span: Span,
    },
    TransformEnd,
}

/// A transform with its values evaluated, except those that need the
/// anchor's screen position, which projection supplies.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvTransform {
    pub origin: [EvNum; 2],
    pub steps: Vec<EvStep>,
    /// The face map `onto` gives, worked out from the box once it is
    /// turned.
    pub onto: Option<[f64; 6]>,
    pub bounds: Option<Bounds>,
    pub strokes: Option<Strokes>,
    /// A text's fill: every paint of its glyphs, but none, drawn in it.
    pub paint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum EvStep {
    Translate([EvNum; 2]),
    Rotate(EvNum),
    Scale([EvNum; 2]),
    SkewX(EvNum),
    SkewY(EvNum),
    Mirror(MirrorAxis),
    Matrix(Box<[EvNum; 6]>),
}

/// A tile-space turn (SPEC §4): a quarter-turn or a mirror across a
/// diagonal, and a shift. A point (x, y) maps to (a0·x + a1·y + t0,
/// a2·x + a3·y + t1), each `a` 0, 1 or −1, so turning is exact.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Turn {
    a: [i8; 4],
    t: [f64; 2],
}

impl Turn {
    pub const NONE: Turn = Turn {
        a: [1, 0, 0, 1],
        t: [0.0, 0.0],
    };
    /// A quarter-turn carries +x onto +y.
    const QUARTER: [i8; 4] = [0, -1, 1, 0];
    const DIAGONAL: [i8; 4] = [0, 1, 1, 0];

    fn mul_a(m: [i8; 4], n: [i8; 4]) -> [i8; 4] {
        [
            m[0] * n[0] + m[1] * n[2],
            m[0] * n[1] + m[1] * n[3],
            m[2] * n[0] + m[3] * n[2],
            m[2] * n[1] + m[3] * n[3],
        ]
    }

    fn quarters(q: u8) -> [i8; 4] {
        (0..q).fold([1, 0, 0, 1], |a, _| Turn::mul_a(Turn::QUARTER, a))
    }

    fn lin(a: [i8; 4], [x, y]: [f64; 2]) -> [f64; 2] {
        // a sum of the terms that are there, so nothing is multiplied by 0
        let term = |k: i8, v: f64| match k {
            1 => v,
            -1 => -v,
            _ => 0.0,
        };
        [term(a[0], x) + term(a[1], y), term(a[2], x) + term(a[3], y)]
    }

    /// `a` about a point rather than the origin.
    fn about(a: [i8; 4], o: [f64; 2]) -> Turn {
        let ao = Turn::lin(a, o);
        Turn {
            a,
            t: [o[0] - ao[0], o[1] - ao[1]],
        }
    }

    /// This after `inner`: a point goes through `inner`, then this.
    fn then(&self, inner: &Turn) -> Turn {
        let t = Turn::lin(self.a, inner.t);
        Turn {
            a: Turn::mul_a(self.a, inner.a),
            t: [t[0] + self.t[0], t[1] + self.t[1]],
        }
    }

    pub fn pt(&self, x: f64, y: f64) -> (f64, f64) {
        let [px, py] = Turn::lin(self.a, [x, y]);
        (px + self.t[0], py + self.t[1])
    }

    /// Width and depth trade places.
    pub fn swaps(&self) -> bool {
        self.a[0] == 0
    }

    /// A rectangle on the floor, turned: still one, its corner nearest
    /// the origin first.
    fn rect(&self, x: f64, y: f64, w: f64, d: f64) -> (f64, f64, f64, f64) {
        let (x0, y0) = self.pt(x, y);
        let (x1, y1) = self.pt(x + w, y + d);
        (x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs())
    }

    /// The face a wall facing +y (front) or +x (right) faces once turned,
    /// or none when it faces away, since only those two are seen.
    fn face(&self, face: Face) -> Option<Face> {
        let normal = match face {
            Face::Front => [0, 1],
            Face::Right => [1, 0],
        };
        let n = [
            self.a[0] * normal[0] + self.a[1] * normal[1],
            self.a[2] * normal[0] + self.a[3] * normal[1],
        ];
        match n {
            [0, 1] => Some(Face::Front),
            [1, 0] => Some(Face::Right),
            _ => None,
        }
    }
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
    /// See [`Shape2::TransformStart`].
    TransformStart {
        tf: EvTransform,
        span: Span,
    },
    TransformEnd,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpandedIcon {
    pub name: String,
    pub category: String,
    /// As drawn: turned with the icon's view.
    pub footprint: Option<[f64; 3]>,
    pub canvas: Option<[f64; 2]>,
    pub describe: Vec<String>,
    pub grants: Vec<(String, String)>,
    pub shapes: Vec<Shape>,
    /// What its transforms do past its frame, unless one says.
    pub transform_bounds: Option<Bounds>,
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
    transform_chain: InForce,
    text_length: InForce,
    text_limit: (InForce, u64),
    /// Transform steps around what is being expanded now.
    chain: u64,
    /// Glyphs being drawn around what is being expanded now.
    in_glyph: u32,
    /// A limit was passed: expansion of this icon stops everywhere.
    exhausted: bool,
    /// Each font in each style text has been set in, or None for one
    /// that could not be, once reported.
    fonts: BTreeMap<(String, Vec<String>), Option<Rc<FontInst<'a>>>>,
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
        self.transform_chain = in_force(LimitKind::TransformChain, &def.limits, lexicon);
        self.text_length = in_force(LimitKind::TextLength, &def.limits, lexicon);
        self.text_limit = (in_force(LimitKind::TextLimit, &def.limits, lexicon), 0);
        self.chain = 0;
        self.in_glyph = 0;
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
    let (icons, diags, _) = expand_with(resolved, None);
    (icons, diags)
}

/// One icon, with the params a host sets on it, and the icons it badges,
/// since it draws them; or what is wrong with the request: an icon the
/// lexicon does not emit, or a param it does not have or of another type.
pub fn expand_only(
    resolved: &Resolved,
    only: &Only,
) -> Result<(Vec<ExpandedIcon>, Vec<SpanDiag>), String> {
    match expand_with(resolved, Some(only)) {
        (_, _, Some(refused)) => Err(refused),
        (icons, diags, None) => Ok((icons, diags)),
    }
}

fn new_ctx(resolved: &Resolved) -> Ctx<'_> {
    let unset = InForce {
        limit: None,
        from: String::new(),
    };
    Ctx {
        resolved,
        diags: Vec::new(),
        insert_stack: Vec::new(),
        icon: String::new(),
        shapes: (unset.clone(), 0),
        repeats: (unset.clone(), 0),
        insert_depth: unset.clone(),
        expression_depth: unset.clone(),
        transform_chain: unset.clone(),
        text_length: unset.clone(),
        text_limit: (unset, 0),
        chain: 0,
        in_glyph: 0,
        exhausted: false,
        fonts: BTreeMap::new(),
    }
}

fn expand_with(
    resolved: &Resolved,
    only: Option<&Only>,
) -> (Vec<ExpandedIcon>, Vec<SpanDiag>, Option<String>) {
    let mut ctx = new_ctx(resolved);
    let mut out = Vec::new();
    // with one icon asked for, every emitted icon, to choose from
    let mut candidates: Vec<(ResolvedDef, String)> = Vec::new();

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
            if only.is_some() {
                candidates.push((def, name));
            } else if let Some(exp) = expand_one(&mut ctx, &def, name, None) {
                out.push(exp);
            }
        }
    }
    let Some(only) = only else {
        return (out, ctx.diags, None);
    };
    let Some(at) = candidates.iter().position(|(_, n)| *n == only.icon) else {
        let refused = format!(
            "the lexicon emits no icon '{}'{}",
            only.icon,
            suggest(&only.icon, candidates.iter().map(|(_, n)| n.as_str()))
        );
        return (out, ctx.diags, Some(refused));
    };
    if let Some(refused) = refuse_host_params(&candidates[at].0, &only.icon, &only.params) {
        return (out, ctx.diags, Some(refused));
    }
    let (def, name) = &candidates[at];
    if let Some(exp) = expand_one(&mut ctx, def, name.clone(), Some(&only.params)) {
        let targets: BTreeSet<&str> = exp
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Badge { icon, .. } => Some(icon.as_str()),
                _ => None,
            })
            .collect();
        let targets: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter(|(i, (_, n))| *i != at && targets.contains(n.as_str()))
            .map(|(i, _)| i)
            .collect();
        out.push(exp);
        for i in targets {
            let (def, name) = &candidates[i];
            if let Some(exp) = expand_one(&mut ctx, def, name.clone(), None) {
                out.push(exp);
            }
        }
    }
    (out, ctx.diags, None)
}

/// A host's params, held to the icon's: each one it has, of its type, and
/// a number finite.
fn refuse_host_params(
    def: &ResolvedDef,
    icon: &str,
    params: &BTreeMap<String, HostValue>,
) -> Option<String> {
    for (name, value) in params {
        let Some((_, declared)) = def.params.iter().find(|(n, _)| n == name) else {
            return Some(format!(
                "icon '{icon}' has no param '{name}'{}",
                suggest(name, def.params.iter().map(|(n, _)| n.as_str()))
            ));
        };
        match (declared, value) {
            (Param::Num(_), HostValue::Num(v)) if !v.is_finite() => {
                return Some(format!("param '{name}' of icon '{icon}' must be finite"));
            }
            (Param::Num(_), HostValue::Text(_)) => {
                return Some(format!(
                    "param '{name}' of icon '{icon}' is a number; give it a number"
                ));
            }
            (Param::Text(_), HostValue::Num(_)) => {
                return Some(format!(
                    "param '{name}' of icon '{icon}' is text; give it a string"
                ));
            }
            _ => {}
        }
    }
    None
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

fn expand_one(
    ctx: &mut Ctx,
    def: &ResolvedDef,
    name: String,
    host: Option<&BTreeMap<String, HostValue>>,
) -> Option<ExpandedIcon> {
    ctx.begin(&name, def);
    // parameters evaluate in declaration order; earlier params are in
    // scope for later defaults, and a host's value takes a param's slot
    let mut env = Scope::default();
    for (pname, pval) in &def.params {
        match host.and_then(|h| h.get(pname)) {
            Some(HostValue::Num(v)) => {
                env.insert(pname.clone(), *v);
            }
            Some(HostValue::Text(t)) => {
                env.texts.insert(pname.clone(), t.clone());
            }
            None => bind(ctx, &mut env, pname, pval, def.span)?,
        }
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

    // the view turns everything in place, in its footprint
    let mut turn = Turn::NONE;
    let mut footprint = footprint;
    if let Some(v) = &def.view {
        let angle = eval_full(ctx, v, &env, def.span)?;
        let q = quarter(ctx, angle, "view", def.span)?;
        if q != 0 {
            let Some([w, d, h]) = footprint else {
                ctx.diag(
                    def.span,
                    format!("icon '{name}' has a view but no footprint; a canvas icon is drawn flat, from no corner"),
                );
                return None;
            };
            let a = Turn::quarters(q);
            let (x, y, _, _) = Turn { a, t: [0.0, 0.0] }.rect(0.0, 0.0, w, d);
            turn = Turn { a, t: [-x, -y] };
            if turn.swaps() {
                footprint = Some([d, w, h]);
            }
        }
    }

    let mut shapes = Vec::new();
    let env: Env = Rc::new(env);
    for (_pname, nodes) in &def.parts {
        run(ctx, nodes, env.clone(), turn, &mut shapes);
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
        transform_bounds: def.transform_bounds,
        span: def.span,
    })
}

/// An angle as quarter-turns, 0 to 3; anything but a multiple of 90° is
/// an error.
fn quarter(ctx: &mut Ctx, angle: f64, what: &str, span: Span) -> Option<u8> {
    if angle % 90.0 != 0.0 {
        ctx.diag(
            span,
            format!("{what} takes a quarter-turn, 0, 90, 180 or 270 degrees, got {angle}"),
        );
        return None;
    }
    Some(((angle / 90.0).rem_euclid(4.0)) as u8)
}

/// A param bound in `scope`, evaluated there: a number, or text with its
/// interpolations filled in.
fn bind(ctx: &mut Ctx, scope: &mut Scope, name: &str, value: &Param, span: Span) -> Option<()> {
    match value {
        Param::Num(n) => {
            let v = eval_full(ctx, n, scope, span)?;
            scope.insert(name.to_string(), v);
        }
        Param::Text(t) => {
            let s = interpolate(ctx, t, scope, span)?;
            scope.texts.insert(name.to_string(), s);
        }
    }
    Some(())
}

/// Text with its `{…}` filled in: a text param's text, or a number as
/// text reads it (SPEC §4). Text is laid out when the lexicon compiles,
/// so the anchor's `x` and `y` are not known to it.
fn interpolate(ctx: &mut Ctx, pieces: &[PathPiece], scope: &Scope, span: Span) -> Option<String> {
    let mut out = String::new();
    for piece in pieces {
        match piece {
            PathPiece::Lit(s) => out.push_str(s),
            PathPiece::Expr { src, ast } => {
                if let Some(text) = ast
                    .as_name()
                    .filter(|n| !scope.contains_key(*n))
                    .and_then(|n| scope.texts.get(n))
                {
                    out.push_str(text);
                    continue;
                }
                let mut free = BTreeSet::new();
                expr::free_vars(ast, &mut free);
                if free.iter().any(|n| n == "x" || n == "y") {
                    ctx.diag(
                        span,
                        format!(
                            "in '{src}': text is laid out when the lexicon compiles, before any \
                             anchor is placed, so x and y are not known to it"
                        ),
                    );
                    return None;
                }
                let num = Num::Expr {
                    src: src.clone(),
                    ast: ast.clone(),
                };
                let v = eval_full(ctx, &num, scope, span)?;
                out.push_str(&crate::text::format_value(v));
            }
        }
    }
    Some(out)
}

/// An unknown name that is a text param, said as such.
fn text_not_number(src: &str, name: &str) -> String {
    format!(
        "in '{src}': '{name}' is text, not a number; text is interpolated whole into text, as \
         {{{name}}}"
    )
}

/// A value of a transform: in 2-D, it may wait for the anchor.
fn eval_tf(ctx: &mut Ctx, num: &Num, env: &Scope, span: Span, two_d: bool) -> Option<EvNum> {
    if two_d {
        eval_2d(ctx, num, env, span)
    } else {
        eval_full(ctx, num, env, span).map(EvNum::Lit)
    }
}

/// A transform's values, and with `onto`, its face map, from the box as
/// turned. None when its face is turned away: then nothing in it is
/// drawn, as a face rect on a wall turned away is not.
fn eval_transform(
    ctx: &mut Ctx,
    spec: &TransformSpec,
    env: &Scope,
    turn: &Turn,
    span: Span,
    two_d: bool,
) -> Option<EvTransform> {
    let ev = |ctx: &mut Ctx, n: &Num| eval_tf(ctx, n, env, span, two_d);
    let origin = match &spec.origin {
        Some([x, y]) => [ev(ctx, x)?, ev(ctx, y)?],
        None => [EvNum::Lit(0.0), EvNum::Lit(0.0)],
    };
    let mut steps = Vec::new();
    for step in &spec.steps {
        steps.push(match step {
            Step::Translate([x, y]) => EvStep::Translate([ev(ctx, x)?, ev(ctx, y)?]),
            Step::Rotate(a) => EvStep::Rotate(ev(ctx, a)?),
            Step::Scale([x, y]) => EvStep::Scale([ev(ctx, x)?, ev(ctx, y)?]),
            Step::SkewX(a) => EvStep::SkewX(ev(ctx, a)?),
            Step::SkewY(a) => EvStep::SkewY(ev(ctx, a)?),
            Step::Mirror(axis) => EvStep::Mirror(*axis),
            Step::Matrix(m) => {
                let mut out: [EvNum; 6] = core::array::from_fn(|_| EvNum::Lit(0.0));
                for (i, v) in m.iter().enumerate() {
                    out[i] = ev(ctx, v)?;
                }
                EvStep::Matrix(Box::new(out))
            }
        });
    }
    let onto = match &spec.onto {
        None => None,
        Some((face, args)) => {
            let mut v = [0.0; 6];
            for (i, a) in args.iter().enumerate() {
                v[i] = eval_full(ctx, a, env, span)?;
            }
            let (x, y, w, d) = turn.rect(v[0], v[1], v[3], v[4]);
            let (z, h) = (v[2], v[5]);
            use crate::project::{p, COS, SIN};
            Some(match face {
                OntoFace::Top => {
                    let (ox, oy) = p(x, y, z + h);
                    [COS, SIN, -COS, SIN, ox, oy]
                }
                OntoFace::Front | OntoFace::Right => {
                    let wall = if *face == OntoFace::Front {
                        Face::Front
                    } else {
                        Face::Right
                    };
                    match turn.face(wall)? {
                        Face::Front => {
                            let (ox, oy) = p(x, y + d, z + h);
                            [COS, SIN, 0.0, 1.0, ox, oy]
                        }
                        Face::Right => {
                            let (ox, oy) = p(x + w, y + d, z + h);
                            [COS, -SIN, 0.0, 1.0, ox, oy]
                        }
                    }
                }
            })
        }
    };
    Some(EvTransform {
        origin,
        steps,
        onto,
        bounds: spec.bounds,
        strokes: spec.strokes,
        paint: None,
    })
}

/// How long a chain a transform adds: a step each, and `onto` one more.
fn chain_length(tf: &EvTransform) -> u64 {
    tf.steps.len() as u64 + u64::from(tf.onto.is_some())
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

fn eval_full(ctx: &mut Ctx, num: &Num, env: &Scope, span: Span) -> Option<f64> {
    match num {
        Num::Lit(v) => Some(*v),
        Num::Expr { src, ast } => {
            if !within_expression_depth(ctx, ast, span) {
                return None;
            }
            let mut free = BTreeSet::new();
            expr::free_vars(ast, &mut free);
            for name in &free {
                if !env.contains_key(name) && env.texts.contains_key(name) {
                    ctx.diag(span, text_not_number(src, name));
                    return None;
                }
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
fn eval_2d(ctx: &mut Ctx, num: &Num, env: &Scope, span: Span) -> Option<EvNum> {
    match num {
        Num::Lit(v) => Some(EvNum::Lit(*v)),
        Num::Expr { src, ast } => {
            if !within_expression_depth(ctx, ast, span) {
                return None;
            }
            let mut free = BTreeSet::new();
            expr::free_vars(ast, &mut free);
            let needs_anchor = free.iter().any(|n| n == "x" || n == "y");
            if needs_anchor && ctx.in_glyph > 0 {
                ctx.diag(
                    span,
                    format!(
                        "in '{src}': a glyph is drawn in its font's em grid, from its origin on \
                         the baseline, where x and y mean nothing"
                    ),
                );
                return None;
            }
            for name in &free {
                if name != "x"
                    && name != "y"
                    && !env.contains_key(name)
                    && env.texts.contains_key(name)
                {
                    ctx.diag(span, text_not_number(src, name));
                    return None;
                }
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

fn eval_style(ctx: &mut Ctx, style: &Style, env: &Scope, span: Span) -> Option<EvStyle> {
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

fn when_passes(ctx: &mut Ctx, when: &Option<Num>, env: &Scope, span: Span) -> bool {
    match when {
        None => true,
        Some(n) => match eval_full(ctx, n, env, span) {
            Some(v) => v != 0.0,
            None => false,
        },
    }
}

fn repeat_count(ctx: &mut Ctx, count: &Num, env: &Scope, span: Span) -> Option<usize> {
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
    env: &Scope,
    turn: &Turn,
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
            let (x, y, w, d) = turn.rect(v[0], v[1], v[3], v[4]);
            out.push(Shape::Box3 {
                x,
                y,
                z: v[2],
                w,
                d,
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
            let (x, y, w, d) = turn.rect(v[0], v[1], v[3], v[4]);
            out.push(Shape::Tile {
                x,
                y,
                z: v[2],
                w,
                d,
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
            let (cx, cy) = turn.pt(v[0], v[1]);
            out.push(Shape::Cylinder {
                cx,
                cy,
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
            // to the wall it now lies on, or nowhere if that is turned away
            let now = turn.face(*face)?;
            let (end_a, end_b) = match face {
                Face::Front => ((v[0], v[1]), (v[0] + v[3], v[1])),
                Face::Right => ((v[0], v[1]), (v[0], v[1] + v[3])),
            };
            let (p, q) = (turn.pt(end_a.0, end_a.1), turn.pt(end_b.0, end_b.1));
            let (a, b) = match now {
                Face::Front => (p.0.min(q.0), p.1),
                Face::Right => (p.0, p.1.min(q.1)),
            };
            out.push(Shape::FaceRect {
                face: now,
                a,
                b,
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
            let (cx, cy) = turn.pt(v[0], v[1]);
            let (rx, ry) = if turn.swaps() {
                (v[3], v[2])
            } else {
                (v[2], v[3])
            };
            out.push(Shape::Shadow {
                cx,
                cy,
                rx,
                ry,
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
            let (ax, ay) = turn.pt(v[0], v[1]);
            let (bx, by) = turn.pt(v[3], v[4]);
            out.push(Shape::Seg {
                a: [ax, ay, v[2]],
                b: [bx, by, v[5]],
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
            let (x, y) = turn.pt(v[0], v[1]);
            return Some(Open::At {
                at: [x, y, v[2]],
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
        Node::Transform {
            spec,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let tf = eval_transform(ctx, spec, env, turn, *span, false)?;
            return Some(Open::Transform {
                tf,
                children,
                span: *span,
            });
        }
        Node::Turn {
            by,
            origin,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let o = match origin {
                Some([x, y]) => [
                    eval_full(ctx, x, env, *span)?,
                    eval_full(ctx, y, env, *span)?,
                ],
                None => [0.0, 0.0],
            };
            let a = match by {
                TurnBy::Angle(angle) => {
                    let angle = eval_full(ctx, angle, env, *span)?;
                    Turn::quarters(quarter(ctx, angle, "turn", *span)?)
                }
                TurnBy::MirrorDiagonal => Turn::DIAGONAL,
            };
            return Some(Open::Turn {
                turn: turn.then(&Turn::about(a, o)),
                children,
            });
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
            // a badge in a transform is one more step for its shapes
            let chain = ctx.chain;
            if chain > 0 && !within_chain(ctx, chain + 1, *span) {
                return None;
            }
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
    env: &Scope,
    turn: &Turn,
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
                let (x, y) = turn.pt(v[0], v[1]);
                out.push([x, y, v[2]]);
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
    env: &Scope,
    turn: &Turn,
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
        Node::Transform {
            spec,
            children,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            let tf = eval_transform(ctx, spec, env, turn, *span, true)?;
            return Some(Open::Transform {
                tf,
                children,
                span: *span,
            });
        }
        Node::Text {
            args,
            content,
            spec,
            when,
            span,
        } => {
            if !when_passes(ctx, when, env, *span) {
                return None;
            }
            if ctx.in_glyph > 0 {
                ctx.diag(
                    *span,
                    "a glyph cannot set text: it is drawn from shapes, in its font's em grid",
                );
                return None;
            }
            let [x, y] = evs2!(args, *span, 2);
            let words = interpolate(ctx, content, env, *span)?;
            let size = eval_full(ctx, &spec.size, env, *span)?;
            if size <= 0.0 {
                ctx.diag(*span, format!("text size must be more than 0, got {size}"));
                return None;
            }
            let max_width = match &spec.max_width {
                Some(m) => {
                    let v = eval_full(ctx, m, env, *span)?;
                    if v <= 0.0 {
                        ctx.diag(*span, format!("max-width must be more than 0, got {v}"));
                        return None;
                    }
                    Some(v)
                }
                None => None,
            };
            return set_text(ctx, &words, [x, y], size, max_width, spec, *span);
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
    env: &Scope,
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
    args: &[(String, Param)],
    env: &Scope,
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
    let mut frag_env = Scope::default();
    for (pname, pval, pspan) in &frag.body.params {
        let _ = bind(ctx, &mut frag_env, pname, pval, *pspan);
    }
    for (aname, aval) in args {
        let Some((_, declared, _)) = frag.body.params.iter().find(|(n, _, _)| n == aname) else {
            let known = frag.body.params.iter().map(|(n, _, _)| n.as_str());
            ctx.diag(
                span,
                format!(
                    "fragment '{fragment}' has no parameter '{aname}'{}",
                    crate::resolve::suggest(aname, known)
                ),
            );
            return None;
        };
        match (declared, aval) {
            (Param::Num(_), Param::Num(n)) => {
                if let Some(v) = eval_full(ctx, n, env, span) {
                    frag_env.insert(aname.clone(), v);
                }
            }
            (Param::Text(_), Param::Text(t)) => {
                if let Some(text) = interpolate(ctx, t, env, span) {
                    frag_env.texts.insert(aname.clone(), text);
                }
            }
            (Param::Num(_), Param::Text(_)) => {
                ctx.diag(
                    span,
                    format!("fragment '{fragment}' takes a number for '{aname}', not text"),
                );
                return None;
            }
            (Param::Text(_), Param::Num(_)) => {
                ctx.diag(
                    span,
                    format!("fragment '{fragment}' takes text for '{aname}': {aname}=(text)\"…\""),
                );
                return None;
            }
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
type Env = Rc<Scope>;

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
    Transform {
        tf: EvTransform,
        children: &'n [Node],
        span: Span,
    },
    Turn {
        turn: Turn,
        children: &'n [Node],
    },
    /// A text laid out: its map, and each glyph's place on its baseline
    /// and shapes, drawn in its font's scope.
    Text {
        outer: EvTransform,
        glyphs: Vec<(f64, &'n [Node])>,
        font_env: Env,
        span: Span,
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
    /// Expand a node, turned, into whatever sink is on top.
    Node(&'n Node, Env, Turn),
    /// Iteration `i` of a repeat's `n`.
    Repeat {
        var: &'n str,
        children: &'n [Node],
        i: usize,
        n: usize,
        env: Env,
        turn: Turn,
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
    /// A transform's children are done: the end of its run, and of its
    /// steps in the chain.
    CloseTransform {
        steps: u64,
    },
    /// A glyph of a text begins at its place on the baseline, or ends.
    StartGlyph {
        pen: f64,
        span: Span,
    },
    CloseGlyph,
}

/// Expand `nodes` into `out` with an explicit stack of tasks and sinks
/// rather than recursion, since repeats, `at`, polygons and fragment
/// inserts may nest to any depth (#28). Tasks run in the order a
/// recursive expansion visited nodes, so diagnostics, limits and the
/// shapes themselves come out in the same order; a node with children is
/// counted toward the shape-limit after them, as it was.
fn run<'a: 'n, 'n>(
    ctx: &mut Ctx<'a>,
    nodes: &'n [Node],
    env: Env,
    turn: Turn,
    out: &mut Vec<Shape>,
) {
    let mut tasks: Vec<Task<'n>> = nodes
        .iter()
        .rev()
        .map(|node| Task::Node(node, env.clone(), turn))
        .collect();
    let mut sinks = vec![Sink::Shapes(std::mem::take(out))];
    while let Some(task) = tasks.pop() {
        match task {
            Task::Node(node, env, turn) => {
                let opened = match sinks.last_mut().expect("a sink") {
                    // once a limit is passed, no more shapes are expanded
                    Sink::Shapes(_) | Sink::Shapes2(_) if ctx.exhausted => None,
                    Sink::Shapes(out) => expand_node_3d(ctx, node, &env, &turn, out),
                    Sink::Shapes2(out) => expand_node_2d(ctx, node, &env, &turn, out),
                    Sink::Pts3(out) => collect_pt3(ctx, node, &env, &turn, out),
                    Sink::Pts2(out) => collect_pt2(ctx, node, &env, out),
                };
                if let Some(open) = opened {
                    open_children(ctx, open, env, turn, &mut tasks, &mut sinks);
                }
            }
            Task::Repeat {
                var,
                children,
                i,
                n,
                env,
                turn,
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
                    turn,
                    span,
                });
                let mut env2 = (*env).clone();
                env2.insert(var.to_string(), i as f64);
                push_nodes(&mut tasks, children, Rc::new(env2), turn);
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
            Task::CloseTransform { steps } => {
                ctx.chain -= steps;
                match sinks.last_mut() {
                    Some(Sink::Shapes(out)) => out.push(Shape::TransformEnd),
                    Some(Sink::Shapes2(out)) => out.push(Shape2::TransformEnd),
                    _ => unreachable!("a transform stands among shapes"),
                }
            }
            Task::StartGlyph { pen, span } => {
                ctx.in_glyph += 1;
                ctx.chain += 1;
                // counted, and marked even past the shape-limit, so every
                // end has its start
                ctx.spend(LimitKind::Shapes, span);
                let Some(Sink::Shapes2(out)) = sinks.last_mut() else {
                    unreachable!("a glyph is drawn among 2-D shapes");
                };
                out.push(Shape2::TransformStart {
                    tf: EvTransform {
                        origin: [EvNum::Lit(0.0), EvNum::Lit(0.0)],
                        steps: vec![EvStep::Translate([EvNum::Lit(pen), EvNum::Lit(0.0)])],
                        onto: None,
                        bounds: None,
                        strokes: None,
                        paint: None,
                    },
                    span,
                });
            }
            Task::CloseGlyph => {
                ctx.in_glyph -= 1;
                ctx.chain -= 1;
                let Some(Sink::Shapes2(out)) = sinks.last_mut() else {
                    unreachable!("a glyph is drawn among 2-D shapes");
                };
                out.push(Shape2::TransformEnd);
            }
        }
    }
    let Some(Sink::Shapes(shapes)) = sinks.pop() else {
        unreachable!("the icon's own shapes, at the bottom");
    };
    *out = shapes;
}

fn push_nodes<'n>(tasks: &mut Vec<Task<'n>>, nodes: &'n [Node], env: Env, turn: Turn) {
    tasks.extend(
        nodes
            .iter()
            .rev()
            .map(|node| Task::Node(node, env.clone(), turn)),
    );
}

/// A chain of `length` steps against the icon's transform-chain (SPEC §9).
fn within_chain(ctx: &mut Ctx, length: u64, span: Span) -> bool {
    match ctx.transform_chain.limit {
        Some(limit) if length > limit => {
            let message = past_limit(
                format!(
                    "icon '{}' draws shapes through {length} transform steps here",
                    ctx.icon
                ),
                LimitKind::TransformChain,
                limit,
                &format!("steps, {}", ctx.transform_chain.from),
            );
            ctx.diag(span, message);
            false
        }
        _ => true,
    }
}

/// Set up what a node opened: a sink for its children where they are
/// collected apart, and the tasks that expand them and then close it.
fn open_children<'n>(
    ctx: &mut Ctx,
    open: Open<'n>,
    env: Env,
    turn: Turn,
    tasks: &mut Vec<Task<'n>>,
    sinks: &mut Vec<Sink>,
) {
    match open {
        Open::At { at, children, span } => {
            tasks.push(Task::CloseAt { at, span });
            sinks.push(Sink::Shapes2(Vec::new()));
            push_nodes(tasks, children, env, turn);
        }
        Open::Transform { tf, children, span } => {
            let steps = chain_length(&tf);
            let chain = ctx.chain + steps;
            if !within_chain(ctx, chain, span) || !ctx.spend(LimitKind::Shapes, span) {
                return;
            }
            ctx.chain += steps;
            match sinks.last_mut() {
                Some(Sink::Shapes(out)) => out.push(Shape::TransformStart { tf, span }),
                Some(Sink::Shapes2(out)) => out.push(Shape2::TransformStart { tf, span }),
                _ => {
                    ctx.chain -= steps;
                    ctx.diag(span, "only pt and repeat belong inside a polygon");
                    return;
                }
            }
            tasks.push(Task::CloseTransform { steps });
            push_nodes(tasks, children, env, turn);
        }
        Open::Turn { turn, children } => push_nodes(tasks, children, env, turn),
        Open::Text {
            outer,
            glyphs,
            font_env,
            span,
        } => {
            // its own map, a matrix and a translate, and each glyph's place
            if !within_chain(ctx, ctx.chain + 3, span) || !ctx.spend(LimitKind::Shapes, span) {
                return;
            }
            let Some(Sink::Shapes2(out)) = sinks.last_mut() else {
                ctx.diag(span, "only pt and repeat belong inside a polygon");
                return;
            };
            out.push(Shape2::TransformStart { tf: outer, span });
            ctx.chain += 2;
            tasks.push(Task::CloseTransform { steps: 2 });
            for (pen, nodes) in glyphs.into_iter().rev() {
                tasks.push(Task::CloseGlyph);
                push_nodes(tasks, nodes, font_env.clone(), Turn::NONE);
                tasks.push(Task::StartGlyph { pen, span });
            }
        }
        Open::Poly3d {
            style,
            children,
            span,
        } => {
            tasks.push(Task::ClosePoly3d { style, span });
            sinks.push(Sink::Pts3(Vec::new()));
            push_nodes(tasks, children, env, turn);
        }
        Open::Poly2d {
            style,
            children,
            span,
        } => {
            tasks.push(Task::ClosePoly2d { style, span });
            sinks.push(Sink::Pts2(Vec::new()));
            push_nodes(tasks, children, env, turn);
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
            turn,
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
                push_nodes(tasks, &part.nodes, env.clone(), turn);
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
        | Node::Badge { span, .. }
        | Node::Transform { span, .. }
        | Node::Turn { span, .. }
        | Node::Text { span, .. } => *span,
    }
}

// --- text ----------------------------------------------------------------

/// A font as text is set in it: in its styles, with its params, metrics,
/// advances and kerning evaluated, and its glyphs by the text they draw.
struct FontInst<'a> {
    name: String,
    kind: Option<FontKind>,
    em: f64,
    ascent: f64,
    descent: f64,
    slant: f64,
    env: Env,
    glyphs: BTreeMap<&'a str, Placed<'a>>,
    /// Characters in its longest glyph name: how far a ligature reaches.
    longest: usize,
    notdef: Option<Placed<'a>>,
    kerns: BTreeMap<(&'a str, &'a str), f64>,
}

/// A glyph ready to place: its name (none for notdef, which is never
/// kerned), advance and shapes.
#[derive(Clone, Copy)]
struct Placed<'a> {
    name: Option<&'a str>,
    advance: f64,
    nodes: &'a [Node],
}

/// "'é' (U+00E9)", or for a character that would not print, its code
/// alone.
fn describe_char(c: char) -> String {
    if c.is_control() || c.is_whitespace() {
        format!("U+{:04X}", c as u32)
    } else {
        format!("'{c}' (U+{:04X})", c as u32)
    }
}

/// The font a text is set in, in its styles, built once and kept.
fn font_instance<'a>(ctx: &mut Ctx<'a>, spec: &TextSpec, span: Span) -> Option<Rc<FontInst<'a>>> {
    let key = (spec.font.clone(), spec.styles.clone());
    if let Some(built) = ctx.fonts.get(&key) {
        return built.clone();
    }
    let built = build_font(ctx, spec, span).map(Rc::new);
    ctx.fonts.insert(key, built.clone());
    built
}

fn build_font<'a>(ctx: &mut Ctx<'a>, spec: &TextSpec, span: Span) -> Option<FontInst<'a>> {
    let resolved: &'a Resolved = ctx.resolved;
    let Some(font) = resolved.fonts.get(&spec.font) else {
        ctx.diag(
            span,
            format!(
                "unknown font '{}'{}",
                spec.font,
                suggest(&spec.font, resolved.fonts.keys().map(|k| k.as_str()))
            ),
        );
        return None;
    };
    // each style a variant of one of the font's axes, one to an axis,
    // laid over it in its axes' order
    let mut chosen: Vec<(usize, &'a Variant)> = Vec::new();
    for style in &spec.styles {
        let found = font.body.axes.iter().enumerate().find_map(|(i, a)| {
            resolved
                .axes
                .get(a)
                .and_then(|axis| axis.variants.iter().find(|v| &v.name == style))
                .map(|v| (i, v))
        });
        let Some((i, v)) = found else {
            let styles: Vec<&str> = font
                .body
                .axes
                .iter()
                .filter_map(|a| resolved.axes.get(a))
                .flat_map(|axis| axis.variants.iter().map(|v| v.name.as_str()))
                .collect();
            ctx.diag(
                span,
                format!(
                    "font '{}' has no style '{style}'{} ({})",
                    font.name,
                    suggest(style, styles.iter().copied()),
                    if styles.is_empty() {
                        "it opts into no axes".to_string()
                    } else {
                        format!("its styles: {}", styles.join(", "))
                    }
                ),
            );
            return None;
        };
        if let Some((_, other)) = chosen.iter().find(|(j, _)| *j == i) {
            ctx.diag(
                span,
                format!(
                    "text asks font '{}' for '{}' and '{style}', both of axis '{}'; choose one",
                    font.name, other.name, font.body.axes[i]
                ),
            );
            return None;
        }
        chosen.push((i, v));
    }
    chosen.sort_by_key(|(i, _)| *i);
    let variants: Vec<&'a Variant> = chosen.into_iter().map(|(_, v)| v).collect();
    let styled = style_font(font, &variants, &mut ctx.diags)?;

    let count = styled.glyphs.len() as u64 + u64::from(styled.notdef.is_some());
    let set: BTreeMap<LimitKind, (Limit, String)> = styled
        .glyph_limit
        .iter()
        .map(|l| (LimitKind::GlyphLimit, l.clone()))
        .collect();
    let limit = in_force(LimitKind::GlyphLimit, &set, &resolved.lexicon_limits);
    if let Some(max) = limit.limit {
        if count > max {
            let message = past_limit(
                format!("font '{}' holds {count} glyphs", font.name),
                LimitKind::GlyphLimit,
                max,
                &format!("glyphs, {}", limit.from),
            );
            ctx.diag(font.span, message);
            return None;
        }
    }

    let mut env = Scope::default();
    for (pname, p) in &styled.params {
        bind(ctx, &mut env, pname, p, font.span)?;
    }
    // the resolver has said what a font without these lacks
    let (Some(em), Some(ascent), Some(descent)) = (styled.em, styled.ascent, styled.descent) else {
        return None;
    };
    let em = eval_full(ctx, em, &env, font.span)?;
    if em <= 0.0 {
        ctx.diag(
            font.span,
            format!(
                "font '{}' has an em of {em}; an em is more than 0",
                font.name
            ),
        );
        return None;
    }
    let ascent = eval_full(ctx, ascent, &env, font.span)?;
    let descent = eval_full(ctx, descent, &env, font.span)?;
    let slant = match styled.slant {
        Some(s) => eval_full(ctx, s, &env, font.span)?,
        None => 0.0,
    };
    let mut glyphs = BTreeMap::new();
    let mut longest = 0;
    for g in &styled.glyphs {
        let advance = eval_full(ctx, &g.advance, &env, g.span)?;
        longest = longest.max(g.name.chars().count());
        glyphs.insert(
            g.name.as_str(),
            Placed {
                name: Some(g.name.as_str()),
                advance,
                nodes: &g.nodes,
            },
        );
    }
    let notdef = match styled.notdef {
        Some(g) => Some(Placed {
            name: None,
            advance: eval_full(ctx, &g.advance, &env, g.span)?,
            nodes: &g.nodes,
        }),
        None => None,
    };
    let mut kerns = BTreeMap::new();
    for k in &styled.kerns {
        for side in [&k.left, &k.right] {
            if !glyphs.contains_key(side.as_str()) {
                // without axes, the resolver has said so
                if !font.body.axes.is_empty() {
                    ctx.diag(
                        k.span,
                        format!(
                            "font '{}' kerns \"{side}\", which it has no glyph for in these \
                             styles{}",
                            font.name,
                            suggest(side, glyphs.keys().copied())
                        ),
                    );
                }
                return None;
            }
        }
        let by = eval_full(ctx, &k.by, &env, k.span)?;
        kerns.insert((k.left.as_str(), k.right.as_str()), by);
    }
    Some(FontInst {
        name: font.name.clone(),
        kind: styled.kind,
        em,
        ascent,
        descent,
        slant,
        env: Rc::new(env),
        glyphs,
        longest,
        notdef,
        kerns,
    })
}

/// A font in some of its styles, for a tool that converts it: its
/// metrics, kerning, and each glyph's shapes, expanded in the font's scope
/// as text would draw them, one icon to a glyph.
#[derive(Debug)]
pub struct ExpandedFont {
    pub name: String,
    pub describe: Vec<String>,
    pub kind: FontKind,
    pub em: f64,
    pub ascent: f64,
    pub descent: f64,
    pub slant: f64,
    /// Each glyph by name, in name order, with its advance.
    pub glyphs: Vec<(String, f64, ExpandedIcon)>,
    pub notdef: Option<(f64, ExpandedIcon)>,
    /// (left, right, space), in name order.
    pub kerns: Vec<(String, String, f64)>,
}

/// A font in the styles named, every glyph expanded; or the errors that
/// stop it, the font's own or an unknown font or style.
pub fn expand_font(
    resolved: &Resolved,
    font: &str,
    styles: &[String],
) -> Result<ExpandedFont, Vec<SpanDiag>> {
    let mut ctx = new_ctx(resolved);
    let Some(def) = resolved.fonts.get(font) else {
        // no node names it: the request is at fault, said at the start
        let span = Span {
            file: 0,
            start: 0,
            len: 0,
        };
        return Err(vec![SpanDiag {
            span,
            message: format!(
                "the lexicon has no font '{font}'{}",
                suggest(font, resolved.fonts.keys().map(|k| k.as_str()))
            ),
        }]);
    };
    let spec = TextSpec {
        font: font.to_string(),
        styles: styles.to_vec(),
        size: Num::Lit(1.0),
        anchor: TextAnchor::Start,
        baseline: Baseline::Alphabetic,
        fill: None,
        max_width: None,
        bounds: None,
    };
    // the font's limits are its own, set nowhere on an icon
    let host = ResolvedDef {
        name: font.to_string(),
        kind: crate::ast::DefKind::Font,
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
        span: def.span,
    };
    ctx.begin(font, &host);
    let Some(inst) = font_instance(&mut ctx, &spec, def.span) else {
        return Err(ctx.diags);
    };
    let draw = |ctx: &mut Ctx, name: &str, nodes: &[Node]| -> ExpandedIcon {
        ctx.begin(&format!("{font} glyph {name}"), &host);
        ctx.in_glyph = 1;
        // drawn as if inside `at 0 0 0`, whose anchor is the origin
        let at = Node::At {
            args: [Num::Lit(0.0), Num::Lit(0.0), Num::Lit(0.0)],
            children: nodes.to_vec(),
            when: None,
            span: def.span,
        };
        let mut shapes = Vec::new();
        run(
            ctx,
            std::slice::from_ref(&at),
            inst.env.clone(),
            Turn::NONE,
            &mut shapes,
        );
        ExpandedIcon {
            name: name.to_string(),
            category: String::new(),
            footprint: None,
            canvas: Some([0.0, 0.0]),
            describe: Vec::new(),
            grants: Vec::new(),
            shapes,
            // a glyph has no frame of its own
            transform_bounds: Some(Bounds::Grow),
            span: def.span,
        }
    };
    let glyphs = inst
        .glyphs
        .iter()
        .map(|(name, g)| (name.to_string(), g.advance, draw(&mut ctx, name, g.nodes)))
        .collect();
    let notdef = inst
        .notdef
        .map(|g| (g.advance, draw(&mut ctx, ".notdef", g.nodes)));
    if !ctx.diags.is_empty() {
        return Err(ctx.diags);
    }
    Ok(ExpandedFont {
        name: inst.name.clone(),
        describe: def.body.describe.clone(),
        kind: inst.kind.unwrap_or(FontKind::Outline),
        em: inst.em,
        ascent: inst.ascent,
        descent: inst.descent,
        slant: inst.slant,
        glyphs,
        notdef,
        kerns: inst
            .kerns
            .iter()
            .map(|((l, r), by)| (l.to_string(), r.to_string(), *by))
            .collect(),
    })
}

/// A text laid out (SPEC §4): its characters held to the icon's text
/// limits and to what can be set left to right, each place taking the
/// longest glyph that matches there, or the font's notdef; advances and
/// kerning lay them along the baseline, and the text's map scales the em
/// grid to its size, turns y up, slants it, and aligns it at its anchor.
fn set_text<'a: 'n, 'n>(
    ctx: &mut Ctx<'a>,
    words: &str,
    [x, y]: [EvNum; 2],
    size: f64,
    max_width: Option<f64>,
    spec: &TextSpec,
    span: Span,
) -> Option<Open<'n>> {
    let chars: Vec<char> = words.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let n = chars.len() as u64;
    if let Some(limit) = ctx.text_length.limit {
        if n > limit {
            let message = past_limit(
                format!("icon '{}' sets {n} characters in one text here", ctx.icon),
                LimitKind::TextLength,
                limit,
                &format!("characters, {}", ctx.text_length.from),
            );
            ctx.exhausted = true;
            ctx.diag(span, message);
            return None;
        }
    }
    ctx.text_limit.1 += n;
    if let Some(limit) = ctx.text_limit.0.limit {
        if ctx.text_limit.1 > limit {
            let message = past_limit(
                format!(
                    "icon '{}' sets {} characters of text by here",
                    ctx.icon, ctx.text_limit.1
                ),
                LimitKind::TextLimit,
                limit,
                &format!("characters, {}", ctx.text_limit.0.from),
            );
            ctx.exhausted = true;
            ctx.diag(span, message);
            return None;
        }
    }
    for &c in &chars {
        if let Some(why) = crate::text::refusal(c) {
            ctx.diag(span, format!("text cannot set {}: {why}", describe_char(c)));
            return None;
        }
    }
    let font = font_instance(ctx, spec, span)?;

    let mut placed: Vec<Placed<'a>> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut found = None;
        for len in (1..=font.longest.min(chars.len() - i)).rev() {
            let piece: String = chars[i..i + len].iter().collect();
            if let Some(g) = font.glyphs.get(piece.as_str()) {
                found = Some((*g, len));
                break;
            }
        }
        match (found, font.notdef) {
            (Some((g, len)), _) => {
                placed.push(g);
                i += len;
            }
            (None, Some(g)) => {
                placed.push(g);
                i += 1;
            }
            (None, None) => {
                ctx.diag(
                    span,
                    format!(
                        "font '{}' has no glyph for {}, and no notdef to draw in its place",
                        font.name,
                        describe_char(chars[i])
                    ),
                );
                return None;
            }
        }
    }

    let mut pens = Vec::with_capacity(placed.len());
    let mut pen = 0.0;
    for (j, g) in placed.iter().enumerate() {
        pens.push(pen);
        pen += g.advance;
        if let (Some(a), Some(b)) = (g.name, placed.get(j + 1).and_then(|h| h.name)) {
            if let Some(k) = font.kerns.get(&(a, b)) {
                pen += k;
            }
        }
    }
    let width = pen;
    let mut k = size / font.em;
    if let Some(most) = max_width {
        if width > 0.0 && width * k > most {
            k = most / width;
        }
    }
    let dx = match spec.anchor {
        TextAnchor::Start => 0.0,
        TextAnchor::Middle => -width / 2.0,
        TextAnchor::End => -width,
    };
    let lift = match spec.baseline {
        Baseline::Alphabetic => 0.0,
        Baseline::Top => font.ascent * k,
        Baseline::Middle => (font.ascent - font.descent) / 2.0 * k,
        Baseline::Bottom => -font.descent * k,
    };
    use crate::affine;
    let m = affine::mul(
        &affine::translate(0.0, lift),
        &affine::mul(
            &affine::skew_x(font.slant),
            &affine::mul(&affine::scale(k, -k), &affine::translate(dx, 0.0)),
        ),
    );
    let outer = EvTransform {
        origin: [EvNum::Lit(0.0), EvNum::Lit(0.0)],
        steps: vec![
            EvStep::Matrix(Box::new(m.map(EvNum::Lit))),
            EvStep::Translate([x, y]),
        ],
        onto: None,
        bounds: spec.bounds,
        strokes: None,
        paint: spec.fill.clone(),
    };
    let glyphs: Vec<(f64, &'n [Node])> = placed
        .iter()
        .zip(pens)
        .filter(|(g, _)| !g.nodes.is_empty())
        .map(|(g, pen)| (pen, g.nodes))
        .collect();
    Some(Open::Text {
        outer,
        glyphs,
        font_env: font.env.clone(),
        span,
    })
}
