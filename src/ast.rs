//! The Isolex AST: what the parser produces and the resolver consumes.
//!
//! Everything is `Serialize` so parser golden tests are stable JSON, and
//! every construct carries a span so later stages can blame source
//! locations without re-parsing.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::expr::Expr;

/// Byte range within one source file (index into `Lexicon::files`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub file: usize,
    pub start: usize,
    pub len: usize,
}

/// A numeric position: literal or expression (source kept for goldens
/// and diagnostics).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Num {
    Lit(f64),
    Expr { src: String, ast: Expr },
}

/// Common paint properties. All optional; nodes apply their own
/// defaults at projection time (spec'd per node).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Style {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub width: Option<Num>,
    pub dash: Option<String>,
    pub opacity: Option<Num>,
}

/// A geometry node. Repeats, `at`, polygons and poly3d hold children,
/// and a lexicon may nest them to any depth (#28), so its `Clone` and
/// `Drop` are written out without recursion; the derived `Debug`,
/// `PartialEq` and `Serialize` recurse, and are for tests and goldens
/// only, which nest shallowly.
#[derive(Debug, PartialEq, Serialize)]
pub enum Node {
    /// 3-D — legal outside `at`
    Box3 {
        args: [Num; 6], // x y z w d h
        fills: Option<String>,
        opacity: Option<Num>,
        when: Option<Num>,
        span: Span,
    },
    Tile {
        args: [Num; 5], // x y z w d
        style: Style,
        when: Option<Num>,
        span: Span,
    },
    Cylinder {
        args: [Num; 5], // cx cy z r h
        top: Option<String>,
        side: Option<String>,
        opacity: Option<Num>,
        when: Option<Num>,
        span: Span,
    },
    FaceRect {
        face: Face,
        args: [Num; 5], // a b z extent h (front: x y z w h; right: x y z d h)
        fill: Option<String>,
        opacity: Option<Num>,
        when: Option<Num>,
        span: Span,
    },
    Poly3d {
        style: Style,
        children: Vec<Node>, // Pt3 and Repeat only
        when: Option<Num>,
        span: Span,
    },
    Shadow {
        args: [Num; 4], // cx cy rx ry
        when: Option<Num>,
        span: Span,
    },
    /// Two-point segment in tile space; both endpoints project via P.
    Seg {
        args: [Num; 6], // ax ay az bx by bz
        style: Style,
        when: Option<Num>,
        span: Span,
    },
    At {
        args: [Num; 3],      // x y z — binds anchor names `x`,`y` in children
        children: Vec<Node>, // 2-D nodes
        when: Option<Num>,
        span: Span,
    },

    /// 2-D — legal only inside `at`
    Line {
        args: [Num; 4],
        style: Style,
        when: Option<Num>,
        span: Span,
    },
    Circle {
        args: [Num; 3],
        style: Style,
        when: Option<Num>,
        span: Span,
    },
    Ellipse {
        args: [Num; 4],
        style: Style,
        when: Option<Num>,
        span: Span,
    },
    Rect2d {
        args: [Num; 4],
        rx: Option<Num>,
        style: Style,
        when: Option<Num>,
        span: Span,
    },
    Arc {
        args: [Num; 5], // cx cy r a0 a1 (degrees)
        style: Style,
        when: Option<Num>,
        span: Span,
    },
    Path {
        template: Vec<PathPiece>,
        style: Style,
        when: Option<Num>,
        span: Span,
    },
    Poly2d {
        style: Style,
        children: Vec<Node>, // Pt2 and Repeat only
        when: Option<Num>,
        span: Span,
    },

    /// context-polymorphic
    Pt3 {
        args: [Num; 3],
        span: Span,
    },
    Pt2 {
        args: [Num; 2],
        span: Span,
    },
    Repeat {
        count: Num,
        var: String,
        children: Vec<Node>,
        when: Option<Num>,
        span: Span,
    },
    Insert {
        fragment: String,
        args: Vec<(String, Num)>,
        when: Option<Num>,
        span: Span,
    },
    Badge {
        icon: String,
        corner: String,
        scale: Option<Num>,
        when: Option<Num>,
        span: Span,
    },

