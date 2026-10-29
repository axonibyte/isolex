//! `isolex font export`: a font of a lexicon, in some of its styles, as a
//! standard font. Its glyphs are drawn by the compiler as text draws them
//! ([`crate::font_outline`]); outline glyphs keep their contours and
//! stroke-built ones are expanded to outlines; a slant is sheared in; and
//! the notices its `describe` lines carry go into the font's names.

use std::path::Path;

use crate::{FontOutline, GlyphOutline};

use super::outline::{self, cubic_to_quads, Contour, Seg};
use super::sfnt;

/// One glyph, ready to write: in font units, y up.
pub struct PGlyph {
    /// As the lexicon names it: the text it draws, or `.notdef`.
    pub name: String,
    pub unicode: Option<char>,
    pub advance: f64,
    pub contours: Vec<Contour>,
}

/// A font ready to write, in either format.
pub struct Prepared {
    pub family: String,
    pub subfamily: String,
    pub version: String,
    pub copyright: Option<String>,
    pub trademark: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub upm: u16,
    pub ascent: f64,
    pub descent: f64,
    pub italic_angle: f64,
    pub bold: bool,
    pub italic: bool,
    /// .notdef first.
    pub glyphs: Vec<PGlyph>,
    /// (left, right, space), by glyph index.
    pub kerns: Vec<(usize, usize, f64)>,
    /// (the glyphs it replaces, the ligature), by glyph index.
    pub ligatures: Vec<(Vec<usize>, usize)>,
    /// Font units of tolerance, for curves made quadratic.
    pub tolerance: f64,
}

impl Prepared {
    pub fn postscript_name(&self) -> String {
        format!("{}-{}", self.family, self.subfamily)
            .chars()
            .filter(|c| c.is_ascii_graphic() && !"[](){}<>/%".contains(*c))
            .take(63)
            .collect()
    }
}

/// The value of a `describe` line that starts `key: `, if one does.
fn notice(describe: &[String], key: &str) -> Option<String> {
    describe
        .iter()
        .find_map(|d| d.strip_prefix(key).and_then(|r| r.strip_prefix(": ")))
        .map(str::to_string)
}

