//! The reference renderer: shape IR to SVG (SPEC §7). It is the half of
//! the formatting rule the compiler does not apply, numbers printed at
//! render time, made concrete: every number this writes has exactly two
//! decimal places, ties to even, as `{:.2}` formats them. Path data is
//! already formatted and passes through unchanged; a badge is a group
//! with a translate and scale, its target's shapes untouched.
//!
//! Theme tokens become colors through a palette: a built-in one for the
//! tokens the compiler paints with by default and a few common ones,
//! which a lexicon's own palette extends or overrides.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::ir::{IrIcon, IrShape};

/// Theme token to color, as CSS writes colors.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette(BTreeMap<String, String>);

impl Default for Palette {
    /// Placeholder colors for the tokens the compiler fills in by default
    /// (SPEC §4) and a few common ones, so a lexicon renders out of the
    /// box; a real palette comes from the lexicon's own palette file.
    fn default() -> Self {
        Palette(
            [
                ("body-top", "#e4e7eb"),
                ("body-left", "#cbd2d9"),
                ("body-right", "#9aa5b1"),
                ("outline", "#1f2933"),
                ("ink", "#1f2933"),
                ("ink-muted", "#7b8794"),
                ("shadow", "#000000"),
                ("virt", "#d9e2ec"),
                ("accent", "#d64545"),
                ("accent-dim", "#e8a0a0"),
            ]
            .into_iter()
            .map(|(t, c)| (t.to_string(), c.to_string()))
            .collect(),
        )
    }
}

impl Palette {
    /// The built-in palette with a palette file's JSON object of token to
    /// color laid over it.
    pub fn with_json(text: &str) -> Result<Palette, String> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("the palette is not JSON: {e}"))?;
        let Some(object) = value.as_object() else {
            return Err("the palette must be a JSON object of token to color".to_string());
        };
        let mut palette = Palette::default();
        for (token, color) in object {
            let Some(color) = color.as_str() else {
                return Err(format!(
                    "the palette's color for '{token}' must be a string"
                ));
            };
            palette.0.insert(token.clone(), color.to_string());
        }
        Ok(palette)
    }
}

/// The two-decimal rule (SPEC §7), the one formatter the compiler writes
/// path data with too.
pub use crate::ir::format_number as fmt;

