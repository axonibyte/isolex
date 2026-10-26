//! A TrueType font, written from scratch: the tables a font needs to be
//! installed and set (head, hhea, maxp, OS/2, hmtx, cmap, loca, glyf,
//! name, post), with kerning as a `kern` table and ligatures as a GSUB
//! `liga` feature. Every byte follows from the font given: no dates, no
//! hinting, so the same font always writes the same file.

/// One glyph: its advance, and its contours of points in font units,
/// each on the curve or a quadratic control point.
#[derive(Debug, Clone, Default)]
pub struct Glyph {
    pub advance: u16,
    pub contours: Vec<Vec<(i16, i16, bool)>>,
}

/// Everything a font file says.
#[derive(Debug, Clone, Default)]
pub struct Font {
    pub family: String,
    pub subfamily: String,
    pub version: String,
    pub copyright: Option<String>,
    pub trademark: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub upm: u16,
    pub ascent: i16,
    /// Below the baseline, as a distance.
    pub descent: i16,
    /// Degrees, negative leaning right, as OpenType's italicAngle.
    pub italic_angle: f64,
    pub bold: bool,
    pub italic: bool,
    /// Glyph 0 is .notdef.
    pub glyphs: Vec<Glyph>,
    /// (character, glyph), in character order.
    pub cmap: Vec<(u32, u16)>,
    /// (left, right, space), any order.
    pub kerns: Vec<(u16, u16, i16)>,
    /// (the glyphs it replaces, the ligature).
    pub ligatures: Vec<(Vec<u16>, u16)>,
}

/// Big-endian bytes, appended.
#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn i16(&mut self, v: i16) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn i64(&mut self, v: i64) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.0.extend_from_slice(b);
        self
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn set_u16(&mut self, at: usize, v: u16) {
        self.0[at..at + 2].copy_from_slice(&v.to_be_bytes());
    }
}

/// The binary search fields of a sorted table of `n` entries of `size`
/// bytes: (searchRange, entrySelector, rangeShift).
fn search(n: usize, size: usize) -> (u16, u16, u16) {
    let mut sel = 0u16;
    while (2usize << sel) <= n {
        sel += 1;
    }
    let range = (1usize << sel) * size;
    (range as u16, sel, (n * size - range.min(n * size)) as u16)
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        sum.wrapping_add(u32::from_be_bytes(w))
    })
}

fn bbox(g: &Glyph) -> Option<(i16, i16, i16, i16)> {
    let mut it = g.contours.iter().flatten();
    let &(x, y, _) = it.next()?;
    Some(it.fold((x, y, x, y), |(a, b, c, d), &(x, y, _)| {
        (a.min(x), b.min(y), c.max(x), d.max(y))
    }))
}

/// One glyph's data in the glyf table: a simple glyph, every flag and
/// coordinate written out in full, or nothing for one with no contours.
fn glyf_entry(g: &Glyph) -> Vec<u8> {
    let Some((x0, y0, x1, y1)) = bbox(g) else {
        return Vec::new();
    };
    let mut w = W::default();
    w.i16(g.contours.len() as i16)
        .i16(x0)
        .i16(y0)
        .i16(x1)
        .i16(y1);
    let mut end = 0usize;
    for c in &g.contours {
        end += c.len();
        w.u16((end - 1) as u16);
    }
    w.u16(0); // no instructions
    for &(_, _, on) in g.contours.iter().flatten() {
        w.u8(u8::from(on));
    }
    let (mut px, mut py) = (0i32, 0i32);
    for &(x, _, _) in g.contours.iter().flatten() {
        w.i16((i32::from(x) - px) as i16);
        px = i32::from(x);
    }
    for &(_, y, _) in g.contours.iter().flatten() {
        w.i16((i32::from(y) - py) as i16);
        py = i32::from(y);
    }
    while w.len() % 4 != 0 {
        w.u8(0);
    }
    w.0
}