/// A font's styles as a subfamily name: "Bold Italic", or "Regular".
fn subfamily(styles: &[String]) -> String {
    if styles.is_empty() {
        return "Regular".to_string();
    }
    styles
        .iter()
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The font's glyphs in font units of `upm` to the em, a slant sheared
/// in, curves flattened for strokes to within `tolerance` of an em.
pub fn prepare(
    f: &FontOutline,
    styles: &[String],
    upm: Option<u16>,
    tolerance: Option<f64>,
) -> Result<Prepared, String> {
    let upm = match upm {
        Some(u) => u,
        None if f.em.fract() == 0.0 && (16.0..=16384.0).contains(&f.em) => f.em as u16,
        None => 1000,
    };
    let scale = f64::from(upm) / f.em;
    let lean = -libm::tan(f.slant.to_radians());
    let m = [scale, 0.0, lean * scale, scale, 0.0, 0.0];
    let tol = tolerance.unwrap_or(f.em / 2000.0) * scale;

    let draw = |g: &GlyphOutline| -> Result<PGlyph, String> {
        let contours = outline::outlines(&g.shapes, &m, tol)
            .map_err(|e| format!("glyph \"{}\": {e}", g.name))?;
        let mut chars = g.name.chars();
        let unicode = match (chars.next(), chars.next()) {
            (Some(c), None) if g.name != ".notdef" => Some(c),
            _ => None,
        };
        Ok(PGlyph {
            name: g.name.clone(),
            unicode,
            advance: g.advance * scale,
            contours,
        })
    };
    let mut glyphs = vec![match &f.notdef {
        Some(g) => draw(g)?,
        None => PGlyph {
            name: ".notdef".to_string(),
            unicode: None,
            advance: f64::from(upm) / 2.0,
            contours: Vec::new(),
        },
    }];
    for g in &f.glyphs {
        glyphs.push(draw(g)?);
    }
    let index = |name: &str| glyphs.iter().position(|g| g.name == name);
    let kerns = f
        .kerns
        .iter()
        .filter_map(|(l, r, by)| Some((index(l)?, index(r)?, by * scale)))
        .collect();
    // a ligature, where each character it replaces has a glyph of its own
    let ligatures = glyphs
        .iter()
        .enumerate()
        .filter(|(_, g)| g.name.chars().count() > 1 && g.name != ".notdef")
        .filter_map(|(i, g)| {
            let comps: Option<Vec<usize>> = g.name.chars().map(|c| index(&c.to_string())).collect();
            Some((comps?, i))
        })
        .collect();
    let lower: Vec<String> = styles.iter().map(|s| s.to_lowercase()).collect();
    Ok(Prepared {
        family: notice(&f.describe, "Family").unwrap_or_else(|| f.name.clone()),
        subfamily: subfamily(styles),
        version: notice(&f.describe, "Version").unwrap_or_else(|| "Version 1.000".to_string()),
        copyright: notice(&f.describe, "Copyright"),
        trademark: notice(&f.describe, "Trademark"),
        license: notice(&f.describe, "License"),
        license_url: notice(&f.describe, "License URL"),
        upm,
        ascent: f.ascent * scale,
        descent: f.descent * scale,
        italic_angle: f.slant,
        bold: lower.iter().any(|s| s.contains("bold")),
        italic: f.slant != 0.0
            || lower
                .iter()
                .any(|s| s.contains("italic") || s.contains("oblique")),
        glyphs,
        kerns,
        ligatures,
        tolerance: tol,
    })
}

fn units(v: f64, what: &str) -> Result<i16, String> {
    let r = v.round();
    if !(f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&r) {
        return Err(format!(
            "{what} reaches {v}, beyond what a TrueType font holds"
        ));
    }
    Ok(r as i16)
}

/// The font as a TrueType file: curves made quadratic, every point on
/// whole font units.
pub fn to_ttf(p: &Prepared) -> Result<Vec<u8>, String> {
    let mut glyphs = Vec::new();
    for g in &p.glyphs {
        let mut contours = Vec::new();
        for c in &g.contours {
            let mut pts: Vec<(i16, i16, bool)> = Vec::new();
            let what = format!("glyph \"{}\"", g.name);
            let mut push = |pt: [f64; 2], on: bool| -> Result<(), String> {
                let q = (units(pt[0], &what)?, units(pt[1], &what)?, on);
                // two points on the curve in the same place are one
                if !(on && pts.last().is_some_and(|l| l.2 && l.0 == q.0 && l.1 == q.1)) {
                    pts.push(q);
                }
                Ok(())
            };
            push(c.start, true)?;
            let mut cur = c.start;
            for s in &c.segs {
                match *s {
                    Seg::Line(e) => {
                        push(e, true)?;
                        cur = e;
                    }
                    Seg::Quad(q, e) => {
                        push(q, false)?;
                        push(e, true)?;
                        cur = e;
                    }
                    Seg::Cubic(a, b, e) => {
                        let mut quads = Vec::new();
                        cubic_to_quads(cur, a, b, e, p.tolerance.max(0.01), &mut quads);
                        for q in quads {
                            if let Seg::Quad(q, e) = q {
                                push(q, false)?;
                                push(e, true)?;
                            }
                        }
                        cur = e;
                    }
                }
            }
            // the contour closes by itself
            if pts.len() > 1 {
                let (f, l) = (pts[0], pts[pts.len() - 1]);
                if l.2 && f.0 == l.0 && f.1 == l.1 {
                    pts.pop();
                }
            }
            if pts.len() > 1 {
                contours.push(pts);
            }
        }
        glyphs.push(sfnt::Glyph {
            advance: g.advance.round().clamp(0.0, 65535.0) as u16,
            contours,
        });
    }
    let mut cmap: Vec<(u32, u16)> = p
        .glyphs
        .iter()
        .enumerate()
        .filter_map(|(i, g)| g.unicode.map(|c| (c as u32, i as u16)))
        .collect();
    cmap.sort_unstable();
    let font = sfnt::Font {
        family: p.family.clone(),
        subfamily: p.subfamily.clone(),
        version: p.version.clone(),
        copyright: p.copyright.clone(),
        trademark: p.trademark.clone(),
        license: p.license.clone(),
        license_url: p.license_url.clone(),
        upm: p.upm,
        ascent: units(p.ascent, "the ascent")?,
        descent: units(p.descent, "the descent")?,
        italic_angle: p.italic_angle,
        bold: p.bold,
        italic: p.italic,
        glyphs,
        cmap,
        kerns: p
            .kerns
            .iter()
            .filter_map(|&(l, r, by)| {
                let v = by.round();
                (v != 0.0).then_some((l as u16, r as u16, v.clamp(-32768.0, 32767.0) as i16))
            })
            .collect(),
        ligatures: p
            .ligatures
            .iter()
            .map(|(c, l)| (c.iter().map(|i| *i as u16).collect(), *l as u16))
            .collect(),
    };
    Ok(sfnt::write(&font))
}

/// `isolex font export`: the font named, in the styles named, to `out`, a
/// TrueType file or a new UFO directory.
pub fn export(
    files: &[(String, String)],
    font: &str,
    styles: &[String],
    out: &Path,
    ufo: bool,
    upm: Option<u16>,
    tolerance: Option<f64>,
) -> Result<String, Vec<String>> {
    let outline = crate::font_outline(files, font, styles).map_err(|ds| {
        ds.iter()
            .map(|d| {
                if d.file.is_empty() {
                    d.message.clone()
                } else {
                    d.to_string()
                }
            })
            .collect::<Vec<_>>()
    })?;
    let prepared = prepare(&outline, styles, upm, tolerance).map_err(|e| vec![e])?;
    if ufo {
        super::ufo::write(&prepared, out).map_err(|e| vec![e])?;
    } else {
        let bytes = to_ttf(&prepared).map_err(|e| vec![e])?;
        std::fs::write(out, bytes)
            .map_err(|e| vec![format!("cannot write {}: {e}", out.display())])?;
    }
    Ok(format!(
        "exported font '{font}' ({}, {} glyph(s)) to {}",
        prepared.subfamily,
        prepared.glyphs.len(),
        out.display()
    ))
}