    /// A screen-space transform around its children, in either dimension:
    /// they are drawn through its matrix, in a group of their own.
    Transform {
        spec: TransformSpec,
        children: Vec<Node>,
        when: Option<Num>,
        span: Span,
    },
    /// A tile-space quarter-turn or diagonal mirror of 3-D children,
    /// applied before projection.
    Turn {
        by: TurnBy,
        origin: Option<[Num; 2]>,
        children: Vec<Node>,
        when: Option<Num>,
        span: Span,
    },
}

/// What a `transform` does: its steps, applied to its shapes in order,
/// each but `translate` and `matrix` about its origin; then, with `onto`,
/// the map that lays the result on a face of a box.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TransformSpec {
    pub origin: Option<[Num; 2]>,
    pub steps: Vec<Step>,
    pub onto: Option<(OntoFace, [Num; 6])>,
    pub bounds: Option<Bounds>,
    pub strokes: Option<Strokes>,
}

/// One step of a transform. Angles are degrees, clockwise on the screen
/// as SVG turns them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Step {
    Translate([Num; 2]),
    Rotate(Num),
    Scale([Num; 2]),
    SkewX(Num),
    SkewY(Num),
    Mirror(MirrorAxis),
    /// a b c d e f, as SVG's matrix(); applied as written, not about the
    /// origin
    Matrix(Box<[Num; 6]>),
}

impl Step {
    pub fn name(&self) -> &'static str {
        match self {
            Step::Translate(_) => "translate",
            Step::Rotate(_) => "rotate",
            Step::Scale(_) => "scale",
            Step::SkewX(_) => "skew-x",
            Step::SkewY(_) => "skew-y",
            Step::Mirror(_) => "mirror",
            Step::Matrix(_) => "matrix",
        }
    }
}

/// `mirror="across"` flips left for right; `"down"` top for bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MirrorAxis {
    Across,
    Down,
}

/// The face of a box `onto` lays flat content on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OntoFace {
    Top,
    Front,
    Right,
}

/// What a transform does when what it draws leaves the icon's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Bounds {
    Error,
    Grow,
    Scale,
    Clip,
}

impl Bounds {
    pub const NAMES: [&'static str; 4] = ["error", "grow", "scale", "clip"];

    pub fn named(name: &str) -> Option<Bounds> {
        match name {
            "error" => Some(Bounds::Error),
            "grow" => Some(Bounds::Grow),
            "scale" => Some(Bounds::Scale),
            "clip" => Some(Bounds::Clip),
            _ => None,
        }
    }
}

/// Whether strokes scale with a transform (the default) or keep the
/// width written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Strokes {
    Scale,
    Fixed,
}

/// A tile-space turn: a quarter-turn by an angle, or a mirror across the
/// diagonal x = y.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TurnBy {
    Angle(Num),
    MirrorDiagonal,
}

impl Node {
    /// Its children: those of a repeat, `at`, polygon or poly3d, and
    /// none for the rest.
    pub fn children(&self) -> &[Node] {
        match self {
            Node::Poly3d { children, .. }
            | Node::At { children, .. }
            | Node::Poly2d { children, .. }
            | Node::Repeat { children, .. }
            | Node::Transform { children, .. }
            | Node::Turn { children, .. } => children,
            _ => &[],
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Node::Poly3d { children, .. }
            | Node::At { children, .. }
            | Node::Poly2d { children, .. }
            | Node::Repeat { children, .. }
            | Node::Transform { children, .. }
            | Node::Turn { children, .. } => Some(children),
            _ => None,
        }
    }

    /// A copy of everything but its children.
    fn shallow_clone(&self) -> Node {
        macro_rules! copy {
            ($($v:ident { $($f:ident),* })* ; $($cv:ident { $($cf:ident),* })*) => {
                match self {
                    $(Node::$v { $($f),* } => Node::$v { $($f: $f.clone()),* },)*
                    $(Node::$cv { $($cf,)* .. } => Node::$cv { children: Vec::new(), $($cf: $cf.clone()),* },)*
                }
            };
        }
        copy!(
            Box3 { args, fills, opacity, when, span }
            Tile { args, style, when, span }
            Cylinder { args, top, side, opacity, when, span }
            FaceRect { face, args, fill, opacity, when, span }
            Shadow { args, when, span }
            Seg { args, style, when, span }
            Line { args, style, when, span }
            Circle { args, style, when, span }
            Ellipse { args, style, when, span }
            Rect2d { args, rx, style, when, span }
            Arc { args, style, when, span }
            Path { template, style, when, span }
            Pt3 { args, span }
            Pt2 { args, span }
            Insert { fragment, args, when, span }
            Badge { icon, corner, scale, when, span }
            ;
            Poly3d { style, when, span }
            At { args, when, span }
            Poly2d { style, when, span }
            Repeat { count, var, when, span }
            Transform { spec, when, span }
            Turn { by, origin, when, span }
        )
    }
}