fn cmap(font: &Font) -> Vec<u8> {
    // runs of characters whose glyphs run alongside them
    let runs = |chars: &mut dyn Iterator<Item = (u32, u16)>| {
        let mut out: Vec<(u32, u32, u16)> = Vec::new();
        for (c, g) in chars {
            match out.last_mut() {
                Some((start, end, g0))
                    if *end + 1 == c && u32::from(*g0) + (c - *start) == u32::from(g) =>
                {
                    *end = c;
                }
                _ => out.push((c, c, g)),
            }
        }
        out
    };
    let bmp = runs(&mut font.cmap.iter().copied().filter(|(c, _)| *c <= 0xFFFF));
    let all = runs(&mut font.cmap.iter().copied());
    let wide = font.cmap.iter().any(|(c, _)| *c > 0xFFFF);

    // format 4, for the basic plane, and its closing segment
    let mut f4 = W::default();
    let mut segs: Vec<(u16, u16, u16)> = bmp
        .iter()
        .map(|&(s, e, g)| {
            (
                s as u16,
                e as u16,
                (u32::from(g).wrapping_sub(s) & 0xFFFF) as u16,
            )
        })
        .collect();
    segs.push((0xFFFF, 0xFFFF, 1));
    let n = segs.len();
    let (range, sel, shift) = search(n, 2);
    f4.u16(4)
        .u16(0)
        .u16(0)
        .u16((n * 2) as u16)
        .u16(range)
        .u16(sel)
        .u16(shift);
    for &(_, e, _) in &segs {
        f4.u16(e);
    }
    f4.u16(0);
    for &(s, _, _) in &segs {
        f4.u16(s);
    }
    for &(_, _, d) in &segs {
        f4.u16(d);
    }
    for _ in &segs {
        f4.u16(0);
    }
    let len = f4.len() as u16;
    f4.set_u16(2, len);

    // format 12, when anything is beyond it
    let mut f12 = W::default();
    if wide {
        f12.u16(12)
            .u16(0)
            .u32((16 + 12 * all.len()) as u32)
            .u32(0)
            .u32(all.len() as u32);
        for &(s, e, g) in &all {
            f12.u32(s).u32(e).u32(u32::from(g));
        }
    }

    let mut w = W::default();
    let count = if wide { 2 } else { 1 };
    w.u16(0).u16(count);
    let header = 4 + 8 * count as usize;
    w.u16(3).u16(1).u32(header as u32);
    if wide {
        w.u16(3).u16(10).u32((header + f4.len()) as u32);
    }
    w.bytes(&f4.0);
    w.bytes(&f12.0);
    w.0
}

fn name(font: &Font) -> Vec<u8> {
    let full = if font.subfamily == "Regular" {
        font.family.clone()
    } else {
        format!("{} {}", font.family, font.subfamily)
    };
    let ps: String = format!("{}-{}", font.family, font.subfamily)
        .chars()
        .filter(|c| c.is_ascii_graphic() && !"[](){}<>/%".contains(*c))
        .take(63)
        .collect();
    let mut records: Vec<(u16, String)> = vec![
        (1, font.family.clone()),
        (2, font.subfamily.clone()),
        (3, format!("{};{}", font.version, ps)),
        (4, full),
        (5, font.version.clone()),
        (6, ps),
    ];
    for (id, v) in [
        (0, &font.copyright),
        (7, &font.trademark),
        (13, &font.license),
        (14, &font.license_url),
    ] {
        if let Some(v) = v {
            records.push((id, v.clone()));
        }
    }
    records.sort_by_key(|(id, _)| *id);
    let mut strings = W::default();
    let mut w = W::default();
    w.u16(0)
        .u16(records.len() as u16)
        .u16((6 + 12 * records.len()) as u16);
    for (id, text) in &records {
        let utf16: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
        w.u16(3)
            .u16(1)
            .u16(0x409)
            .u16(*id)
            .u16(utf16.len() as u16)
            .u16(strings.len() as u16);
        strings.bytes(&utf16);
    }
    w.bytes(&strings.0);
    w.0
}

fn kern(font: &Font) -> Option<Vec<u8>> {
    if font.kerns.is_empty() {
        return None;
    }
    let mut pairs = font.kerns.clone();
    pairs.sort_by_key(|(l, r, _)| (u32::from(*l) << 16) | u32::from(*r));
    // a format 0 subtable is at most 65535 bytes long
    pairs.truncate((0xFFFF - 14) / 6);
    let n = pairs.len();
    let (range, sel, shift) = search(n, 6);
    let mut w = W::default();
    w.u16(0).u16(1);
    w.u16(0).u16((14 + 6 * n) as u16).u16(0x0001);
    w.u16(n as u16).u16(range).u16(sel).u16(shift);
    for (l, r, v) in pairs {
        w.u16(l).u16(r).i16(v);
    }
    Some(w.0)
}

