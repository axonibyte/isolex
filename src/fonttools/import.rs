//! `isolex font import`: a TrueType, OpenType or WOFF font as an isolex
//! font file. Each glyph's outline becomes path data on the font's grid;
//! advances, kerning (GPOS pair adjustment, or a `kern` table) and simple
//! ligatures (GSUB `liga`) come with it; other weights, slopes or
//! variable-font settings become styles; and the font's embedding
//! permissions are honored and its copyright and license notices copied.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::text::format_value as num;
use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
use ttf_parser::gsub::SubstitutionSubtable;
use ttf_parser::{Face, GlyphId, Tag};

/// Which characters to keep.
#[derive(Debug, Clone, PartialEq)]
pub enum Charset {
    /// U+0020 to U+007E.
    Ascii,
    /// ASCII and U+00A0 to U+00FF: the default.
    Latin1,
    /// Every character the font maps.
    All,
    /// These.
    Given(Vec<char>),
}

/// Where a style's outlines come from: another font file, or the same
/// variable font at other axis values.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    File(PathBuf),
    Variation(Vec<(Tag, f32)>),
}

/// `--style axis.variant=source`.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleSource {
    pub axis: String,
    pub variant: String,
    pub source: Source,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub input: PathBuf,
    pub name: Option<String>,
    pub chars: Charset,
    pub em: Option<f64>,
    /// Font units after scaling: a curve closer than this to its chord is
    /// written as a line.
    pub tolerance: f64,
    pub index: u32,
    pub styles: Vec<StyleSource>,
}

/// A style's source as written: `wght:700,wdth:87.5` is a variation of
/// the font itself, anything else a file.
pub fn parse_source(text: &str) -> Source {
    let settings: Option<Vec<(Tag, f32)>> = text
        .split(',')
        .map(|s| {
            let (tag, value) = s.split_once(':')?;
            let tag = tag.trim();
            if tag.len() != 4 || !tag.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return None;
            }
            Some((
                Tag::from_bytes(tag.as_bytes().try_into().ok()?),
                value.trim().parse().ok()?,
            ))
        })
        .collect();
    match settings {
        Some(s) if !s.is_empty() => Source::Variation(s),
        _ => Source::File(PathBuf::from(text)),
    }
}

/// A font file's bytes as a font: WOFF unwrapped into the font it holds.
fn load(path: &Path) -> Result<Vec<u8>, String> {
    let data = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    match data.get(..4) {
        Some(b"wOFF") => unwoff(&data).map_err(|e| format!("{}: {e}", path.display())),
        Some(b"wOF2") => Err(format!(
            "{}: WOFF2 is not read yet; convert it to TrueType, OpenType or WOFF first",
            path.display()
        )),
        _ => Ok(data),
    }
}

