//! 2-D affine maps for transforms (SPEC §4, §7), and the extents of what
//! the IR draws through them: how far a group's shapes reach once drawn
//! through its matrix, for the check that keeps a transform inside its
//! icon's frame. Every angle goes through the portable `cos` and `sin`
//! (SPEC §7), and everything else is basic IEEE-754 arithmetic, so the
//! matrices and the extents they give are the same on every platform.

use crate::ir::IrShape;

/// `[a, b, c, d, e, f]`, as SVG's `matrix()`: a point (x, y) maps to
/// (a·x + c·y + e, b·x + d·y + f).
pub type Mat = [f64; 6];

pub const IDENTITY: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m` after `n`: a point goes through `n`, then `m`.
pub fn mul(m: &Mat, n: &Mat) -> Mat {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

pub fn apply(m: &Mat, [x, y]: [f64; 2]) -> [f64; 2] {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

/// Only the linear part: a direction, not a point.
fn apply_linear(m: &Mat, [x, y]: [f64; 2]) -> [f64; 2] {
    [m[0] * x + m[2] * y, m[1] * x + m[3] * y]
}

pub fn translate(tx: f64, ty: f64) -> Mat {
    [1.0, 0.0, 0.0, 1.0, tx, ty]
}

pub fn scale(sx: f64, sy: f64) -> Mat {
    [sx, 0.0, 0.0, sy, 0.0, 0.0]
}

/// The cosine and sine of an angle in degrees, as `cos()` and `sin()`
/// compute them in expressions: portable, and quantized so a right angle
/// gives exactly 0 and 1.
fn cos_sin(degrees: f64) -> (f64, f64) {
    let r = degrees.to_radians();
    (
        crate::expr::quantize(crate::expr::cos(r)),
        crate::expr::quantize(crate::expr::sin(r)),
    )
}

/// Clockwise on the screen, as SVG's `rotate()`, since y runs down.
pub fn rotate(degrees: f64) -> Mat {
    let (c, s) = cos_sin(degrees);
    [c, s, -s, c, 0.0, 0.0]
}

/// SVG's `skewX()`: x moves by tan(angle) for each unit of y. At ±90° the
/// tangent is not finite, which the caller reports.
pub fn skew_x(degrees: f64) -> Mat {
    let (c, s) = cos_sin(degrees);
    [1.0, 0.0, s / c, 1.0, 0.0, 0.0]
}

pub fn skew_y(degrees: f64) -> Mat {
    let (c, s) = cos_sin(degrees);
    [1.0, s / c, 0.0, 1.0, 0.0, 0.0]
}

/// `m` about a point rather than about (0, 0).
pub fn about([ox, oy]: [f64; 2], m: &Mat) -> Mat {
    mul(&translate(ox, oy), &mul(m, &translate(-ox, -oy)))
}

pub fn is_identity(m: &Mat) -> bool {
    *m == IDENTITY
}

/// An axis-aligned box: what a shape reaches, or a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl Extent {
    pub fn of_point(p: [f64; 2]) -> Extent {
        Extent { min: p, max: p }
    }

    /// A viewBox, `[minx, miny, w, h]`.
    pub fn of_viewbox(v: [f64; 4]) -> Extent {
        Extent {
            min: [v[0], v[1]],
            max: [v[0] + v[2], v[1] + v[3]],
        }
    }

    pub fn viewbox(&self) -> [f64; 4] {
        [
            self.min[0],
            self.min[1],
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
        ]
    }

    pub fn add(&mut self, p: [f64; 2]) {
        for ((lo, hi), v) in self.min.iter_mut().zip(&mut self.max).zip(p) {
            *lo = lo.min(v);
            *hi = hi.max(v);
        }
    }

    pub fn union(&self, o: &Extent) -> Extent {
        let mut out = *self;
        out.add(o.min);
        out.add(o.max);
        out
    }

    /// Where the two overlap, or none.
    pub fn intersect(&self, o: &Extent) -> Option<Extent> {
        let min = [self.min[0].max(o.min[0]), self.min[1].max(o.min[1])];
        let max = [self.max[0].min(o.max[0]), self.max[1].min(o.max[1])];
        (min[0] <= max[0] && min[1] <= max[1]).then_some(Extent { min, max })
    }

    /// Whether it lies inside `frame`, at the precision output is printed
    /// at (SPEC §7): what rounds to the frame's edge is inside it.
    pub fn within(&self, frame: &Extent) -> bool {
        const EPS: f64 = 0.005;
        self.min[0] >= frame.min[0] - EPS
            && self.min[1] >= frame.min[1] - EPS
            && self.max[0] <= frame.max[0] + EPS
            && self.max[1] <= frame.max[1] + EPS
    }

    pub fn corners(&self) -> Vec<[f64; 2]> {
        vec![
            self.min,
            [self.max[0], self.min[1]],
            self.max,
            [self.min[0], self.max[1]],
        ]
    }

    pub fn is_finite(&self) -> bool {
        self.min.iter().chain(&self.max).all(|v| v.is_finite())
    }
}

fn grow(out: &mut Option<Extent>, p: [f64; 2]) {
    match out {
        Some(e) => e.add(p),
        None => *out = Some(Extent::of_point(p)),
    }
}

fn merge(out: &mut Option<Extent>, e: Option<Extent>) {
    if let Some(e) = e {
        grow(out, e.min);
        grow(out, e.max);
    }
}

/// How far an axis-aligned ellipse at `c` with radii `rx`, `ry` reaches
/// through `m`: through an affine map an ellipse stays one, and each half
/// extent is the length of a row of the mapped radii.
fn ellipse_extent(m: &Mat, c: [f64; 2], rx: f64, ry: f64) -> Extent {
    let center = apply(m, c);
    let hx = (m[0] * rx).hypot(m[2] * ry);
    let hy = (m[1] * rx).hypot(m[3] * ry);
    Extent {
        min: [center[0] - hx, center[1] - hy],
        max: [center[0] + hx, center[1] + hy],
    }
}

/// What a shape reaches when drawn through `m`: its geometry, not its
/// stroke. A shape that draws nothing (a circle of no radius, a rect of
/// no size, a path of moves alone) reaches nothing. Path data it cannot
/// read is an error, saying why.
pub fn shape_extent(shape: &IrShape, m: &Mat) -> Result<Option<Extent>, String> {
    let mut out = None;
    match shape {
        IrShape::Polygon { points, .. } => {
            for p in points {
                grow(&mut out, apply(m, *p));
            }
        }
        IrShape::Path { d, .. } => return path_extent(d, m),
        IrShape::Circle { cx, cy, r, .. } => {
            if *r > 0.0 {
                out = Some(ellipse_extent(m, [*cx, *cy], *r, *r));
            }
        }
        IrShape::Ellipse { cx, cy, rx, ry, .. } => {
            if *rx > 0.0 && *ry > 0.0 {
                out = Some(ellipse_extent(m, [*cx, *cy], *rx, *ry));
            }
        }
        IrShape::Rect { x, y, w, h, rx, .. } => {
            if *w > 0.0 && *h > 0.0 {
                // a rounded rect is its inner rect grown by a circle of the
                // corner radius, as SVG clamps it
                let r = rx.max(0.0).min(w / 2.0).min(h / 2.0);
                let inner = [
                    [x + r, y + r],
                    [x + w - r, y + r],
                    [x + w - r, y + h - r],
                    [x + r, y + h - r],
                ];
                let mut corners = None;
                for p in inner {
                    grow(&mut corners, apply(m, p));
                }
                let mut e = corners.expect("four corners");
                if r > 0.0 {
                    // the corners' reach, by the map's linear part alone
                    let linear = [m[0], m[1], m[2], m[3], 0.0, 0.0];
                    let round = ellipse_extent(&linear, [0.0, 0.0], r, r);
                    e.min = [e.min[0] + round.min[0], e.min[1] + round.min[1]];
                    e.max = [e.max[0] + round.max[0], e.max[1] + round.max[1]];
                }
                out = Some(e);
            }
        }
        IrShape::Group {
            matrix,
            clip,
            shapes,
            ..
        } => {
            let inner = mul(m, matrix);
            for s in shapes {
                merge(&mut out, shape_extent(s, &inner)?);
            }
            // what a clip cuts away is not drawn
            if let (Some(e), Some(clip)) = (out, clip) {
                let mut cut = None;
                for p in clip {
                    grow(&mut cut, apply(m, *p));
                }
                out = cut.and_then(|c| e.intersect(&c));
            }
        }
    }
    Ok(out)
}

/// Where a cubic Bézier's coordinate `k` turns, as parameters in (0, 1).
fn cubic_turns(p: [[f64; 2]; 4], k: usize) -> Vec<f64> {
    // the derivative, a quadratic a·t² + b·t + c
    let (p0, p1, p2, p3) = (p[0][k], p[1][k], p[2][k], p[3][k]);
    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
    let b = 2.0 * (p0 - 2.0 * p1 + p2);
    let c = p1 - p0;
    let mut out = Vec::new();
    if a.abs() < 1e-12 {
        if b.abs() > 1e-12 {
            out.push(-c / b);
        }
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let q = disc.sqrt();
            out.push((-b + q) / (2.0 * a));
            out.push((-b - q) / (2.0 * a));
        }
    }
    out.retain(|t| *t > 0.0 && *t < 1.0);
    out
}

fn cubic_at(p: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [
        w0 * p[0][0] + w1 * p[1][0] + w2 * p[2][0] + w3 * p[3][0],
        w0 * p[0][1] + w1 * p[1][1] + w2 * p[2][1] + w3 * p[3][1],
    ]
}

/// A cubic's extent: its ends and wherever it turns. Points are already
/// mapped, since an affine map takes a Bézier to the Bézier of its mapped
/// control points.
fn cubic_extent(out: &mut Option<Extent>, p: [[f64; 2]; 4]) {
    grow(out, p[0]);
    grow(out, p[3]);
    for k in 0..2 {
        for t in cubic_turns(p, k) {
            grow(out, cubic_at(p, t));
        }
    }
}

/// An elliptical arc in SVG's endpoint form, drawn through `m`: its ends,
/// and where the mapped ellipse reaches furthest in each direction while
/// on the arc (SVG 1.1 appendix F.6.5 and F.6.6).
#[allow(clippy::too_many_arguments)]
fn arc_extent(
    out: &mut Option<Extent>,
    m: &Mat,
    from: [f64; 2],
    rx: f64,
    ry: f64,
    phi_degrees: f64,
    large: bool,
    sweep: bool,
    to: [f64; 2],
) {
    grow(out, apply(m, from));
    grow(out, apply(m, to));
    if from == to {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        return; // a straight line, its ends already counted
    }
    let phi = phi_degrees.to_radians();
    let (cos_phi, sin_phi) = (libm::cos(phi), libm::sin(phi));
    let dx = (from[0] - to[0]) / 2.0;
    let dy = (from[1] - to[1]) / 2.0;
    let x1 = cos_phi * dx + sin_phi * dy;
    let y1 = -sin_phi * dx + cos_phi * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let k = lambda.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut coef = if den == 0.0 {
        0.0
    } else {
        (num / den).max(0.0).sqrt()
    };
    if large == sweep {
        coef = -coef;
    }
    let cx1 = coef * rx * y1 / ry;
    let cy1 = -coef * ry * x1 / rx;
    let center = [
        cos_phi * cx1 - sin_phi * cy1 + (from[0] + to[0]) / 2.0,
        sin_phi * cx1 + cos_phi * cy1 + (from[1] + to[1]) / 2.0,
    ];
    let angle = |ux: f64, uy: f64| libm::atan2(uy, ux);
    let theta1 = angle((x1 - cx1) / rx, (y1 - cy1) / ry);
    let theta2 = angle((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let tau = std::f64::consts::TAU;
    let mut delta = theta2 - theta1;
    if sweep && delta < 0.0 {
        delta += tau;
    } else if !sweep && delta > 0.0 {
        delta -= tau;
    }
    // the mapped arc: C + U·cos t + V·sin t
    let u = apply_linear(m, [rx * cos_phi, rx * sin_phi]);
    let v = apply_linear(m, [-ry * sin_phi, ry * cos_phi]);
    let c = apply(m, center);
    let on_arc = |t: f64| {
        // how far t is along the sweep, in its direction
        let mut along = (t - theta1) * delta.signum();
        along = along.rem_euclid(tau);
        along <= delta.abs()
    };
    for k in 0..2 {
        let t0 = libm::atan2(v[k], u[k]);
        for t in [t0, t0 + std::f64::consts::PI] {
            if on_arc(t) {
                let (ct, st) = (libm::cos(t), libm::sin(t));
                grow(
                    out,
                    [c[0] + u[0] * ct + v[0] * st, c[1] + u[1] * ct + v[1] * st],
                );
            }
        }
    }
}

/// Reads SVG path data, the whole grammar: every command, absolute and
/// relative, with repeated arguments, numbers run together where SVG
/// allows it (`1-2`, `.5.5`) and arc flags written without separators.
struct PathReader<'a> {
    b: &'a [u8],
    i: usize,
}

impl PathReader<'_> {
    fn skip(&mut self) {
        while self.i < self.b.len()
            && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r' | 0x0c | b',')
        {
            self.i += 1;
        }
    }

    fn at_number(&mut self) -> bool {
        self.skip();
        matches!(self.b.get(self.i), Some(b'0'..=b'9' | b'.' | b'-' | b'+'))
    }

    fn number(&mut self) -> Result<f64, String> {
        self.skip();
        let start = self.i;
        if matches!(self.b.get(self.i), Some(b'-' | b'+')) {
            self.i += 1;
        }
        let mut digits = false;
        while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
            self.i += 1;
            digits = true;
        }
        if self.b.get(self.i) == Some(&b'.') {
            self.i += 1;
            while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
                digits = true;
            }
        }
        if !digits {
            return Err(format!("expected a number at offset {start}"));
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            let mark = self.i;
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'-' | b'+')) {
                self.i += 1;
            }
            let exp_start = self.i;
            while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if self.i == exp_start {
                self.i = mark; // an `e` that begins no exponent
            }
        }
        let text = std::str::from_utf8(&self.b[start..self.i]).expect("ASCII");
        text.parse::<f64>()
            .map_err(|_| format!("'{text}' is not a number"))
    }

    fn flag(&mut self) -> Result<bool, String> {
        self.skip();
        match self.b.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.i += 1;
                Ok(true)
            }
            _ => Err(format!(
                "expected an arc flag, 0 or 1, at offset {}",
                self.i
            )),
        }
    }
}

/// What path data reaches drawn through `m`.
pub fn path_extent(d: &str, m: &Mat) -> Result<Option<Extent>, String> {
    let mut r = PathReader {
        b: d.as_bytes(),
        i: 0,
    };
    let mut out = None;
    let mut cur = [0.0, 0.0];
    let mut start = [0.0, 0.0];
    // the last control point, for S and T to reflect
    let mut last_cubic: Option<[f64; 2]> = None;
    let mut last_quad: Option<[f64; 2]> = None;
    let mut cmd: Option<u8> = None;
    let line = |out: &mut Option<Extent>, a: [f64; 2], b: [f64; 2]| {
        grow(out, apply(m, a));
        grow(out, apply(m, b));
    };
    loop {
        r.skip();
        let Some(&c) = r.b.get(r.i) else { break };
        let command = if c.is_ascii_alphabetic() {
            r.i += 1;
            c
        } else {
            match cmd {
                // after a move, more pairs are lines
                Some(b'M') => b'L',
                Some(b'm') => b'l',
                Some(prev) if !matches!(prev, b'Z' | b'z') => prev,
                _ => {
                    return Err(format!(
                        "path data must begin with a command, at offset {}",
                        r.i
                    ))
                }
            }
        };
        let rel = command.is_ascii_lowercase();
        let base = if rel { cur } else { [0.0, 0.0] };
        let pt = |r: &mut PathReader| -> Result<[f64; 2], String> {
            let x = r.number()?;
            let y = r.number()?;
            Ok([base[0] + x, base[1] + y])
        };
        let (mut cubic_ctrl, mut quad_ctrl) = (None, None);
        match command.to_ascii_uppercase() {
            b'M' => {
                cur = pt(&mut r)?;
                start = cur;
            }
            b'L' => {
                let p = pt(&mut r)?;
                line(&mut out, cur, p);
                cur = p;
            }
            b'H' => {
                let x = r.number()? + if rel { cur[0] } else { 0.0 };
                let p = [x, cur[1]];
                line(&mut out, cur, p);
                cur = p;
            }
            b'V' => {
                let y = r.number()? + if rel { cur[1] } else { 0.0 };
                let p = [cur[0], y];
                line(&mut out, cur, p);
                cur = p;
            }
            b'C' | b'S' => {
                let c1 = if command.eq_ignore_ascii_case(&b'C') {
                    pt(&mut r)?
                } else {
                    match last_cubic {
                        Some(q) => [2.0 * cur[0] - q[0], 2.0 * cur[1] - q[1]],
                        None => cur,
                    }
                };
                let c2 = pt(&mut r)?;
                let p = pt(&mut r)?;
                cubic_extent(
                    &mut out,
                    [apply(m, cur), apply(m, c1), apply(m, c2), apply(m, p)],
                );
                cubic_ctrl = Some(c2);
                cur = p;
            }
            b'Q' | b'T' => {
                let q = if command.eq_ignore_ascii_case(&b'Q') {
                    pt(&mut r)?
                } else {
                    match last_quad {
                        Some(q) => [2.0 * cur[0] - q[0], 2.0 * cur[1] - q[1]],
                        None => cur,
                    }
                };
                let p = pt(&mut r)?;
                // a quadratic is the cubic with these control points
                let c1 = [
                    cur[0] + 2.0 / 3.0 * (q[0] - cur[0]),
                    cur[1] + 2.0 / 3.0 * (q[1] - cur[1]),
                ];
                let c2 = [
                    p[0] + 2.0 / 3.0 * (q[0] - p[0]),
                    p[1] + 2.0 / 3.0 * (q[1] - p[1]),
                ];
                cubic_extent(
                    &mut out,
                    [apply(m, cur), apply(m, c1), apply(m, c2), apply(m, p)],
                );
                quad_ctrl = Some(q);
                cur = p;
            }
            b'A' => {
                let rx = r.number()?;
                let ry = r.number()?;
                let phi = r.number()?;
                let large = r.flag()?;
                let sweep = r.flag()?;
                let p = pt(&mut r)?;
                arc_extent(&mut out, m, cur, rx, ry, phi, large, sweep, p);
                cur = p;
            }
            b'Z' => {
                line(&mut out, cur, start);
                cur = start;
            }
            _ => return Err(format!("'{}' is not a path command", char::from(command))),
        }
        last_cubic = cubic_ctrl;
        last_quad = quad_ctrl;
        cmd = Some(command);
        if matches!(command, b'Z' | b'z') && r.at_number() {
            return Err(format!("numbers after a closepath, at offset {}", r.i));
        }
    }
    Ok(out)
}

/// A piece of an outline, in absolute coordinates: a contour starts with
/// a move, and may close back to its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move([f64; 2]),
    Line([f64; 2]),
    /// control point, end
    Quad([f64; 2], [f64; 2]),
    /// two control points, end
    Cubic([f64; 2], [f64; 2], [f64; 2]),
    Close,
}

/// Path data as an outline: every command made absolute, H and V as
/// lines, S and T with their reflected controls, and each arc as cubics of
/// a quarter-turn or less, the usual approximation (its error under
/// 0.03% of the radius). What converts a drawing to a font's outlines.
pub fn path_segments(d: &str) -> Result<Vec<Seg>, String> {
    let mut r = PathReader {
        b: d.as_bytes(),
        i: 0,
    };
    let mut out = Vec::new();
    let mut cur = [0.0, 0.0];
    let mut start = [0.0, 0.0];
    let mut last_cubic: Option<[f64; 2]> = None;
    let mut last_quad: Option<[f64; 2]> = None;
    let mut cmd: Option<u8> = None;
    loop {
        r.skip();
        let Some(&c) = r.b.get(r.i) else { break };
        let command = if c.is_ascii_alphabetic() {
            r.i += 1;
            c
        } else {
            match cmd {
                Some(b'M') => b'L',
                Some(b'm') => b'l',
                Some(prev) if !matches!(prev, b'Z' | b'z') => prev,
                _ => {
                    return Err(format!(
                        "path data must begin with a command, at offset {}",
                        r.i
                    ))
                }
            }
        };
        let rel = command.is_ascii_lowercase();
        let base = if rel { cur } else { [0.0, 0.0] };
        let pt = |r: &mut PathReader| -> Result<[f64; 2], String> {
            let x = r.number()?;
            let y = r.number()?;
            Ok([base[0] + x, base[1] + y])
        };
        let (mut cubic_ctrl, mut quad_ctrl) = (None, None);
        match command.to_ascii_uppercase() {
            b'M' => {
                cur = pt(&mut r)?;
                start = cur;
                out.push(Seg::Move(cur));
            }
            b'L' => {
                cur = pt(&mut r)?;
                out.push(Seg::Line(cur));
            }
            b'H' => {
                cur = [r.number()? + if rel { cur[0] } else { 0.0 }, cur[1]];
                out.push(Seg::Line(cur));
            }
            b'V' => {
                cur = [cur[0], r.number()? + if rel { cur[1] } else { 0.0 }];
                out.push(Seg::Line(cur));
            }
            b'C' | b'S' => {
                let c1 = if command.eq_ignore_ascii_case(&b'C') {
                    pt(&mut r)?
                } else {
                    match last_cubic {
                        Some(q) => [2.0 * cur[0] - q[0], 2.0 * cur[1] - q[1]],
                        None => cur,
                    }
                };
                let c2 = pt(&mut r)?;
                cur = pt(&mut r)?;
                out.push(Seg::Cubic(c1, c2, cur));
                cubic_ctrl = Some(c2);
            }
            b'Q' | b'T' => {
                let q = if command.eq_ignore_ascii_case(&b'Q') {
                    pt(&mut r)?
                } else {
                    match last_quad {
                        Some(q) => [2.0 * cur[0] - q[0], 2.0 * cur[1] - q[1]],
                        None => cur,
                    }
                };
                cur = pt(&mut r)?;
                out.push(Seg::Quad(q, cur));
                quad_ctrl = Some(q);
            }
            b'A' => {
                let rx = r.number()?;
                let ry = r.number()?;
                let phi = r.number()?;
                let large = r.flag()?;
                let sweep = r.flag()?;
                let p = pt(&mut r)?;
                arc_cubics(&mut out, cur, rx, ry, phi, large, sweep, p);
                cur = p;
            }
            b'Z' => {
                out.push(Seg::Close);
                cur = start;
            }
            _ => return Err(format!("'{}' is not a path command", char::from(command))),
        }
        last_cubic = cubic_ctrl;
        last_quad = quad_ctrl;
        cmd = Some(command);
        if matches!(command, b'Z' | b'z') && r.at_number() {
            return Err(format!("numbers after a closepath, at offset {}", r.i));
        }
    }
    Ok(out)
}

/// An arc in SVG's endpoint form (appendix F.6.5) as cubics, each a
/// quarter-turn or less; a line when a radius is 0, nothing when its ends
/// meet.
#[allow(clippy::too_many_arguments)]
fn arc_cubics(
    out: &mut Vec<Seg>,
    from: [f64; 2],
    rx: f64,
    ry: f64,
    phi_degrees: f64,
    large: bool,
    sweep: bool,
    to: [f64; 2],
) {
    if from == to {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        out.push(Seg::Line(to));
        return;
    }
    let phi = phi_degrees.to_radians();
    let (cos_phi, sin_phi) = (libm::cos(phi), libm::sin(phi));
    let dx = (from[0] - to[0]) / 2.0;
    let dy = (from[1] - to[1]) / 2.0;
    let x1 = cos_phi * dx + sin_phi * dy;
    let y1 = -sin_phi * dx + cos_phi * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let k = lambda.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut coef = if den == 0.0 {
        0.0
    } else {
        (num / den).max(0.0).sqrt()
    };
    if large == sweep {
        coef = -coef;
    }
    let cx1 = coef * rx * y1 / ry;
    let cy1 = -coef * ry * x1 / rx;
    let center = [
        cos_phi * cx1 - sin_phi * cy1 + (from[0] + to[0]) / 2.0,
        sin_phi * cx1 + cos_phi * cy1 + (from[1] + to[1]) / 2.0,
    ];
    let theta1 = libm::atan2((y1 - cy1) / ry, (x1 - cx1) / rx);
    let theta2 = libm::atan2((-y1 - cy1) / ry, (-x1 - cx1) / rx);
    let tau = std::f64::consts::TAU;
    let mut delta = theta2 - theta1;
    if sweep && delta < 0.0 {
        delta += tau;
    } else if !sweep && delta > 0.0 {
        delta -= tau;
    }
    let at = |t: f64| {
        let (ct, st) = (libm::cos(t), libm::sin(t));
        [
            center[0] + rx * ct * cos_phi - ry * st * sin_phi,
            center[1] + rx * ct * sin_phi + ry * st * cos_phi,
        ]
    };
    let slope = |t: f64| {
        let (ct, st) = (libm::cos(t), libm::sin(t));
        [
            -rx * st * cos_phi - ry * ct * sin_phi,
            -rx * st * sin_phi + ry * ct * cos_phi,
        ]
    };
    let pieces = (delta.abs() / (std::f64::consts::FRAC_PI_2 + 1e-9))
        .ceil()
        .max(1.0) as usize;
    let step = delta / pieces as f64;
    let k = 4.0 / 3.0 * libm::tan(step / 4.0);
    for i in 0..pieces {
        let (a, b) = (theta1 + step * i as f64, theta1 + step * (i + 1) as f64);
        let (pa, pb) = (at(a), at(b));
        let (da, db) = (slope(a), slope(b));
        let end = if i + 1 == pieces { to } else { pb };
        out.push(Seg::Cubic(
            [pa[0] + k * da[0], pa[1] + k * da[1]],
            [pb[0] - k * db[0], pb[1] - k * db[1]],
            end,
        ));
    }
}

/// The part of a convex polygon inside another (Sutherland–Hodgman):
/// both in the same coordinates, either winding. Nothing in common is an
/// empty list.
pub fn clip_convex(subject: &[[f64; 2]], clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let area: f64 = clip
        .iter()
        .zip(clip.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum();
    let sign = if area < 0.0 { -1.0 } else { 1.0 };
    let mut out: Vec<[f64; 2]> = subject.to_vec();
    for (a, b) in clip.iter().zip(clip.iter().cycle().skip(1)) {
        if out.is_empty() {
            break;
        }
        let side =
            |p: &[f64; 2]| sign * ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]));
        let input = std::mem::take(&mut out);
        for (i, p) in input.iter().enumerate() {
            let q = &input[(i + 1) % input.len()];
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                out.push(*p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                out.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn path_segments_reads_every_command_absolute() {
        let segs =
            path_segments("M1,2 h3 v4 l-1,1 q1 1 2 0 t2 0 c1 1 2 1 3 0 s2 -1 3 0 z").unwrap();
        assert_eq!(
            segs,
            vec![
                Seg::Move([1.0, 2.0]),
                Seg::Line([4.0, 2.0]),
                Seg::Line([4.0, 6.0]),
                Seg::Line([3.0, 7.0]),
                Seg::Quad([4.0, 8.0], [5.0, 7.0]),
                Seg::Quad([6.0, 6.0], [7.0, 7.0]),
                Seg::Cubic([8.0, 8.0], [9.0, 8.0], [10.0, 7.0]),
                Seg::Cubic([11.0, 6.0], [12.0, 6.0], [13.0, 7.0]),
                Seg::Close,
            ]
        );
        assert!(path_segments("1 2").is_err());
    }

    #[test]
    fn an_arc_is_cubics_of_a_quarter_turn_that_stay_on_it() {
        // a half circle of radius 10 about (10, 0): two quarter cubics
        let segs = path_segments("M0,0 A10 10 0 0 1 20,0").unwrap();
        assert_eq!(segs.len(), 3);
        let Seg::Cubic(_, _, end) = segs[2] else {
            panic!("{segs:?}")
        };
        assert_eq!(end, [20.0, 0.0]);
        let Seg::Cubic(c1, c2, mid) = segs[1] else {
            panic!("{segs:?}")
        };
        // the middle of the first quarter cubic is within 0.03% of the circle
        let p0 = [0.0, 0.0];
        let at = |t: f64| {
            let u = 1.0 - t;
            [
                u * u * u * p0[0]
                    + 3.0 * u * u * t * c1[0]
                    + 3.0 * u * t * t * c2[0]
                    + t * t * t * mid[0],
                u * u * u * p0[1]
                    + 3.0 * u * u * t * c1[1]
                    + 3.0 * u * t * t * c2[1]
                    + t * t * t * mid[1],
            ]
        };
        let m = at(0.5);
        let r = ((m[0] - 10.0).powi(2) + m[1].powi(2)).sqrt();
        assert!((r - 10.0).abs() < 0.003, "{r}");
        assert!(
            (mid[0] - 10.0).abs() < 1e-9 && (mid[1].abs() - 10.0).abs() < 1e-9,
            "{mid:?}"
        );
        // a radius of 0 is a line
        assert_eq!(
            path_segments("M0,0 A0 5 0 0 1 4,0").unwrap()[1],
            Seg::Line([4.0, 0.0])
        );
    }

    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn maps_compose_right_to_left() {
        let m = mul(&translate(10.0, 0.0), &scale(2.0, 2.0));
        assert_eq!(apply(&m, [1.0, 1.0]), [12.0, 2.0]);
        // a right angle is exact, through the quantized cos and sin
        assert_eq!(rotate(90.0), [0.0, 1.0, -1.0, 0.0, 0.0, 0.0]);
        assert_eq!(
            apply(&about([5.0, 5.0], &rotate(180.0)), [6.0, 5.0]),
            [4.0, 5.0]
        );
        assert_eq!(skew_x(45.0), [1.0, 0.0, 1.0, 1.0, 0.0, 0.0]);
        assert!(!skew_x(90.0)[2].is_finite());
    }

    #[test]
    fn a_rotated_circle_reaches_as_far_and_an_ellipse_turns() {
        let e = ellipse_extent(&rotate(90.0), [10.0, 0.0], 4.0, 2.0);
        assert!(close(e.min[0], -2.0) && close(e.max[0], 2.0));
        assert!(close(e.min[1], 6.0) && close(e.max[1], 14.0));
    }

    #[test]
    fn a_rounded_rect_is_its_inner_rect_grown_by_its_corners() {
        let rect = IrShape::Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            rx: 5.0,
            fill: "ink".into(),
            fill_opacity: 1.0,
            stroke: "none".into(),
            stroke_width: 0.0,
            dash: None,
        };
        // a circle, really: it reaches as far turned as not
        let e = shape_extent(&rect, &about([5.0, 5.0], &rotate(45.0)))
            .unwrap()
            .unwrap();
        // to the quantum of cos and sin (SPEC §7)
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(near(e.min[0], 0.0) && near(e.max[0], 10.0), "{e:?}");
    }

    #[test]
    fn path_extents_follow_curves_and_arcs() {
        let e = |d: &str| path_extent(d, &IDENTITY).unwrap().unwrap();
        // a half circle bulging up
        let half = e("M0,0 A10 10 0 0 1 20,0");
        assert!(
            close(half.min[1], -10.0) && close(half.max[1], 0.0),
            "{half:?}"
        );
        // the other way it bulges down
        let down = e("M0,0 A10 10 0 0 0 20,0");
        assert!(close(down.max[1], 10.0), "{down:?}");
        // a cubic turning above its ends
        let c = e("M0 0 C0 -10 10 -10 10 0");
        assert!(close(c.min[1], -7.5), "{c:?}");
        // relative, repeated and run together
        let rel = e("m10,10 l5-5.5.5.5h-20v10z");
        assert!(
            close(rel.min[0], -4.5) && close(rel.min[1], 4.5) && close(rel.max[1], 15.0),
            "{rel:?}"
        );
        // the cylinder's body, as projection writes it
        let body = e("M -10.00,-5.00 L -10.00,5.00 A 10.00 5.00 0 0 0 10.00,5.00 L 10.00,-5.00 Z");
        assert!(close(body.max[1], 10.0), "{body:?}");
        // flags without separators
        let flags = e("M0,0 a5,5 0 1020,0");
        assert!(close(flags.max[0], 20.0), "{flags:?}");
        assert!(path_extent("M0 0 L", &IDENTITY).is_err());
        assert!(path_extent("10 10", &IDENTITY).is_err());
        assert_eq!(path_extent("M5 5", &IDENTITY).unwrap(), None);
    }

    #[test]
    fn an_arc_turned_reaches_where_the_turned_ellipse_does() {
        let m = rotate(90.0);
        let e = path_extent("M0,0 A10 10 0 0 1 20,0", &m).unwrap().unwrap();
        // the half circle above (0..20, -10..0) turns to (0..10, 0..20)
        assert!(close(e.min[0], 0.0) && close(e.max[0], 10.0), "{e:?}");
        assert!(close(e.max[1], 20.0), "{e:?}");
    }

    #[test]
    fn convex_clipping_keeps_what_overlaps() {
        let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let shifted = [[5.0, 5.0], [15.0, 5.0], [15.0, 15.0], [5.0, 15.0]];
        let mut cut = clip_convex(&square, &shifted);
        cut.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            cut,
            vec![[5.0, 5.0], [5.0, 10.0], [10.0, 5.0], [10.0, 10.0]]
        );
        let far = [[20.0, 20.0], [30.0, 20.0], [30.0, 30.0]];
        assert!(clip_convex(&square, &far).is_empty());
        // the other winding clips the same
        let reversed: Vec<[f64; 2]> = shifted.iter().rev().copied().collect();
        assert_eq!(clip_convex(&square, &reversed).len(), 4);
    }
}
