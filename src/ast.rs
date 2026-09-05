//! The Isolex AST: what the parser produces and the resolver consumes.
//!
//! Everything is `Serialize` so parser golden tests are stable JSON, and
//! every construct carries a span so later stages can blame source
//! locations without re-parsing.

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
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Lexicon {
    /// Source file paths, in sorted order — `Span::file` indexes this.
    pub files: Vec<String>,
    pub config: Option<LexiconConfig>,
    pub defs: Vec<Definition>,
}