/// GSUB with one `liga` feature, for the default script and Latin, of one
/// ligature lookup.
fn gsub(font: &Font) -> Option<Vec<u8>> {
    if font.ligatures.is_empty() {
        return None;
    }
    // ligature sets by first glyph, longest ligatures first in each
    let mut firsts: Vec<u16> = font.ligatures.iter().map(|(c, _)| c[0]).collect();
    firsts.sort_unstable();
    firsts.dedup();
    let mut subst = W::default();
    subst.u16(1).u16(0).u16(firsts.len() as u16);
    let set_offsets_at = subst.len();
    for _ in &firsts {
        subst.u16(0);
    }
    let mut sets = Vec::new();
    for &first in &firsts {
        let mut ligs: Vec<&(Vec<u16>, u16)> = font
            .ligatures
            .iter()
            .filter(|(c, _)| c[0] == first)
            .collect();
        ligs.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
        let mut set = W::default();
        set.u16(ligs.len() as u16);
        let mut body = W::default();
        let head = 2 + 2 * ligs.len();
        for (comps, lig) in &ligs {
            set.u16((head + body.len()) as u16);
            body.u16(*lig).u16(comps.len() as u16);
            for c in &comps[1..] {
                body.u16(*c);
            }
        }
        set.bytes(&body.0);
        sets.push(set.0);
    }
    for (i, set) in sets.iter().enumerate() {
        let at = subst.len() as u16;
        subst.set_u16(set_offsets_at + 2 * i, at);
        subst.bytes(set);
    }
    let coverage_at = subst.len() as u16;
    subst.set_u16(2, coverage_at);
    subst.u16(1).u16(firsts.len() as u16);
    for g in &firsts {
        subst.u16(*g);
    }

    let mut lookup = W::default();
    lookup.u16(4).u16(0).u16(1).u16(8).bytes(&subst.0);
    let mut lookups = W::default();
    lookups.u16(1).u16(4).bytes(&lookup.0);

    let mut features = W::default();
    features.u16(1).bytes(b"liga").u16(8).u16(0).u16(1).u16(0);

    // two scripts, each its default language system with feature 0
    let langsys = [0u8, 0, 0xFF, 0xFF, 0, 1, 0, 0];
    let mut script = W::default();
    script.u16(4).u16(0).bytes(&langsys);
    let mut scripts = W::default();
    scripts.u16(2);
    scripts.bytes(b"DFLT").u16(14);
    scripts.bytes(b"latn").u16((14 + script.len()) as u16);
    scripts.bytes(&script.0).bytes(&script.0);

    let mut w = W::default();
    let (s, f) = (10, 10 + scripts.len());
    let l = f + features.len();
    w.u16(1).u16(0).u16(s as u16).u16(f as u16).u16(l as u16);
    w.bytes(&scripts.0).bytes(&features.0).bytes(&lookups.0);
    Some(w.0)
}

