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

#[derive(Debug, Clone, PartialEq, Serialize)]
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
}

impl LimitKind {
    pub const ALL: [LimitKind; 4] = [
        LimitKind::Shapes,
        LimitKind::Repeats,
        LimitKind::InsertDepth,
        LimitKind::Icons,
    ];

    /// The directive that sets it.
    pub fn directive(self) -> &'static str {
        match self {
            LimitKind::Shapes => "shape-limit",
            LimitKind::Repeats => "repeat-limit",
            LimitKind::InsertDepth => "insert-depth",
            LimitKind::Icons => "icon-limit",
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
