//! A glyph's shapes as a font holds them: closed contours, filled by the
//! nonzero rule. Fills are their shapes' outlines; strokes are expanded
//! to outlines of their own, as SVG draws them with no cap or join
//! attributes: butt caps, and miter joins up to a limit of 4, bevelled
//! past it.

use crate::affine::{self, Mat, Seg as PathSeg};
use crate::ir::IrShape;

pub type P = [f64; 2];

/// A segment from wherever the last one ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Line(P),
    /// control point, end
    Quad(P, P),
    /// two control points, end
    Cubic(P, P, P),
}

/// A closed contour: it starts at `start`, and after its last segment
/// returns there, by a line if it has not.
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    pub start: P,
    pub segs: Vec<Seg>,
}

fn end_of(s: &Seg) -> P {
    match s {
        Seg::Line(p) | Seg::Quad(_, p) | Seg::Cubic(_, _, p) => *p,
    }
}

fn lerp(a: P, b: P, t: f64) -> P {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn dist(a: P, b: P) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl Contour {
    pub fn map(&self, m: &Mat) -> Contour {
        let f = |p: P| affine::apply(m, p);
        Contour {
            start: f(self.start),
            segs: self
                .segs
                .iter()
                .map(|s| match *s {
                    Seg::Line(p) => Seg::Line(f(p)),
                    Seg::Quad(c, p) => Seg::Quad(f(c), f(p)),
                    Seg::Cubic(a, b, p) => Seg::Cubic(f(a), f(b), f(p)),
                })
                .collect(),
        }
    }

    /// Its points, curves flattened to within `tol`, the start once.
    pub fn points(&self, tol: f64) -> Vec<P> {
        let mut out = vec![self.start];
        let mut cur = self.start;
        for s in &self.segs {
            match *s {
                Seg::Line(p) => out.push(p),
                Seg::Quad(c, p) => {
                    let dd = dist(
                        [cur[0] - 2.0 * c[0] + p[0], cur[1] - 2.0 * c[1] + p[1]],
                        [0.0, 0.0],
                    );
                    let n = steps(0.25 * dd, tol);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        out.push(lerp(lerp(cur, c, t), lerp(c, p, t), t));
                    }
                }
                Seg::Cubic(a, b, p) => {
                    let d1 = dist(
                        [cur[0] - 2.0 * a[0] + b[0], cur[1] - 2.0 * a[1] + b[1]],
                        [0.0, 0.0],
                    );
                    let d2 = dist(
                        [a[0] - 2.0 * b[0] + p[0], a[1] - 2.0 * b[1] + p[1]],
                        [0.0, 0.0],
                    );
                    let n = steps(0.75 * d1.max(d2), tol);
                    for i in 1..=n {
                        out.push(cubic_at([cur, a, b, p], i as f64 / n as f64));
                    }
                }
            }
            cur = end_of(s);
        }
        if out.len() > 1 && dist(out[out.len() - 1], out[0]) < 1e-12 {
            out.pop();
        }
        out
    }

    /// Twice its signed area, by its flattened points: positive turns
    /// counter-clockwise with y up.
    pub fn area(&self, tol: f64) -> f64 {
        let p = self.points(tol);
        (0..p.len())
            .map(|i| {
                let (a, b) = (p[i], p[(i + 1) % p.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum()
    }

    /// The same contour, the other way round.
    pub fn reversed(&self) -> Contour {
        let mut pts = vec![self.start];
        for s in &self.segs {
            pts.push(end_of(s));
        }
        // close it explicitly, so each segment has both its ends
        let last = *pts.last().expect("the start");
        let mut segs = self.segs.clone();
        if dist(last, self.start) > 0.0 {
            segs.push(Seg::Line(self.start));
            pts.push(self.start);
        }
        let n = segs.len();
        let mut out = Vec::with_capacity(n);
        for i in (0..n).rev() {
            let to = pts[i];
            out.push(match segs[i] {
                Seg::Line(_) => Seg::Line(to),
                Seg::Quad(c, _) => Seg::Quad(c, to),
                Seg::Cubic(a, b, _) => Seg::Cubic(b, a, to),
            });
        }
        Contour {
            start: self.start,
            segs: out,
        }
    }
}

fn steps(dd: f64, tol: f64) -> usize {
    if dd <= 0.0 || tol <= 0.0 {
        return 1;
    }
    ((dd / tol).sqrt().ceil() as usize).clamp(1, 256)
}

pub fn cubic_at([p0, a, b, p]: [P; 4], t: f64) -> P {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [
        w0 * p0[0] + w1 * a[0] + w2 * b[0] + w3 * p[0],
        w0 * p0[1] + w1 * a[1] + w2 * b[1] + w3 * p[1],
    ]
}

/// A cubic as quadratics, each within `tol` of it, for TrueType outlines:
/// the usual single-quadratic estimate, halved until it is close enough.
pub fn cubic_to_quads(p0: P, a: P, b: P, p: P, tol: f64, out: &mut Vec<Seg>) {
    fn go(p0: P, a: P, b: P, p: P, tol: f64, depth: u32, out: &mut Vec<Seg>) {
        // how far the best single quadratic can stray from the cubic
        let e = [
            p[0] - 3.0 * b[0] + 3.0 * a[0] - p0[0],
            p[1] - 3.0 * b[1] + 3.0 * a[1] - p0[1],
        ];
        let err = 3f64.sqrt() / 36.0 * dist(e, [0.0, 0.0]);
        if err <= tol || depth >= 10 {
            let q = [
                (3.0 * (a[0] + b[0]) - p0[0] - p[0]) / 4.0,
                (3.0 * (a[1] + b[1]) - p0[1] - p[1]) / 4.0,
            ];
            out.push(Seg::Quad(q, p));
            return;
        }
        let (ab, bc, cd) = (lerp(p0, a, 0.5), lerp(a, b, 0.5), lerp(b, p, 0.5));
        let (abc, bcd) = (lerp(ab, bc, 0.5), lerp(bc, cd, 0.5));
        let mid = lerp(abc, bcd, 0.5);
        go(p0, ab, abc, mid, tol, depth + 1, out);
        go(mid, bcd, cd, p, tol, depth + 1, out);
    }
    go(p0, a, b, p, tol, 0, out);
}

/// Subpaths of path data: each a contour, and whether it was closed.
fn subpaths(segs: &[PathSeg]) -> Vec<(Contour, bool)> {
    let mut out: Vec<(Contour, bool)> = Vec::new();
    let mut open: Option<Contour> = None;
    let mut start = [0.0, 0.0];
    let mut cur = [0.0, 0.0];
    for s in segs {
        let seg = match *s {
            PathSeg::Move(p) => {
                if let Some(c) = open.take() {
                    out.push((c, false));
                }
                start = p;
                cur = p;
                continue;
            }
            PathSeg::Close => {
                if let Some(c) = open.take() {
                    out.push((c, true));
                }
                cur = start;
                continue;
            }
            PathSeg::Line(p) => Seg::Line(p),
            PathSeg::Quad(c, p) => Seg::Quad(c, p),
            PathSeg::Cubic(a, b, p) => Seg::Cubic(a, b, p),
        };
        let c = open.get_or_insert_with(|| Contour {
            start: cur,
            segs: Vec::new(),
        });
        c.segs.push(seg);
        cur = end_of(&seg);
    }
    if let Some(c) = open {
        out.push((c, false));
    }
    out
}

/// A cubic quarter of an ellipse's circumference, as a share of its radius.
const KAPPA: f64 = 0.552_284_749_830_793_4;

fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Contour {
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    Contour {
        start: [cx + rx, cy],
        segs: vec![
            Seg::Cubic([cx + rx, cy + ky], [cx + kx, cy + ry], [cx, cy + ry]),
            Seg::Cubic([cx - kx, cy + ry], [cx - rx, cy + ky], [cx - rx, cy]),
            Seg::Cubic([cx - rx, cy - ky], [cx - kx, cy - ry], [cx, cy - ry]),
            Seg::Cubic([cx + kx, cy - ry], [cx + rx, cy - ky], [cx + rx, cy]),
        ],
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64, rx: f64) -> Contour {
    let r = rx.max(0.0).min(w / 2.0).min(h / 2.0);
    if r == 0.0 {
        return Contour {
            start: [x, y],
            segs: vec![
                Seg::Line([x + w, y]),
                Seg::Line([x + w, y + h]),
                Seg::Line([x, y + h]),
                Seg::Line([x, y]),
            ],
        };
    }
    let k = r * (1.0 - KAPPA);
    Contour {
        start: [x + r, y],
        segs: vec![
            Seg::Line([x + w - r, y]),
            Seg::Cubic([x + w - k, y], [x + w, y + k], [x + w, y + r]),
            Seg::Line([x + w, y + h - r]),
            Seg::Cubic([x + w, y + h - k], [x + w - k, y + h], [x + w - r, y + h]),
            Seg::Line([x + r, y + h]),
            Seg::Cubic([x + k, y + h], [x, y + h - k], [x, y + h - r]),
            Seg::Line([x, y + r]),
            Seg::Cubic([x, y + k], [x + k, y], [x + r, y]),
        ],
    }
}

/// A shape's subpaths, each with whether it is closed, and its paints:
/// fill, stroke and stroke width.
type Geometry<'a> = (Vec<(Contour, bool)>, &'a str, &'a str, f64);

/// The geometry of one shape.
fn geometry(shape: &IrShape) -> Result<Geometry<'_>, String> {
    Ok(match shape {
        IrShape::Path {
            d,
            fill,
            stroke,
            stroke_width,
            ..
        } => (
            subpaths(&affine::path_segments(d)?),
            fill,
            stroke,
            *stroke_width,
        ),
        IrShape::Polygon {
            points,
            fill,
            stroke,
            stroke_width,
            ..
        } => {
            let geo = match points.split_first() {
                Some((first, rest)) if !rest.is_empty() => vec![(
                    Contour {
                        start: *first,
                        segs: rest.iter().map(|p| Seg::Line(*p)).collect(),
                    },
                    true,
                )],
                _ => Vec::new(),
            };
            (geo, fill, stroke, *stroke_width)
        }
        IrShape::Rect {
            x,
            y,
            w,
            h,
            rx,
            fill,
            stroke,
            stroke_width,
            ..
        } => (
            vec![(rect(*x, *y, *w, *h, *rx), true)],
            fill,
            stroke,
            *stroke_width,
        ),
        IrShape::Circle {
            cx,
            cy,
            r,
            fill,
            stroke,
            stroke_width,
            ..
        } => (
            vec![(ellipse(*cx, *cy, *r, *r), true)],
            fill,
            stroke,
            *stroke_width,
        ),
        IrShape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            fill,
            stroke,
            stroke_width,
            ..
        } => (
            vec![(ellipse(*cx, *cy, *rx, *ry), true)],
            fill,
            stroke,
            *stroke_width,
        ),
        IrShape::Group { .. } => unreachable!("groups are opened by the caller"),
    })
}

/// A stroke of width `w` along a subpath, as outlines: a quadrilateral
/// for each straight piece of it (curves flattened to within `tol`), and
/// at each corner, the wedge its outer side needs, mitered or bevelled.
fn stroke(c: &Contour, closed: bool, w: f64, tol: f64) -> Vec<Contour> {
    let mut pts: Vec<P> = Vec::new();
    for p in c.points(tol) {
        if pts.last().is_none_or(|q| dist(*q, p) > 1e-9) {
            pts.push(p);
        }
    }
    if closed && pts.len() > 2 && dist(pts[0], pts[pts.len() - 1]) <= 1e-9 {
        pts.pop();
    }
    if !closed && pts.len() > 1 {
        // an open subpath's flattened points do not come back round
        if let Some(last) = c.segs.last() {
            let e = end_of(last);
            if dist(*pts.last().unwrap(), e) > 1e-9 {
                pts.push(e);
            }
        }
    }
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let h = w / 2.0;
    let edges: Vec<(P, P)> = (0..n - 1)
        .map(|i| (pts[i], pts[i + 1]))
        .chain(closed.then(|| (pts[n - 1], pts[0])))
        .collect();
    let normal = |(a, b): (P, P)| {
        let l = dist(a, b);
        [-(b[1] - a[1]) / l, (b[0] - a[0]) / l]
    };
    let poly = |pts: Vec<P>| Contour {
        start: pts[0],
        segs: pts[1..].iter().map(|p| Seg::Line(*p)).collect(),
    };
    let mut out = Vec::new();
    for &(a, b) in &edges {
        let m = normal((a, b));
        out.push(poly(vec![
            [a[0] + m[0] * h, a[1] + m[1] * h],
            [b[0] + m[0] * h, b[1] + m[1] * h],
            [b[0] - m[0] * h, b[1] - m[1] * h],
            [a[0] - m[0] * h, a[1] - m[1] * h],
        ]));
    }
    // the corners: between each edge and the next
    let corners = if closed { edges.len() } else { edges.len() - 1 };
    for i in 0..corners {
        let (e0, e1) = (edges[i], edges[(i + 1) % edges.len()]);
        let p = e0.1;
        let d0 = [e0.1[0] - e0.0[0], e0.1[1] - e0.0[1]];
        let d1 = [e1.1[0] - e1.0[0], e1.1[1] - e1.0[1]];
        let cross = d0[0] * d1[1] - d0[1] * d1[0];
        let len = dist(d0, [0.0, 0.0]) * dist(d1, [0.0, 0.0]);
        if cross.abs() <= 1e-12 * len {
            continue; // straight on, or straight back: no wedge to fill
        }
        // the outer side is the one the path turns away from
        let s = if cross > 0.0 { -1.0 } else { 1.0 };
        let (n0, n1) = (normal(e0), normal(e1));
        let o0 = [n0[0] * s * h, n0[1] * s * h];
        let o1 = [n1[0] * s * h, n1[1] * s * h];
        let b = [o0[0] + o1[0], o1[1] + o0[1]];
        let bl = dist(b, [0.0, 0.0]);
        let mut wedge = vec![p, [p[0] + o0[0], p[1] + o0[1]]];
        if bl > 0.0 {
            let reach = 2.0 * h * h / bl;
            if reach / h <= 4.0 {
                wedge.push([p[0] + b[0] / bl * reach, p[1] + b[1] / bl * reach]);
            }
        }
        wedge.push([p[0] + o1[0], p[1] + o1[1]]);
        out.push(poly(wedge));
    }
    out
}

/// The scale a map gives lengths, on average: for flattening tolerances.
fn scale_of(m: &Mat) -> f64 {
    (m[0] * m[3] - m[1] * m[2]).abs().sqrt().max(1e-12)
}

/// Every contour a glyph's shapes fill, drawn through `m`, curves kept
/// where they are filled and flattened to within `tol` where a stroke is
/// expanded. Strokes are wound as the glyph's largest filled contour is,
/// so where they overlap it they add to it rather than cut it away.
pub fn outlines(shapes: &[IrShape], m: &Mat, tol: f64) -> Result<Vec<Contour>, String> {
    let mut fills = Vec::new();
    let mut strokes = Vec::new();
    collect(shapes, m, false, tol, &mut fills, &mut strokes)?;
    let clockwise = fills
        .iter()
        .map(|c: &Contour| c.area(tol))
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
        .is_none_or(|a| a < 0.0);
    for c in strokes {
        let a = c.area(tol);
        if a == 0.0 {
            continue;
        }
        fills.push(if (a < 0.0) == clockwise {
            c
        } else {
            c.reversed()
        });
    }
    Ok(fills)
}

fn collect(
    shapes: &[IrShape],
    m: &Mat,
    fixed: bool,
    tol: f64,
    fills: &mut Vec<Contour>,
    strokes: &mut Vec<Contour>,
) -> Result<(), String> {
    for shape in shapes {
        if let IrShape::Group {
            matrix,
            clip,
            strokes: fixed_strokes,
            shapes,
        } = shape
        {
            if clip.is_some() {
                return Err(
                    "a glyph clipped by its transform's bounds cannot become a font's outline"
                        .to_string(),
                );
            }
            let inner = affine::mul(m, matrix);
            collect(
                shapes,
                &inner,
                fixed || fixed_strokes.as_deref() == Some("fixed"),
                tol,
                fills,
                strokes,
            )?;
            continue;
        }
        let (geo, fill, stroke_paint, width) = geometry(shape)?;
        if fill != "none" {
            fills.extend(geo.iter().map(|(c, _)| c.map(m)));
        }
        if stroke_paint != "none" && width > 0.0 {
            for (c, closed) in &geo {
                if fixed {
                    // the width as written, after the map
                    strokes.extend(stroke(&c.map(m), *closed, width, tol));
                } else {
                    // drawn in its own coordinates, then mapped, as SVG
                    // scales a stroke with its shape
                    let local = tol / scale_of(m);
                    strokes.extend(stroke(c, *closed, width, local).iter().map(|s| s.map(m)));
                }
            }
        }
    }
    Ok(())
}

/// What `outlines` gives with no map: for tests.
#[cfg(test)]
pub fn plain(shapes: &[IrShape]) -> Vec<Contour> {
    outlines(shapes, &affine::IDENTITY, 0.01).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(d: &str, fill: &str, stroke: &str, w: f64) -> IrShape {
        IrShape::Path {
            d: d.to_string(),
            fill: fill.to_string(),
            fill_opacity: 1.0,
            stroke: stroke.to_string(),
            stroke_width: w,
            dash: None,
        }
    }

    #[test]
    fn a_filled_path_keeps_its_curves() {
        let c = plain(&[path("M0,0 L10,0 Q10,10 0,10 Z", "ink", "none", 0.0)]);
        assert_eq!(
            c,
            vec![Contour {
                start: [0.0, 0.0],
                segs: vec![Seg::Line([10.0, 0.0]), Seg::Quad([10.0, 10.0], [0.0, 10.0])],
            }]
        );
    }

    #[test]
    fn a_stroked_line_is_a_rectangle_its_width_across() {
        let c = plain(&[path("M0,0 L10,0", "none", "ink", 2.0)]);
        assert_eq!(c.len(), 1);
        assert_eq!(
            c[0].points(0.01),
            vec![[0.0, 1.0], [10.0, 1.0], [10.0, -1.0], [0.0, -1.0]]
        );
        // wound clockwise with y up, as TrueType winds outer contours
        assert!(c[0].area(0.01) < 0.0);
    }

    #[test]
    fn a_corner_is_mitered_within_the_limit_and_bevelled_past_it() {
        // a right angle: its miter reaches √2·h, inside 4
        let c = plain(&[path("M0,0 L10,0 L10,10", "none", "ink", 2.0)]);
        assert_eq!(c.len(), 3);
        let wedge = c[2].points(0.01);
        assert_eq!(wedge.len(), 4, "{wedge:?}");
        assert!(
            wedge
                .iter()
                .any(|p| (p[0] - 11.0).abs() < 1e-9 && (p[1] + 1.0).abs() < 1e-9),
            "{wedge:?}"
        );
        // a hairpin of 10°: past the limit, a bevel of three points
        let c = plain(&[path("M0,0 L10,0 L0,1.76", "none", "ink", 2.0)]);
        assert_eq!(c[2].points(0.01).len(), 3);
    }

    #[test]
    fn strokes_are_wound_as_the_fill_they_overlap() {
        // a counter-clockwise square, filled and stroked: every contour
        // turns the same way, so nothing cancels where they overlap
        let c = plain(&[path("M0,0 L10,0 L10,10 L0,10 Z", "ink", "ink", 1.0)]);
        assert!(c[0].area(0.01) > 0.0);
        assert!(c.iter().all(|c| c.area(0.01) > 0.0));
    }

    #[test]
    fn a_stroke_scales_with_its_group_unless_fixed() {
        let g = |fixed: bool| IrShape::Group {
            matrix: [3.0, 0.0, 0.0, 3.0, 0.0, 0.0],
            clip: None,
            strokes: fixed.then(|| "fixed".to_string()),
            shapes: vec![path("M0,0 L10,0", "none", "ink", 2.0)],
        };
        assert_eq!(plain(&[g(false)])[0].points(0.01)[0], [0.0, 3.0]);
        assert_eq!(plain(&[g(true)])[0].points(0.01)[0], [0.0, 1.0]);
    }

    #[test]
    fn a_reversed_contour_has_the_same_points_the_other_way() {
        let c = Contour {
            start: [0.0, 0.0],
            segs: vec![
                Seg::Line([4.0, 0.0]),
                Seg::Cubic([4.0, 2.0], [2.0, 4.0], [0.0, 4.0]),
            ],
        };
        let r = c.reversed();
        assert_eq!(r.start, [0.0, 0.0]);
        assert_eq!(
            r.segs,
            vec![
                Seg::Line([0.0, 4.0]),
                Seg::Cubic([2.0, 4.0], [4.0, 2.0], [4.0, 0.0]),
                Seg::Line([0.0, 0.0]),
            ]
        );
        assert!((r.area(0.01) + c.area(0.01)).abs() < 1e-9);
    }

    #[test]
    fn a_cubic_becomes_quadratics_close_to_it() {
        let (p0, a, b, p) = ([0.0, 0.0], [0.0, 50.0], [100.0, 50.0], [100.0, 0.0]);
        let mut q = Vec::new();
        cubic_to_quads(p0, a, b, p, 0.5, &mut q);
        assert!(q.len() > 1);
        let mut cur = p0;
        for s in &q {
            let Seg::Quad(c, e) = *s else { panic!() };
            // the quadratic's middle lies near the cubic
            let mid = lerp(lerp(cur, c, 0.5), lerp(c, e, 0.5), 0.5);
            let near = (0..=1000)
                .map(|i| dist(cubic_at([p0, a, b, p], i as f64 / 1000.0), mid))
                .fold(f64::INFINITY, f64::min);
            assert!(near < 0.6, "{near}");
            cur = e;
        }
        assert_eq!(cur, p);
    }
}