/// An icon name used as a file name must stay one: no path separators, no
/// leading dot, nothing a filesystem treats specially.
pub fn safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Every token the icons paint with that the palette has no color for,
/// with the icons that use it. `none` is no paint, never looked up.
pub fn missing_colors(icons: &[IrIcon], palette: &Palette) -> BTreeMap<String, BTreeSet<String>> {
    fn walk(
        shapes: &[IrShape],
        icon: &str,
        palette: &Palette,
        out: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        for shape in shapes {
            let (fill, stroke) = match shape {
                IrShape::Polygon { fill, stroke, .. }
                | IrShape::Path { fill, stroke, .. }
                | IrShape::Ellipse { fill, stroke, .. }
                | IrShape::Circle { fill, stroke, .. }
                | IrShape::Rect { fill, stroke, .. } => (fill, stroke),
                IrShape::Group { shapes, .. } => {
                    walk(shapes, icon, palette, out);
                    continue;
                }
            };
            for token in [fill, stroke] {
                if token != "none" && !palette.0.contains_key(token) {
                    out.entry(token.clone())
                        .or_default()
                        .insert(icon.to_string());
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for icon in icons {
        walk(&icon.shapes, &icon.name, palette, &mut out);
    }
    out
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// One icon as a standalone SVG document. Every token it paints with
/// must be in the palette ([`missing_colors`] says which are not).
pub fn render_icon(icon: &IrIcon, palette: &Palette) -> String {
    let [minx, miny, w, h] = icon.viewbox;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" width=\"{}\" height=\"{}\">",
        fmt(minx),
        fmt(miny),
        fmt(w),
        fmt(h),
        fmt(w),
        fmt(h)
    );
    let _ = writeln!(out, "<title>{}</title>", escape(&icon.name));
    shapes(&mut out, &icon.shapes, palette, 1);
    out.push_str("</svg>\n");
    out
}

fn shapes(out: &mut String, shapes_: &[IrShape], palette: &Palette, depth: usize) {
    let indent = "  ".repeat(depth);
    for shape in shapes_ {
        match shape {
            IrShape::Group {
                tx,
                ty,
                scale,
                shapes: inner,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}<g transform=\"translate({} {}) scale({})\">",
                    fmt(*tx),
                    fmt(*ty),
                    fmt(*scale)
                );
                shapes(out, inner, palette, depth + 1);
                let _ = writeln!(out, "{indent}</g>");
            }
            IrShape::Polygon {
                points,
                fill,
                fill_opacity,
                stroke,
                stroke_width,
                dash,
            } => {
                let points: Vec<String> = points
                    .iter()
                    .map(|[x, y]| format!("{},{}", fmt(*x), fmt(*y)))
                    .collect();
                let _ = writeln!(
                    out,
                    "{indent}<polygon points=\"{}\"{}/>",
                    points.join(" "),
                    paint(palette, fill, *fill_opacity, stroke, *stroke_width, dash)
                );
            }
            IrShape::Path {
                d,
                fill,
                fill_opacity,
                stroke,
                stroke_width,
                dash,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}<path d=\"{}\"{}/>",
                    escape(d),
                    paint(palette, fill, *fill_opacity, stroke, *stroke_width, dash)
                );
            }
            IrShape::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill,
                fill_opacity,
                stroke,
                stroke_width,
                dash,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{}/>",
                    fmt(*cx),
                    fmt(*cy),
                    fmt(*rx),
                    fmt(*ry),
                    paint(palette, fill, *fill_opacity, stroke, *stroke_width, dash)
                );
            }
            IrShape::Circle {
                cx,
                cy,
                r,
                fill,
                fill_opacity,
                stroke,
                stroke_width,
                dash,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{}/>",
                    fmt(*cx),
                    fmt(*cy),
                    fmt(*r),
                    paint(palette, fill, *fill_opacity, stroke, *stroke_width, dash)
                );
            }
            IrShape::Rect {
                x,
                y,
                w,
                h,
                rx,
                fill,
                fill_opacity,
                stroke,
                stroke_width,
                dash,
            } => {
                let rx = if *rx > 0.0 {
                    format!(" rx=\"{}\"", fmt(*rx))
                } else {
                    String::new()
                };
                let _ = writeln!(
                    out,
                    "{indent}<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx}{}/>",
                    fmt(*x),
                    fmt(*y),
                    fmt(*w),
                    fmt(*h),
                    paint(palette, fill, *fill_opacity, stroke, *stroke_width, dash)
                );
            }
        }
    }
}

