//! The shape IR: what `isolex compile` emits. Field names and defaults
//! mirror the primitive dictionaries consumers render; everything
//! positional is f64 (renderers apply the normative fixed-two-decimal
//! formatting), everything painted is a theme token left unresolved.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IrShape {
    Polygon {
        points: Vec<[f64; 2]>,
        fill: String,
        fill_opacity: f64,
        stroke: String,
        stroke_width: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        dash: Option<String>,
    },
    Path {
        d: String,
        fill: String,
        fill_opacity: f64,
        stroke: String,
        stroke_width: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        dash: Option<String>,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        fill: String,
        fill_opacity: f64,
        stroke: String,
        stroke_width: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        dash: Option<String>,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: String,
        fill_opacity: f64,
        stroke: String,
        stroke_width: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        dash: Option<String>,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rx: f64,
        fill: String,
        fill_opacity: f64,
        stroke: String,
        stroke_width: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        dash: Option<String>,
    },
    /// A placed sub-icon (badge composition). Renderers wrap the child
    /// shapes in a group with the given translate/scale — path data is
    /// never rewritten.
    Group {
        tx: f64,
        ty: f64,
        scale: f64,
        shapes: Vec<IrShape>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct IrIcon {
    pub name: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footprint: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvas: Option<[f64; 2]>,
    /// minx, miny, width, height — the frame renderers should use.
    pub viewbox: [f64; 4],
    pub describe: Vec<String>,
    pub grants: Vec<(String, String)>,
    pub shapes: Vec<IrShape>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IrDocument {
    /// Format version for consumers; bumped on breaking IR changes.
    pub ir_version: u32,
    pub icons: Vec<IrIcon>,
}

pub const IR_VERSION: u32 = 1;