/// The whole font file.
pub fn write(font: &Font) -> Vec<u8> {
    let n = font.glyphs.len();
    let boxes: Vec<Option<(i16, i16, i16, i16)>> = font.glyphs.iter().map(bbox).collect();
    let all = boxes
        .iter()
        .flatten()
        .fold(None, |acc: Option<(i16, i16, i16, i16)>, b| {
            Some(match acc {
                None => *b,
                Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
            })
        });
    let (x0, y0, x1, y1) = all.unwrap_or((0, 0, 0, 0));

    let mut glyf = Vec::new();
    let mut loca = W::default();
    for g in &font.glyphs {
        loca.u32(glyf.len() as u32);
        glyf.extend(glyf_entry(g));
    }
    loca.u32(glyf.len() as u32);

    let mut hmtx = W::default();
    for (g, b) in font.glyphs.iter().zip(&boxes) {
        hmtx.u16(g.advance).i16(b.map(|b| b.0).unwrap_or(0));
    }
    let advance_max = font.glyphs.iter().map(|g| g.advance).max().unwrap_or(0);
    let lsbs = boxes.iter().flatten().map(|b| b.0);
    let rsbs = font
        .glyphs
        .iter()
        .zip(&boxes)
        .filter_map(|(g, b)| b.map(|b| g.advance as i16 - b.2));
    let extents = boxes.iter().flatten().map(|b| b.2);
    let rad = font.italic_angle.to_radians();

    let mut hhea = W::default();
    hhea.u32(0x0001_0000)
        .i16(font.ascent)
        .i16(-font.descent)
        .i16(0)
        .u16(advance_max)
        .i16(lsbs.clone().min().unwrap_or(0))
        .i16(rsbs.min().unwrap_or(0))
        .i16(extents.max().unwrap_or(0));
    if font.italic_angle == 0.0 {
        hhea.i16(1).i16(0);
    } else {
        hhea.i16(1000)
            .i16((-1000.0 * libm::tan(rad)).round() as i16);
    }
    hhea.i16(0).i16(0).i16(0).i16(0).i16(0).i16(0).u16(n as u16);

    let points = |g: &Glyph| g.contours.iter().map(|c| c.len()).sum::<usize>();
    let mut maxp = W::default();
    maxp.u32(0x0001_0000)
        .u16(n as u16)
        .u16(font.glyphs.iter().map(points).max().unwrap_or(0) as u16)
        .u16(
            font.glyphs
                .iter()
                .map(|g| g.contours.len())
                .max()
                .unwrap_or(0) as u16,
        )
        .u16(0)
        .u16(0)
        .u16(2);
    // no twilight points, storage, functions, instructions or components
    for _ in 0..8 {
        maxp.u16(0);
    }

    let upm = i32::from(font.upm);
    let part = |share: f64| (f64::from(font.upm) * share).round() as i16;
    let advances: Vec<i32> = font
        .glyphs
        .iter()
        .map(|g| i32::from(g.advance))
        .filter(|a| *a > 0)
        .collect();
    let avg = if advances.is_empty() {
        0
    } else {
        advances.iter().sum::<i32>() / advances.len() as i32
    };
    let first = font.cmap.iter().map(|c| c.0).min().unwrap_or(0).min(0xFFFF) as u16;
    let last = font.cmap.iter().map(|c| c.0).max().unwrap_or(0).min(0xFFFF) as u16;
    let mut selection = 0u16;
    if font.italic {
        selection |= 0x01;
    }
    if font.bold {
        selection |= 0x20;
    }
    if !font.italic && !font.bold {
        selection |= 0x40;
    }
    selection |= 0x80; // use the typographic metrics
    let mut os2 = W::default();
    os2.u16(4)
        .i16(avg as i16)
        .u16(if font.bold { 700 } else { 400 })
        .u16(5)
        .u16(0) // installable: the font's own license says the rest
        .i16(part(0.65))
        .i16(part(0.6))
        .i16(0)
        .i16(part(0.075))
        .i16(part(0.65))
        .i16(part(0.6))
        .i16(0)
        .i16(part(0.35))
        .i16(part(0.05))
        .i16(part(0.25))
        .i16(0)
        .bytes(&[0; 10])
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0)
        .bytes(b"NONE")
        .u16(selection)
        .u16(first)
        .u16(last)
        .i16(font.ascent)
        .i16(-font.descent)
        .i16(0)
        .u16(i32::from(font.ascent).max(i32::from(y1)).clamp(0, 0xFFFF) as u16)
        .u16(i32::from(font.descent).max(-i32::from(y0)).clamp(0, 0xFFFF) as u16)
        .u32(1)
        .u32(0)
        .i16(0)
        .i16(0)
        .u16(0)
        .u16(32)
        .u16(
            font.ligatures
                .iter()
                .map(|(c, _)| c.len())
                .max()
                .unwrap_or(1)
                .max(1) as u16,
        );

    let mut post = W::default();
    let angle = (font.italic_angle * 65536.0).round() as i32;
    post.u32(0x0003_0000)
        .u32(angle as u32)
        .i16(-(upm / 10) as i16)
        .i16((upm / 20) as i16)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0);

    let mut head = W::default();
    let mut style = 0u16;
    if font.bold {
        style |= 1;
    }
    if font.italic {
        style |= 2;
    }
    head.u32(0x0001_0000)
        .u32(0x0001_0000)
        .u32(0) // checkSumAdjustment, set once the file is whole
        .u32(0x5F0F_3CF5)
        .u16(0x0009)
        .u16(font.upm)
        .i64(0)
        .i64(0)
        .i16(x0)
        .i16(y0)
        .i16(x1)
        .i16(y1)
        .u16(style)
        .u16(8)
        .i16(2)
        .i16(1)
        .i16(0);

    let mut tables: Vec<([u8; 4], Vec<u8>)> = vec![
        (*b"OS/2", os2.0),
        (*b"cmap", cmap(font)),
        (*b"glyf", glyf),
        (*b"head", head.0),
        (*b"hhea", hhea.0),
        (*b"hmtx", hmtx.0),
        (*b"loca", loca.0),
        (*b"maxp", maxp.0),
        (*b"name", name(font)),
        (*b"post", post.0),
    ];
    if let Some(k) = kern(font) {
        tables.push((*b"kern", k));
    }
    if let Some(g) = gsub(font) {
        tables.push((*b"GSUB", g));
    }
    tables.sort_by_key(|t| t.0);

    let count = tables.len();
    let (range, sel, shift) = search(count, 16);
    let mut out = W::default();
    out.u32(0x0001_0000)
        .u16(count as u16)
        .u16(range)
        .u16(sel)
        .u16(shift);
    let mut offset = 12 + 16 * count;
    let mut head_at = 0;
    for (tag, data) in &tables {
        if tag == b"head" {
            head_at = offset;
        }
        out.bytes(tag)
            .u32(checksum(data))
            .u32(offset as u32)
            .u32(data.len() as u32);
        offset += data.len().div_ceil(4) * 4;
    }
    for (_, data) in &tables {
        out.bytes(data);
        while out.len() % 4 != 0 {
            out.u8(0);
        }
    }
    let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out.0));
    out.0[head_at + 8..head_at + 12].copy_from_slice(&adjust.to_be_bytes());
    out.0
}