/// Fill and stroke attributes, always in this order; an opacity of one,
/// and a stroke's width and dash where there is no stroke, are left out.
fn paint(
    palette: &Palette,
    fill: &str,
    opacity: f64,
    stroke: &str,
    width: f64,
    dash: &Option<String>,
) -> String {
    let color = |token: &str| -> String {
        if token == "none" {
            "none".to_string()
        } else {
            escape(palette.0.get(token).map(|c| c.as_str()).unwrap_or(token))
        }
    };
    let mut out = format!(" fill=\"{}\"", color(fill));
    if fill != "none" && opacity != 1.0 {
        let _ = write!(out, " fill-opacity=\"{}\"", fmt(opacity));
    }
    let _ = write!(out, " stroke=\"{}\"", color(stroke));
    if stroke != "none" {
        let _ = write!(out, " stroke-width=\"{}\"", fmt(width));
        if opacity != 1.0 {
            let _ = write!(out, " stroke-opacity=\"{}\"", fmt(opacity));
        }
        if let Some(dash) = dash {
            let _ = write!(out, " stroke-dasharray=\"{}\"", escape(dash));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_take_two_decimals_ties_to_even() {
        // SPEC §7, as a second implementation must reproduce it
        for (v, want) in [
            (0.125, "0.12"),
            (0.375, "0.38"),
            (0.625, "0.62"),
            (0.875, "0.88"),
            (-0.125, "-0.12"),
            (2.5, "2.50"),
            // not ties: 0.145 is 0.14499999… in binary, 0.135 0.13500…01
            (0.145, "0.14"),
            (0.135, "0.14"),
            // one tile along x, projected
            (crate::project::p(1.0, 0.0, 0.0).0, "34.64"),
        ] {
            assert_eq!(fmt(v), want, "{v}");
        }
    }

    #[test]
    fn icon_names_that_are_not_safe_file_names() {
        for good in ["switch-l3", "srv-down", "state_ok", "v1.2"] {
            assert!(safe_file_name(good), "{good}");
        }
        for bad in ["", "../x", "a/b", "a\\b", ".hidden", "c:x", "sp ace", "ünï"] {
            assert!(!safe_file_name(bad), "{bad}");
        }
    }

    #[test]
    fn each_shape_is_written_the_same_way_every_time() {
        let icon = IrIcon {
            name: "t".to_string(),
            category: "c".to_string(),
            footprint: None,
            canvas: Some([24.0, 24.0]),
            viewbox: [0.0, 0.0, 24.0, 24.0],
            describe: Vec::new(),
            grants: Vec::new(),
            shapes: vec![
                IrShape::Polygon {
                    points: vec![[0.125, 1.0], [2.0, 3.375]],
                    fill: "accent".to_string(),
                    fill_opacity: 0.55,
                    stroke: "none".to_string(),
                    stroke_width: 0.0,
                    dash: None,
                },
                IrShape::Path {
                    d: "M1.00,2.00 L3.00,4.00".to_string(),
                    fill: "none".to_string(),
                    fill_opacity: 1.0,
                    stroke: "ink".to_string(),
                    stroke_width: 1.4,
                    dash: Some("3 2.5".to_string()),
                },
                IrShape::Group {
                    tx: 10.0,
                    ty: -2.0,
                    scale: 0.5,
                    shapes: vec![IrShape::Rect {
                        x: 3.0,
                        y: 3.0,
                        w: 18.0,
                        h: 18.0,
                        rx: 3.0,
                        fill: "none".to_string(),
                        fill_opacity: 1.0,
                        stroke: "ink-muted".to_string(),
                        stroke_width: 1.5,
                        dash: None,
                    }],
                },
            ],
        };
        assert_eq!(
            render_icon(&icon, &Palette::default()),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0.00 0.00 24.00 24.00\" width=\"24.00\" height=\"24.00\">\n\
             <title>t</title>\n\
             \x20 <polygon points=\"0.12,1.00 2.00,3.38\" fill=\"#d64545\" fill-opacity=\"0.55\" stroke=\"none\"/>\n\
             \x20 <path d=\"M1.00,2.00 L3.00,4.00\" fill=\"none\" stroke=\"#1f2933\" stroke-width=\"1.40\" stroke-dasharray=\"3 2.5\"/>\n\
             \x20 <g transform=\"translate(10.00 -2.00) scale(0.50)\">\n\
             \x20   <rect x=\"3.00\" y=\"3.00\" width=\"18.00\" height=\"18.00\" rx=\"3.00\" fill=\"none\" stroke=\"#7b8794\" stroke-width=\"1.50\"/>\n\
             \x20 </g>\n\
             </svg>\n"
        );
    }

    #[test]
    fn a_palette_extends_and_overrides_the_built_in_one() {
        let p = Palette::with_json(r#"{"ink": "black", "brand": "rgb(200, 0, 0)"}"#).unwrap();
        assert_eq!(p.0["ink"], "black");
        assert_eq!(p.0["brand"], "rgb(200, 0, 0)");
        assert_eq!(p.0["outline"], "#1f2933", "the rest stays");
        assert!(Palette::with_json("[1]")
            .unwrap_err()
            .contains("JSON object"));
        assert!(Palette::with_json(r#"{"ink": 3}"#)
            .unwrap_err()
            .contains("'ink'"));
        assert!(Palette::with_json("{").unwrap_err().contains("not JSON"));
    }

    #[test]
    fn colors_are_escaped() {
        let p = Palette::with_json(r#"{"ink": "\"><script>"}"#).unwrap();
        let out = paint(&p, "ink", 1.0, "none", 0.0, &None);
        assert_eq!(out, " fill=\"&quot;&gt;&lt;script&gt;\" stroke=\"none\"");
    }
}
