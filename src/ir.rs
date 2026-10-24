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

/// The formatting rule (SPEC §7), the one formatter for every number a
/// toolchain prints into output text: path data at compile time,
/// coordinates at render time. Two decimal places, ties to even on the
/// number's exact binary value, which is what `{:.2}` does (0.125 is
/// 0.12, 0.375 is 0.38). Zero is written 0.00: a value that rounds to it
/// from below, or is -0.0, would otherwise print as -0.00.
pub fn format_number(v: f64) -> String {
    let s = format!("{v:.2}");
    if s == "-0.00" {
        "0.00".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::format_number;

    #[test]
    fn zero_is_never_negative() {
        for v in [-0.0, -0.001, -0.004_999, 0.0, 0.004] {
            assert_eq!(format_number(v), "0.00", "{v}");
        }
        // not zero: these round away from it
        assert_eq!(format_number(-0.005_001), "-0.01");
        assert_eq!(format_number(-0.01), "-0.01");
    }

    #[test]
    fn ties_go_to_even() {
        for (v, want) in [
            (0.125, "0.12"),
            (0.375, "0.38"),
            (-0.125, "-0.12"),
            (0.145, "0.14"),
        ] {
            assert_eq!(format_number(v), want, "{v}");
        }
    }
}
