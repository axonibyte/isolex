//! A UFO 3 font source: the format font editors read and write, as a
//! directory of property lists and one `.glif` file a glyph. Curves are
//! kept as they are drawn, cubic or quadratic, so nothing is
//! approximated on the way out.

use std::fmt::Write as _;
use std::path::Path;

use super::export::Prepared;
use super::outline::Seg;
use isolex::text::format_value as num;

/// The glyph name a font editor knows a glyph by: `uni0041` for a
/// character (`u1F600` beyond the basic plane), components joined by `_`
/// for a ligature, and `.notdef`.
pub fn production_name(name: &str) -> String {
    if name == ".notdef" {
        return name.to_string();
    }
    name.chars()
        .map(|c| {
            let u = c as u32;
            if u <= 0xFFFF {
                format!("uni{u:04X}")
            } else {
                format!("u{u:05X}")
            }
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// UFO 3's user name to file name convention: a capital letter gains an
/// underscore after it, so names differing only in case stay apart on
/// file systems that ignore case; characters file systems refuse become
/// underscores; a leading period does too; and a name a file system
/// reserves is prefixed with one.
pub fn file_name(name: &str) -> String {
    const ILLEGAL: &str = "\"*+/:<>?[\\]|";
    const RESERVED: [&str; 22] = [
        "con", "prn", "aux", "clock$", "nul", "com1", "com2", "com3", "com4", "com5", "com6",
        "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8",
    ];
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if (c as u32) < 0x20 || c as u32 == 0x7F || ILLEGAL.contains(c) || (i == 0 && c == '.') {
            out.push('_');
        } else if c.is_ascii_uppercase() {
            out.push(c);
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let out: String = out
        .split('.')
        .map(|part| {
            if RESERVED.contains(&part.to_ascii_lowercase().as_str()) {
                format!("_{part}")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(".");
    format!("{out}.glif")
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A property list of one dict, its entries written in the order given.
fn plist(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n{body}</plist>\n"
    )
}

fn number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("<integer>{}</integer>", v as i64)
    } else {
        format!("<real>{}</real>", num(v))
    }
}

/// One glyph's `.glif`: its advance, its character if it has one, and its
/// contours, each point the end of the segment that reaches it, so a
/// contour's last point is where it started.
fn glif(
    name: &str,
    advance: f64,
    unicode: Option<char>,
    contours: &[super::outline::Contour],
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(s, "<glyph name=\"{}\" format=\"2\">", xml(name));
    let _ = writeln!(s, "  <advance width=\"{}\"/>", num(advance));
    if let Some(c) = unicode {
        let _ = writeln!(s, "  <unicode hex=\"{:04X}\"/>", c as u32);
    }
    if contours.is_empty() {
        let _ = writeln!(s, "  <outline/>");
    } else {
        let _ = writeln!(s, "  <outline>");
        for c in contours {
            let _ = writeln!(s, "    <contour>");
            let mut segs = c.segs.clone();
            let last = segs.last().map(|s| match s {
                Seg::Line(p) | Seg::Quad(_, p) | Seg::Cubic(_, _, p) => *p,
            });
            if last != Some(c.start) {
                segs.push(Seg::Line(c.start));
            }
            let pt = |s: &mut String, p: [f64; 2], kind: Option<&str>| {
                let _ = match kind {
                    Some(k) => writeln!(
                        s,
                        "      <point x=\"{}\" y=\"{}\" type=\"{k}\"/>",
                        num(p[0]),
                        num(p[1])
                    ),
                    None => writeln!(s, "      <point x=\"{}\" y=\"{}\"/>", num(p[0]), num(p[1])),
                };
            };
            for seg in &segs {
                match *seg {
                    Seg::Line(p) => pt(&mut s, p, Some("line")),
                    Seg::Quad(c1, p) => {
                        pt(&mut s, c1, None);
                        pt(&mut s, p, Some("qcurve"));
                    }
                    Seg::Cubic(a, b, p) => {
                        pt(&mut s, a, None);
                        pt(&mut s, b, None);
                        pt(&mut s, p, Some("curve"));
                    }
                }
            }
            let _ = writeln!(s, "    </contour>");
        }
        let _ = writeln!(s, "  </outline>");
    }
    let _ = writeln!(s, "</glyph>");
    s
}

/// The font as a UFO at `dir`, which must not exist yet.
pub fn write(p: &Prepared, dir: &Path) -> Result<(), String> {
    if dir.exists() {
        return Err(format!(
            "{} already exists; a UFO is written into a new directory",
            dir.display()
        ));
    }
    let names: Vec<String> = p.glyphs.iter().map(|g| production_name(&g.name)).collect();
    let mut files: Vec<(String, String)> = Vec::new();

    files.push((
        "metainfo.plist".into(),
        plist("<dict>\n  <key>creator</key>\n  <string>com.axonibyte.isolex</string>\n  <key>formatVersion</key>\n  <integer>3</integer>\n</dict>\n"),
    ));

    let mut info = String::from("<dict>\n");
    let mut entry = |k: &str, v: String| {
        let _ = writeln!(info, "  <key>{k}</key>\n  {v}");
    };
    let string = |s: &str| format!("<string>{}</string>", xml(s));
    entry("familyName", string(&p.family));
    entry("styleName", string(&p.subfamily));
    entry("versionMajor", "<integer>1</integer>".into());
    entry("versionMinor", "<integer>0</integer>".into());
    if let Some(c) = &p.copyright {
        entry("copyright", string(c));
    }
    if let Some(t) = &p.trademark {
        entry("trademark", string(t));
    }
    entry("unitsPerEm", number(f64::from(p.upm)));
    entry("ascender", number(p.ascent));
    entry("descender", number(-p.descent));
    entry("italicAngle", number(p.italic_angle));
    if let Some(l) = &p.license {
        entry("openTypeNameLicense", string(l));
    }
    if let Some(u) = &p.license_url {
        entry("openTypeNameLicenseURL", string(u));
    }
    entry(
        "openTypeOS2WeightClass",
        number(if p.bold { 700.0 } else { 400.0 }),
    );
    entry("postscriptFontName", string(&p.postscript_name()));
    info.push_str("</dict>\n");
    files.push(("fontinfo.plist".into(), plist(&info)));

    files.push((
        "layercontents.plist".into(),
        plist("<array>\n  <array>\n    <string>public.default</string>\n    <string>glyphs</string>\n  </array>\n</array>\n"),
    ));

    let mut order = String::from("<dict>\n  <key>public.glyphOrder</key>\n  <array>\n");
    for n in &names {
        let _ = writeln!(order, "    <string>{}</string>", xml(n));
    }
    order.push_str("  </array>\n</dict>\n");
    files.push(("lib.plist".into(), plist(&order)));

    if !p.kerns.is_empty() {
        let mut k = String::from("<dict>\n");
        let mut kerns = p.kerns.clone();
        kerns.sort_by(|a, b| {
            names[a.0]
                .cmp(&names[b.0])
                .then(names[a.1].cmp(&names[b.1]))
        });
        let mut i = 0;
        while i < kerns.len() {
            let left = kerns[i].0;
            let _ = writeln!(k, "  <key>{}</key>\n  <dict>", xml(&names[left]));
            while i < kerns.len() && kerns[i].0 == left {
                let _ = writeln!(
                    k,
                    "    <key>{}</key>\n    {}",
                    xml(&names[kerns[i].1]),
                    number(kerns[i].2)
                );
                i += 1;
            }
            k.push_str("  </dict>\n");
        }
        k.push_str("</dict>\n");
        files.push(("kerning.plist".into(), plist(&k)));
    }

    if !p.ligatures.is_empty() {
        let mut fea = String::from(
            "languagesystem DFLT dflt;\nlanguagesystem latn dflt;\n\nfeature liga {\n",
        );
        for (comps, lig) in &p.ligatures {
            let comps: Vec<&str> = comps.iter().map(|c| names[*c].as_str()).collect();
            let _ = writeln!(fea, "    sub {} by {};", comps.join(" "), names[*lig]);
        }
        fea.push_str("} liga;\n");
        files.push(("features.fea".into(), fea));
    }

    let mut contents = String::from("<dict>\n");
    let mut taken: Vec<String> = Vec::new();
    let mut sorted: Vec<usize> = (0..names.len()).collect();
    sorted.sort_by(|a, b| names[*a].cmp(&names[*b]));
    let mut glif_files = vec![String::new(); names.len()];
    for i in sorted {
        // names that differ only in case are kept apart by a number
        let mut f = file_name(&names[i]);
        let mut n = 1;
        while taken.iter().any(|t| t.eq_ignore_ascii_case(&f)) {
            f = format!(
                "{}{n:015}.glif",
                file_name(&names[i]).trim_end_matches(".glif")
            );
            n += 1;
        }
        taken.push(f.clone());
        let _ = writeln!(
            contents,
            "  <key>{}</key>\n  <string>{}</string>",
            xml(&names[i]),
            xml(&f)
        );
        glif_files[i] = f;
    }
    contents.push_str("</dict>\n");
    files.push(("glyphs/contents.plist".into(), plist(&contents)));
    for (i, g) in p.glyphs.iter().enumerate() {
        files.push((
            format!("glyphs/{}", glif_files[i]),
            glif(&names[i], g.advance, g.unicode, &g.contours),
        ));
    }

    std::fs::create_dir_all(dir.join("glyphs"))
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for (path, text) in files {
        let at = dir.join(path);
        std::fs::write(&at, text).map_err(|e| format!("cannot write {}: {e}", at.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_names_follow_the_production_and_file_conventions() {
        assert_eq!(production_name("A"), "uni0041");
        assert_eq!(production_name("fi"), "uni0066_uni0069");
        assert_eq!(production_name("😀"), "u1F600");
        assert_eq!(file_name("uni00C5"), "uni00C_5.glif");
        assert_eq!(file_name(".notdef"), "_notdef.glif");
        assert_eq!(file_name("con"), "_con.glif");
        assert_eq!(file_name("a/b"), "a_b.glif");
    }
}