/// WOFF 1.0: its tables, each inflated when compressed, laid out again as
/// the sfnt font they came from.
fn unwoff(w: &[u8]) -> Result<Vec<u8>, String> {
    let be16 = |at: usize| -> Result<u16, String> {
        w.get(at..at + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .ok_or_else(|| "a truncated WOFF file".to_string())
    };
    let be32 = |at: usize| -> Result<u32, String> {
        w.get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "a truncated WOFF file".to_string())
    };
    let flavor = be32(4)?;
    let n = usize::from(be16(12)?);
    let mut tables = Vec::with_capacity(n);
    for i in 0..n {
        let at = 44 + 20 * i;
        let tag = be32(at)?;
        let (offset, comp, orig, sum) = (
            be32(at + 4)? as usize,
            be32(at + 8)? as usize,
            be32(at + 12)? as usize,
            be32(at + 16)?,
        );
        let raw = w
            .get(offset..offset + comp)
            .ok_or_else(|| "a WOFF table reaches past the file".to_string())?;
        let data = if comp < orig {
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(raw, orig)
                .map_err(|e| format!("a WOFF table does not inflate: {e:?}"))?
        } else {
            raw.to_vec()
        };
        if data.len() != orig {
            return Err("a WOFF table inflates to the wrong length".to_string());
        }
        tables.push((tag, sum, data));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&flavor.to_be_bytes());
    let mut sel = 0u16;
    while (2usize << sel) <= n {
        sel += 1;
    }
    let range = (1u16 << sel) * 16;
    for v in [n as u16, range, sel, (n as u16) * 16 - range] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    let mut offset = 12 + 16 * n;
    for (tag, sum, data) in &tables {
        for v in [*tag, *sum, offset as u32, data.len() as u32] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        offset += data.len().div_ceil(4) * 4;
    }
    for (_, _, data) in &tables {
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    Ok(out)
}

/// What the font's license, in its OS/2 embedding bits, allows: a
/// restricted font, or one that lets only its bitmaps be embedded, is not
/// converted; one that may not be subset is converted whole or not at
/// all.
fn permitted(face: &Face, chars: &Charset, path: &Path) -> Result<(), String> {
    let Some(os2) = face.tables().os2 else {
        return Ok(()); // no embedding bits: nothing is restricted
    };
    let refuse = |why: &str| {
        Err(format!(
            "{} {why}; isolex will not convert it. Its license, not isolex, says what it may be \
             used for: ask its vendor for a license that allows it",
            path.display()
        ))
    };
    if os2.permissions() == Some(ttf_parser::Permissions::Restricted) {
        return refuse("is marked restricted (OS/2 fsType): its license allows it to be neither embedded nor converted");
    }
    if !os2.is_outline_embedding_allowed() {
        return refuse("allows only its bitmaps to be embedded (OS/2 fsType), not its outlines");
    }
    if !os2.is_subsetting_allowed() && *chars != Charset::All {
        return Err(format!(
            "{} may not be subset (OS/2 fsType); convert it whole with --chars all",
            path.display()
        ));
    }
    Ok(())
}

/// The font's name `id`, in English if it has it.
fn name_of(face: &Face, id: u16) -> Option<String> {
    let mut best: Option<(u8, String)> = None;
    for n in face.names() {
        if n.name_id != id {
            continue;
        }
        let Some(text) = n.to_string() else { continue };
        let rank = match (n.platform_id, n.language_id) {
            (ttf_parser::PlatformId::Windows, 0x409) => 0,
            (ttf_parser::PlatformId::Unicode, _) => 1,
            _ => 2,
        };
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, t)| t)
}

/// A font name as an isolex name: lower case, words joined by hyphens.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// A KDL string of `s`, quoted, with what KDL would not take written as
/// escapes.
fn kdl(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == ' ' || c.is_ascii_graphic() || (c.is_alphanumeric() && !c.is_control()) => {
                out.push(c)
            }
            c => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
        }
    }
    out.push('"');
    out
}

/// Path data from an outline: absolute commands, numbers as text reads
/// them, curves within `tol` of their chord written as lines.
struct PathOut {
    d: String,
    scale: f64,
    tol: f64,
    cur: [f64; 2],
}

impl PathOut {
    fn p(&self, x: f32, y: f32) -> [f64; 2] {
        [f64::from(x) * self.scale, f64::from(y) * self.scale]
    }
    fn put(&mut self, cmd: char, pts: &[[f64; 2]]) {
        if !self.d.is_empty() {
            self.d.push(' ');
        }
        self.d.push(cmd);
        for (i, p) in pts.iter().enumerate() {
            if i > 0 {
                self.d.push(' ');
            }
            let _ = write!(self.d, "{},{}", num(p[0]), num(p[1]));
        }
    }
    /// How far a control point is from the chord to `end`.
    fn off(&self, c: [f64; 2], end: [f64; 2]) -> f64 {
        let (a, b) = (self.cur, end);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dy * dy).sqrt();
        if l == 0.0 {
            return ((c[0] - a[0]).powi(2) + (c[1] - a[1]).powi(2)).sqrt();
        }
        ((c[0] - a[0]) * dy - (c[1] - a[1]) * dx).abs() / l
    }
}