impl Clone for Node {
    /// Without recursion: the nodes in post-order, each rebuilt once its
    /// children have been.
    fn clone(&self) -> Node {
        let mut order: Vec<&Node> = Vec::new();
        let mut todo: Vec<(&Node, bool)> = vec![(self, false)];
        while let Some((node, children_done)) = todo.pop() {
            if children_done {
                order.push(node);
            } else {
                todo.push((node, true));
                todo.extend(node.children().iter().rev().map(|c| (c, false)));
            }
        }
        let mut built: Vec<Node> = Vec::new();
        for node in order {
            let children = built.split_off(built.len() - node.children().len());
            let mut copy = node.shallow_clone();
            if let Some(slot) = copy.children_mut() {
                *slot = children;
            }
            built.push(copy);
        }
        built.pop().expect("the node itself")
    }
}

impl Drop for Node {
    /// Without recursion: every descendant moved onto one list and
    /// dropped from there, childless.
    fn drop(&mut self) {
        let Some(children) = self.children_mut() else {
            return;
        };
        let mut todo = std::mem::take(children);
        while let Some(mut node) = todo.pop() {
            if let Some(children) = node.children_mut() {
                todo.append(children);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Face {
    Front,
    Right,
}

/// `path d="M{x-10},{y} h20"` splits into literal and expression pieces.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PathPiece {
    Lit(String),
    Expr { src: String, ast: Expr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DefKind {
    Base,
    Icon,
    Fragment,
    Axis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PartOp {
    Add,
    Replace,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PartDecl {
    pub op: PartOp,
    pub name: String,
    pub nodes: Vec<Node>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Grant {
    pub token: String,
    pub reason: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Variant {
    pub name: String,
    pub body: DefBody,
    pub span: Span,
}

// The defaults, each overridable when the compiler is built, as an
// #ifndef would be in C:
//
//     ISOLEX_DEFAULT_SHAPE_LIMIT=500000 cargo build --release
//
// A value that is not a positive whole number stops the build. SPEC §9
// gives the values used when nothing is set; a lexicon that needs more
// should say so with a directive rather than rely on a custom build.
pub const DEFAULT_SHAPE_LIMIT: u64 = build_default(
    option_env!("ISOLEX_DEFAULT_SHAPE_LIMIT"),
    100_000,
    "ISOLEX_DEFAULT_SHAPE_LIMIT must be a positive whole number",
);
pub const DEFAULT_REPEAT_LIMIT: u64 = build_default(
    option_env!("ISOLEX_DEFAULT_REPEAT_LIMIT"),
    1_000_000,
    "ISOLEX_DEFAULT_REPEAT_LIMIT must be a positive whole number",
);
pub const DEFAULT_INSERT_DEPTH: u64 = build_default(
    option_env!("ISOLEX_DEFAULT_INSERT_DEPTH"),
    32,
    "ISOLEX_DEFAULT_INSERT_DEPTH must be a positive whole number",
);
pub const DEFAULT_ICON_LIMIT: u64 = build_default(
    option_env!("ISOLEX_DEFAULT_ICON_LIMIT"),
    100_000,
    "ISOLEX_DEFAULT_ICON_LIMIT must be a positive whole number",
);
pub const DEFAULT_EXPRESSION_DEPTH: u64 = build_default(
    option_env!("ISOLEX_DEFAULT_EXPRESSION_DEPTH"),
    256,
    "ISOLEX_DEFAULT_EXPRESSION_DEPTH must be a positive whole number",
);
pub const DEFAULT_TRANSFORM_CHAIN: u64 = build_default(
    option_env!("ISOLEX_DEFAULT_TRANSFORM_CHAIN"),
    64,
    "ISOLEX_DEFAULT_TRANSFORM_CHAIN must be a positive whole number",
);

const fn build_default(set: Option<&str>, fallback: u64, complaint: &str) -> u64 {
    match set {
        None => fallback,
        Some(s) => match u64::from_str_radix(s, 10) {
            Ok(n) if n > 0 => n,
            _ => panic!("{}", complaint),
        },
    }
}

/// A resource limit (SPEC §9). Each has a generous default and can be
/// raised, lowered, or switched off: the defaults keep a mistake from
/// exhausting the machine, and the directives let an author who means
/// it say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum LimitKind {
    /// Shapes and polygon points one icon emits.
    #[serde(rename = "shape-limit")]
    Shapes,
    /// Repeat iterations in one icon, every loop together.
    #[serde(rename = "repeat-limit")]
    Repeats,
    /// How deeply fragment insertions nest.
    #[serde(rename = "insert-depth")]
    InsertDepth,
    /// Icons the whole lexicon emits, after axis products.
    #[serde(rename = "icon-limit")]
    Icons,
    /// How deeply one expression nests.
    #[serde(rename = "expression-depth")]
    ExpressionDepth,
    /// How many transform steps one shape is drawn through.
    #[serde(rename = "transform-chain")]
    TransformChain,
}

impl LimitKind {
    pub const ALL: [LimitKind; 6] = [
        LimitKind::Shapes,
        LimitKind::Repeats,
        LimitKind::InsertDepth,
        LimitKind::Icons,
        LimitKind::ExpressionDepth,
        LimitKind::TransformChain,
    ];

    /// The directive that sets it.
    pub fn directive(self) -> &'static str {
        match self {
            LimitKind::Shapes => "shape-limit",
            LimitKind::Repeats => "repeat-limit",
            LimitKind::InsertDepth => "insert-depth",
            LimitKind::Icons => "icon-limit",
            LimitKind::ExpressionDepth => "expression-depth",
            LimitKind::TransformChain => "transform-chain",
        }
    }

    /// The limit when no directive sets one: one of the DEFAULT_*
    /// constants below, fixed when the compiler is built.
    pub fn default_limit(self) -> u64 {
        match self {
            LimitKind::Shapes => DEFAULT_SHAPE_LIMIT,
            LimitKind::Repeats => DEFAULT_REPEAT_LIMIT,
            LimitKind::InsertDepth => DEFAULT_INSERT_DEPTH,
            LimitKind::Icons => DEFAULT_ICON_LIMIT,
            LimitKind::ExpressionDepth => DEFAULT_EXPRESSION_DEPTH,
            LimitKind::TransformChain => DEFAULT_TRANSFORM_CHAIN,
        }
    }

    /// Only the lexicon block can set it: it counts the whole lexicon.
    pub fn lexicon_wide(self) -> bool {
        self == LimitKind::Icons
    }
}

/// A limit as written: a positive whole number, or `#null` for none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Limit {
    At(u64),
    Unlimited,
}

pub type Limits = BTreeMap<LimitKind, Limit>;

/// The shared body shape of base/icon/fragment definitions and axis
/// variants: the things inheritance merges.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DefBody {
    pub category: Option<String>,
    pub footprint: Option<[Num; 3]>,
    pub canvas: Option<[Num; 2]>,
    pub describe: Vec<String>,
    pub params: Vec<(String, Num, Span)>,
    pub axes: Vec<String>,
    pub grants: Vec<Grant>,
    pub parts: Vec<PartDecl>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: Limits,
    /// The corner the icon is drawn from: a quarter-turn of everything in
    /// its footprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<Num>,
    /// What its transforms do when they leave its frame, unless one says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform_bounds: Option<Bounds>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Definition {
    pub kind: DefKind,
    pub name: String,
    pub extends: Option<String>,
    pub body: DefBody,
    /// Axis definitions only.
    pub variants: Vec<Variant>,
    /// Axis definitions only; default true.
    pub emit_base: bool,
    pub span: Span,
}

/// Pack-level configuration from the `lexicon` node.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LexiconConfig {
    pub tokens: Vec<String>,
    pub stroke_widths: Vec<f64>,
    /// Tokens a definition may paint with only under a `grant`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reserved: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: Limits,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Lexicon {
    /// Source file paths, in sorted order — `Span::file` indexes this.
    pub files: Vec<String>,
    pub config: Option<LexiconConfig>,
    pub defs: Vec<Definition>,
}