impl ttf_parser::OutlineBuilder for PathOut {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.put('M', &[p]);
        self.cur = p;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.put('L', &[p]);
        self.cur = p;
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, p) = (self.p(x1, y1), self.p(x, y));
        if self.tol > 0.0 && self.off(c, p) <= self.tol {
            self.put('L', &[p]);
        } else {
            self.put('Q', &[c, p]);
        }
        self.cur = p;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b, p) = (self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        if self.tol > 0.0 && self.off(a, p) <= self.tol && self.off(b, p) <= self.tol {
            self.put('L', &[p]);
        } else {
            self.put('C', &[a, b, p]);
        }
        self.cur = p;
    }
    fn close(&mut self) {
        self.d.push_str(" Z");
    }
}

/// A glyph: its advance, and its path data, if it draws anything.
#[derive(Debug, Clone, PartialEq)]
struct Drawn {
    advance: f64,
    d: Option<String>,
}

/// One instance of a font, read: metrics, glyphs by name, kerning.
#[derive(Debug, Clone)]
struct Read {
    ascent: f64,
    descent: f64,
    /// By name: characters in code point order, then ligatures.
    glyphs: Vec<(String, Drawn)>,
    notdef: Drawn,
    kerns: BTreeMap<(String, String), f64>,
}

/// The characters to keep, each with the glyph the font maps it to.
fn mapped(face: &Face, chars: &Charset) -> Vec<(char, GlyphId)> {
    let wanted: Vec<char> = match chars {
        Charset::Ascii => (0x20u32..=0x7E).filter_map(char::from_u32).collect(),
        Charset::Latin1 => (0x20u32..=0x7E)
            .chain(0xA0..=0xFF)
            .filter_map(char::from_u32)
            .collect(),
        Charset::Given(cs) => {
            let mut cs = cs.clone();
            cs.sort_unstable();
            cs.dedup();
            cs
        }
        Charset::All => {
            let mut all = Vec::new();
            if let Some(cmap) = face.tables().cmap {
                for sub in cmap.subtables {
                    if sub.is_unicode() {
                        sub.codepoints(|c| all.extend(char::from_u32(c)));
                    }
                }
            }
            all.sort_unstable();
            all.dedup();
            all
        }
    };
    wanted
        .into_iter()
        // what text could never set is not worth a glyph
        .filter(|c| crate::text::refusal(*c).is_none())
        .filter_map(|c| face.glyph_index(c).filter(|g| g.0 != 0).map(|g| (c, g)))
        .collect()
}

/// The lookups of a layout table's features of `tag`, in lookup order.
fn lookups_of(table: &ttf_parser::opentype_layout::LayoutTable, tag: &[u8; 4]) -> Vec<u16> {
    let mut out: Vec<u16> = table
        .features
        .into_iter()
        .filter(|f| f.tag == Tag::from_bytes(tag))
        .flat_map(|f| f.lookup_indices.into_iter())
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The ligatures `liga` makes from the characters kept: the run of
/// characters each replaces, and its glyph.
fn ligatures(face: &Face, kept: &[(char, GlyphId)]) -> Vec<(String, GlyphId)> {
    let Some(gsub) = face.tables().gsub else {
        return Vec::new();
    };
    // the first character a glyph draws, among those kept
    let mut char_of: BTreeMap<u16, char> = BTreeMap::new();
    for (c, g) in kept {
        char_of.entry(g.0).or_insert(*c);
    }
    let mut out: Vec<(String, GlyphId)> = Vec::new();
    for li in lookups_of(&gsub, b"liga") {
        let Some(lookup) = gsub.lookups.get(li) else {
            continue;
        };
        for sub in lookup.subtables.into_iter::<SubstitutionSubtable>() {
            let SubstitutionSubtable::Ligature(l) = sub else {
                continue;
            };
            for (c, g) in kept {
                let Some(ci) = l.coverage.get(*g) else {
                    continue;
                };
                let Some(set) = l.ligature_sets.get(ci) else {
                    continue;
                };
                for lig in set {
                    let rest: Option<String> = lig
                        .components
                        .into_iter()
                        .map(|g| char_of.get(&g.0).copied())
                        .collect();
                    let Some(rest) = rest else { continue };
                    let name = format!("{c}{rest}");
                    if !out.iter().any(|(n, _)| *n == name) {
                        out.push((name, lig.glyph));
                    }
                }
            }
        }
    }
    out.sort();
    out
}

/// The space GPOS `kern` puts between two glyphs, or failing GPOS, the
/// `kern` table's: in font units.
fn kerning(face: &Face, lookups: &[u16], left: GlyphId, right: GlyphId) -> i32 {
    if let Some(gpos) = face.tables().gpos {
        if !lookups.is_empty() {
            let mut sum = 0i32;
            for &li in lookups {
                let Some(lookup) = gpos.lookups.get(li) else {
                    continue;
                };
                for sub in lookup.subtables.into_iter::<PositioningSubtable>() {
                    let PositioningSubtable::Pair(pair) = sub else {
                        continue;
                    };
                    let found = match pair {
                        PairAdjustment::Format1 { coverage, sets } => coverage
                            .get(left)
                            .and_then(|i| sets.get(i))
                            .and_then(|set| set.get(right))
                            .map(|(a, _)| a.x_advance),
                        PairAdjustment::Format2 {
                            coverage,
                            classes,
                            matrix,
                        } => coverage.contains(left).then(|| {
                            matrix
                                .get((classes.0.get(left), classes.1.get(right)))
                                .map(|(a, _)| a.x_advance)
                                .unwrap_or(0)
                        }),
                    };
                    // the first subtable a pair matches is the one applied
                    if let Some(v) = found {
                        sum += i32::from(v);
                        break;
                    }
                }
            }
            return sum;
        }
    }
    face.tables()
        .kern
        .map(|k| {
            k.subtables
                .into_iter()
                .filter(|s| s.horizontal && !s.variable && !s.has_cross_stream)
                .filter_map(|s| s.glyphs_kerning(left, right))
                .map(i32::from)
                .sum()
        })
        .unwrap_or(0)
}

/// One instance of a font: its glyphs for the characters kept and their
/// ligatures, scaled to `em`.
fn read(face: &Face, chars: &Charset, em: f64, tol: f64) -> Read {
    let scale = em / f64::from(face.units_per_em());
    let draw = |g: GlyphId| {
        let mut out = PathOut {
            d: String::new(),
            scale,
            tol,
            cur: [0.0, 0.0],
        };
        face.outline_glyph(g, &mut out);
        Drawn {
            advance: f64::from(face.glyph_hor_advance(g).unwrap_or(0)) * scale,
            d: (!out.d.is_empty()).then_some(out.d),
        }
    };
    let kept = mapped(face, chars);
    let mut named: Vec<(String, GlyphId)> = kept.iter().map(|(c, g)| (c.to_string(), *g)).collect();
    named.extend(ligatures(face, &kept));
    let lookups = face
        .tables()
        .gpos
        .map(|gpos| lookups_of(&gpos, b"kern"))
        .unwrap_or_default();
    let mut kerns = BTreeMap::new();
    for (l, lg) in &named {
        for (r, rg) in &named {
            let k = kerning(face, &lookups, *lg, *rg);
            if k != 0 {
                kerns.insert((l.clone(), r.clone()), f64::from(k) * scale);
            }
        }
    }
    Read {
        ascent: f64::from(face.ascender()) * scale,
        descent: -f64::from(face.descender()) * scale,
        glyphs: named.iter().map(|(n, g)| (n.clone(), draw(*g))).collect(),
        notdef: draw(GlyphId(0)),
        kerns,
    }
}

fn glyph_line(out: &mut String, indent: &str, name: Option<&str>, g: &Drawn) {
    let head = match name {
        Some(n) => format!("glyph {}", kdl(n)),
        None => "notdef".to_string(),
    };
    match &g.d {
        None => {
            let _ = writeln!(out, "{indent}{head} advance={}", num(g.advance));
        }
        Some(d) => {
            let _ = writeln!(out, "{indent}{head} advance={} {{", num(g.advance));
            let _ = writeln!(
                out,
                "{indent}  path d={} fill=\"ink\" stroke=\"none\"",
                kdl(d)
            );
            let _ = writeln!(out, "{indent}}}");
        }
    }
}

/// The font at `opts.input`, and its styles, as isolex source.
pub fn import(opts: &Options) -> Result<String, String> {
    let data = load(&opts.input)?;
    let face = Face::parse(&data, opts.index)
        .map_err(|e| format!("{}: not a font isolex can read: {e}", opts.input.display()))?;
    permitted(&face, &opts.chars, &opts.input)?;
    let upm = f64::from(face.units_per_em());
    let em = opts.em.unwrap_or(upm);
    let tol = opts.tolerance;
    let base = read(&face, &opts.chars, em, tol);
    if base.glyphs.is_empty() {
        return Err(format!(
            "{} has no glyphs for the characters asked for",
            opts.input.display()
        ));
    }
    let family = name_of(&face, 16)
        .or_else(|| name_of(&face, 1))
        .unwrap_or_default();
    let name = match &opts.name {
        Some(n) => n.clone(),
        None if !slug(&family).is_empty() => slug(&family),
        None => "imported".to_string(),
    };

    // the styles: each read as the base was, from its own source
    let mut styles: Vec<(&StyleSource, Read)> = Vec::new();
    let mut extra: Vec<Vec<u8>> = Vec::new();
    for s in &opts.styles {
        if let Source::File(p) = &s.source {
            extra.push(load(p)?);
        }
    }
    let mut files = extra.iter();
    for s in &opts.styles {
        let r = match &s.source {
            Source::File(p) => {
                let data = files.next().expect("loaded above");
                let f = Face::parse(data, 0)
                    .map_err(|e| format!("{}: not a font isolex can read: {e}", p.display()))?;
                permitted(&f, &opts.chars, p)?;
                // its own grid, scaled to the base's em
                read(&f, &opts.chars, em, tol)
            }
            Source::Variation(settings) => {
                if !face.is_variable() {
                    return Err(format!(
                        "{} is not a variable font, so style {}.{} cannot set its axes",
                        opts.input.display(),
                        s.axis,
                        s.variant
                    ));
                }
                let mut f = face.clone();
                let axes: Vec<Tag> = face.variation_axes().into_iter().map(|a| a.tag).collect();
                for (tag, v) in settings {
                    // ttf-parser takes any tag, and sets nothing for one
                    // the font has not
                    if !axes.contains(tag) || f.set_variation(*tag, *v).is_none() {
                        let axes: Vec<String> = axes.iter().map(|t| t.to_string()).collect();
                        return Err(format!(
                            "{} has no axis '{tag}' (its axes: {})",
                            opts.input.display(),
                            axes.join(", ")
                        ));
                    }
                }
                read(&f, &opts.chars, em, tol)
            }
        };
        styles.push((s, r));
    }

    let mut out = String::new();
    let source = opts
        .input
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "// {name}: converted by isolex font import from {source}."
    );
    let _ = writeln!(
        out,
        "// It is a derivative of that font, and is under the font's license:"
    );
    let notices = [
        (0, "Copyright"),
        (7, "Trademark"),
        (13, "License"),
        (14, "License URL"),
    ];
    for (id, _) in notices {
        if let Some(text) = name_of(&face, id) {
            for line in text.lines() {
                let _ = writeln!(out, "// {line}");
            }
        }
    }
    let _ = writeln!(out, "font {} {{", kdl(&name));
    if !family.is_empty() {
        let _ = writeln!(out, "  describe {}", kdl(&format!("Family: {family}")));
    }
    if let Some(v) = name_of(&face, 5) {
        let _ = writeln!(out, "  describe {}", kdl(&format!("Version: {v}")));
    }
    for (id, key) in notices {
        if let Some(text) = name_of(&face, id) {
            let _ = writeln!(out, "  describe {}", kdl(&format!("{key}: {text}")));
        }
    }
    let _ = writeln!(out, "  em {}", num(em));
    let _ = writeln!(out, "  ascent {}", num(base.ascent));
    let _ = writeln!(out, "  descent {}", num(base.descent));
    let _ = writeln!(out, "  kind \"outline\"");
    let mut axes: Vec<String> = Vec::new();
    for (s, _) in &styles {
        let a = format!("{name}-{}", s.axis);
        if !axes.contains(&a) {
            axes.push(a);
        }
    }
    if !axes.is_empty() {
        let list: Vec<String> = axes.iter().map(|a| kdl(a)).collect();
        let _ = writeln!(out, "  axes {}", list.join(" "));
    }
    for (n, g) in &base.glyphs {
        glyph_line(&mut out, "  ", Some(n), g);
    }
    glyph_line(&mut out, "  ", None, &base.notdef);
    for ((l, r), v) in &base.kerns {
        let _ = writeln!(out, "  kern {} {} {}", kdl(l), kdl(r), num(*v));
    }
    let _ = writeln!(out, "}}");

    // each axis, its variants: what each style draws differently
    for axis in &opts
        .styles
        .iter()
        .map(|s| s.axis.clone())
        .fold(Vec::new(), |mut v, a| {
            if !v.contains(&a) {
                v.push(a);
            }
            v
        })
    {
        let _ = writeln!(out, "\naxis {} {{", kdl(&format!("{name}-{axis}")));
        for (s, r) in styles.iter().filter(|(s, _)| &s.axis == axis) {
            let _ = writeln!(out, "  variant {} {{", kdl(&s.variant));
            if r.ascent != base.ascent {
                let _ = writeln!(out, "    ascent {}", num(r.ascent));
            }
            if r.descent != base.descent {
                let _ = writeln!(out, "    descent {}", num(r.descent));
            }
            let theirs: BTreeMap<&str, &Drawn> =
                base.glyphs.iter().map(|(n, g)| (n.as_str(), g)).collect();
            for (n, g) in &r.glyphs {
                if theirs.get(n.as_str()) != Some(&g) {
                    glyph_line(&mut out, "    ", Some(n), g);
                }
            }
            if r.notdef != base.notdef {
                glyph_line(&mut out, "    ", None, &r.notdef);
            }
            for (pair, v) in &r.kerns {
                if base.kerns.get(pair) != Some(v) {
                    let _ = writeln!(
                        out,
                        "    kern {} {} {}",
                        kdl(&pair.0),
                        kdl(&pair.1),
                        num(*v)
                    );
                }
            }
            for pair in base.kerns.keys() {
                if !r.kerns.contains_key(pair) {
                    let _ = writeln!(out, "    kern {} {} 0", kdl(&pair.0), kdl(&pair.1));
                }
            }
            let _ = writeln!(out, "  }}");
        }
        let _ = writeln!(out, "}}");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_source_is_a_variation_or_a_file() {
        assert_eq!(
            parse_source("wght:700,wdth:87.5"),
            Source::Variation(vec![
                (Tag::from_bytes(b"wght"), 700.0),
                (Tag::from_bytes(b"wdth"), 87.5)
            ])
        );
        assert_eq!(
            parse_source("Bold.ttf"),
            Source::File(PathBuf::from("Bold.ttf"))
        );
        // a Windows path is a file, though it has a colon
        assert_eq!(
            parse_source("C:\\fonts\\Bold.ttf"),
            Source::File(PathBuf::from("C:\\fonts\\Bold.ttf"))
        );
    }

    #[test]
    fn names_and_strings_are_written_as_kdl_takes_them() {
        assert_eq!(slug("Noto Sans"), "noto-sans");
        assert_eq!(slug("  Fira  Code!"), "fira-code");
        assert_eq!(kdl("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(kdl("é Å"), "\"é Å\"");
        assert_eq!(kdl("\u{a0}\u{ad}"), "\"\\u{a0}\\u{ad}\"");
    }
}
