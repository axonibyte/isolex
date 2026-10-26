//! `isolex font import` and `isolex font export` (#42): real fonts in,
//! isolex fonts out, and back again. The Noto Sans fixtures are OFL-1.1
//! test data (tests/fixtures/fonts/noto-sans/README.md); the synthetic
//! font is written here.
#![cfg(feature = "font-tools")]

use std::path::{Path, PathBuf};
use std::process::Command;

use isolex::affine::{path_segments, Seg};
use isolex::ir::IrShape;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_isolex"))
}

fn noto(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fonts/noto-sans")
        .join(file)
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("isolex-fonts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `isolex font import`, its output or its error.
fn import(font: &Path, args: &[&str]) -> Result<String, String> {
    let out = bin()
        .args(["font", "import"])
        .arg(font)
        .args(args)
        .output()
        .unwrap();
    if out.status.success() {
        Ok(String::from_utf8(out.stdout).unwrap())
    } else {
        Err(String::from_utf8(out.stderr).unwrap())
    }
}

/// `isolex font export` of `font` in a lexicon of `source`.
fn export(tag: &str, source: &str, font: &str, args: &[&str]) -> Result<PathBuf, String> {
    let dir = scratch(tag);
    std::fs::create_dir_all(dir.join("lex")).unwrap();
    std::fs::write(dir.join("lex/font.ilx"), source).unwrap();
    let ttf = dir.join("out.ttf");
    let out = bin()
        .args(["font", "export"])
        .arg(dir.join("lex"))
        .args(["--font", font, "--otf"])
        .arg(&ttf)
        .args(args)
        .output()
        .unwrap();
    if out.status.success() {
        Ok(ttf)
    } else {
        Err(String::from_utf8(out.stderr).unwrap())
    }
}

/// The font a source holds, every glyph drawn.
fn outline(source: &str, font: &str, styles: &[&str]) -> isolex::FontOutline {
    let styles: Vec<String> = styles.iter().map(|s| s.to_string()).collect();
    isolex::font_outline(&[("f.ilx".to_string(), source.to_string())], font, &styles).unwrap()
}

/// Every path a glyph draws as polylines, its curves sampled finely.
fn samples(shapes: &[IrShape]) -> Vec<Vec<[f64; 2]>> {
    let mut lines: Vec<Vec<[f64; 2]>> = Vec::new();
    for s in shapes {
        let IrShape::Path { d, .. } = s else { continue };
        let (mut cur, mut start) = ([0.0, 0.0], [0.0, 0.0]);
        let mut out: Vec<[f64; 2]> = Vec::new();
        for seg in path_segments(d).unwrap() {
            let along = |f: &dyn Fn(f64) -> [f64; 2], out: &mut Vec<[f64; 2]>| {
                for i in 0..=16 {
                    out.push(f(f64::from(i) / 16.0));
                }
            };
            match seg {
                Seg::Move(p) => {
                    if !out.is_empty() {
                        lines.push(std::mem::take(&mut out));
                    }
                    cur = p;
                    start = p;
                }
                Seg::Line(p) => {
                    let a = cur;
                    along(
                        &|t| [a[0] + (p[0] - a[0]) * t, a[1] + (p[1] - a[1]) * t],
                        &mut out,
                    );
                    cur = p;
                }
                Seg::Quad(c, p) => {
                    let a = cur;
                    along(
                        &|t| {
                            let u = 1.0 - t;
                            [
                                u * u * a[0] + 2.0 * u * t * c[0] + t * t * p[0],
                                u * u * a[1] + 2.0 * u * t * c[1] + t * t * p[1],
                            ]
                        },
                        &mut out,
                    );
                    cur = p;
                }
                Seg::Cubic(b, c, p) => {
                    let a = cur;
                    along(
                        &|t| {
                            let u = 1.0 - t;
                            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                            [
                                w[0] * a[0] + w[1] * b[0] + w[2] * c[0] + w[3] * p[0],
                                w[0] * a[1] + w[1] * b[1] + w[2] * c[1] + w[3] * p[1],
                            ]
                        },
                        &mut out,
                    );
                    cur = p;
                }
                Seg::Close => {
                    out.push(start);
                    cur = start;
                }
            }
        }
        if !out.is_empty() {
            lines.push(out);
        }
    }
    lines
}

/// How far `p` is from the segment from `a` to `b`.
fn to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx * dx + dy * dy;
    let t = if l == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l).clamp(0.0, 1.0)
    };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

/// How far the furthest point of either drawing is from the other's
/// lines: the Hausdorff distance between them, over their samples.
fn apart(a: &[Vec<[f64; 2]>], b: &[Vec<[f64; 2]>]) -> f64 {
    let one = |a: &[Vec<[f64; 2]>], b: &[Vec<[f64; 2]>]| {
        a.iter()
            .flatten()
            .map(|p| {
                b.iter()
                    .flat_map(|l| l.windows(2))
                    .map(|w| to_segment(*p, w[0], w[1]))
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(0.0, f64::max)
    };
    one(a, b).max(one(b, a))
}

/// Two fonts agree: metrics, glyphs and advances, kerning and notices
/// exactly, and every glyph's outline within `tol`.
fn assert_agree(a: &isolex::FontOutline, b: &isolex::FontOutline, tol: f64) {
    assert_eq!((a.em, a.ascent, a.descent), (b.em, b.ascent, b.descent));
    let names = |f: &isolex::FontOutline| {
        f.glyphs
            .iter()
            .map(|g| (g.name.clone(), g.advance))
            .collect::<Vec<_>>()
    };
    assert_eq!(names(a), names(b));
    assert_eq!(a.kerns, b.kerns);
    assert_eq!(a.describe, b.describe);
    for (g, h) in a.glyphs.iter().zip(&b.glyphs) {
        let (s, t) = (samples(&g.shapes), samples(&h.shapes));
        assert_eq!(s.is_empty(), t.is_empty(), "glyph {:?}", g.name);
        if !s.is_empty() {
            let d = apart(&s, &t);
            assert!(d <= tol, "glyph {:?} moved {d}", g.name);
        }
    }
}

#[test]
fn a_font_imports_to_its_golden_with_its_notices() {
    let got = import(&noto("noto-sans-regular.ttf"), &[]).unwrap();
    let path = noto("noto-sans-regular.ilx");
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(&path, &got).unwrap();
    }
    let want = std::fs::read_to_string(&path)
        .expect("golden missing: run with UPDATE_GOLDENS=1 once, then review the diff");
    assert_eq!(got, want, "the import drifted from its golden");
    for notice in [
        "describe \"Copyright: Copyright 2022 The Noto Project Authors",
        "describe \"Trademark: Noto is a trademark of Google LLC.\"",
        "describe \"License: This Font Software is licensed under the SIL Open Font License, Version 1.1.",
        "describe \"License URL: https://scripts.sil.org/OFL\"",
        "// It is a derivative of that font, and is under the font's license:",
    ] {
        assert!(got.contains(notice), "{notice}");
    }
    // ligatures from liga, kerning from GPOS
    let f = outline(&got, "noto-sans", &[]);
    for lig in ["fi", "fl", "ff", "ffi", "ffl"] {
        assert!(f.glyphs.iter().any(|g| g.name == lig), "{lig}");
    }
    assert!(f
        .kerns
        .iter()
        .any(|(l, r, by)| l == "A" && r == "V" && *by < 0.0));
    assert_eq!(f.glyphs.len(), 95 + 2 + 5);
}

#[test]
fn woff_and_its_font_import_alike() {
    let chars = ["--chars", "AVTaefilo 0123456789éÅ"];
    let body = |s: String| s.lines().skip(1).collect::<Vec<_>>().join("\n");
    let ttf = import(&noto("noto-sans-regular.ttf"), &chars).unwrap();
    let woff = import(&noto("noto-sans-regular.woff"), &chars).unwrap();
    assert_eq!(body(woff), body(ttf));
}

#[test]
fn a_cff_font_imports_its_cubics() {
    let otf = import(&noto("noto-sans-regular.otf"), &[]).unwrap();
    assert!(otf.contains(" C"), "cubic outlines");
    let f = outline(&otf, "noto-sans", &[]);
    assert!(f.glyphs.iter().any(|g| g.name == "A"));
}

#[test]
fn a_font_round_trips_through_truetype() {
    // TrueType outlines come back to within rounding to whole units
    let first = import(&noto("noto-sans-regular.ttf"), &[]).unwrap();
    let ttf = export("rt-ttf", &first, "noto-sans", &[]).unwrap();
    let again = import(&ttf, &[]).unwrap();
    assert_agree(
        &outline(&first, "noto-sans", &[]),
        &outline(&again, "noto-sans", &[]),
        1.0,
    );

    // and CFF's cubics, made quadratic, to within that and its tolerance
    let first = import(&noto("noto-sans-regular.otf"), &[]).unwrap();
    let ttf = export("rt-otf", &first, "noto-sans", &[]).unwrap();
    let again = import(&ttf, &[]).unwrap();
    assert_agree(
        &outline(&first, "noto-sans", &[]),
        &outline(&again, "noto-sans", &[]),
        1.5,
    );
}

/// A font drawn here: lines, quadratics, cubics and an arc, a counter, a
/// rect, a circle and a polygon, a stroke-built glyph, a ligature,
/// kerning, notdef, and notices.
const SYNTHETIC: &str = r#"
font "synth" {
  describe "Family: Synth Test"
  describe "Copyright: Copyright 2026 Axonibyte Innovations, LLC"
  describe "License: Licensed under the Apache License, Version 2.0"
  em 1000
  ascent 800
  descent 200
  kind "outline"
  glyph " " advance=300
  glyph "O" advance=700 {
    path d="M50,0 L650,0 Q650,350 650,700 C450,760 250,760 50,700 Z M200,150 L200,550 L500,550 L500,150 Z" fill="ink" stroke="none"
  }
  glyph "D" advance=600 {
    path d="M50,0 L300,0 A250 350 0 0 1 300,700 L50,700 Z" fill="ink" stroke="none"
  }
  glyph "I" advance=300 {
    rect 50 0 200 700 fill="ink"
    circle 150 850 60 fill="ink"
  }
  glyph "T" advance=600 {
    poly { pt 0 600; pt 600 600; pt 600 700; pt 0 700 }
    poly { pt 250 0; pt 350 0; pt 350 600; pt 250 600 }
  }
  glyph "TI" advance=850 {
    poly { pt 0 600; pt 850 600; pt 850 700; pt 0 700 }
  }
  notdef advance=500 {
    path d="M50,0 L450,0 L450,700 L50,700 Z M100,50 L100,650 L400,650 L400,50 Z" fill="ink" stroke="none"
  }
  kern "T" "O" -60
  kern "O" "T" -40
}
"#;

#[test]
fn a_font_drawn_in_isolex_round_trips_through_truetype() {
    let ttf = export("synth", SYNTHETIC, "synth", &[]).unwrap();
    let back = import(&ttf, &["--chars", " ODIT"]).unwrap();
    let a = outline(SYNTHETIC, "synth", &[]);
    let b = outline(&back, "synth-test", &[]);
    assert_eq!((a.em, a.ascent, a.descent), (b.em, b.ascent, b.descent));
    assert_eq!(a.kerns, b.kerns);
    // the notices come back, as notices
    for key in ["Copyright", "License"] {
        let line = a.describe.iter().find(|d| d.starts_with(key)).unwrap();
        assert!(b.describe.contains(line), "{line}");
    }
    let by = |f: &isolex::FontOutline, n: &str| {
        let g = f
            .glyphs
            .iter()
            .find(|g| g.name == n)
            .unwrap_or_else(|| panic!("{n}"));
        (g.advance, g.shapes.clone())
    };
    for n in ["O", "D", "I", "T", "TI"] {
        let (adv_a, sa) = by(&a, n);
        let (adv_b, sb) = by(&b, n);
        assert_eq!(adv_a, adv_b, "{n}");
        // the rect, circle and polygons as paths, to compare with
        let drawn = |shapes: &[IrShape]| -> Vec<Vec<[f64; 2]>> {
            let mut lines = samples(shapes);
            for s in shapes {
                match s {
                    IrShape::Polygon { points, .. } => {
                        let mut l = points.clone();
                        l.push(points[0]);
                        lines.push(l);
                    }
                    IrShape::Rect { x, y, w, h, .. } => {
                        lines.push(vec![
                            [*x, *y],
                            [x + w, *y],
                            [x + w, y + h],
                            [*x, y + h],
                            [*x, *y],
                        ]);
                    }
                    IrShape::Circle { cx, cy, r, .. } => {
                        lines.push(
                            (0..=256)
                                .map(|k| {
                                    let t = f64::from(k) / 256.0 * std::f64::consts::TAU;
                                    [cx + r * t.cos(), cy + r * t.sin()]
                                })
                                .collect(),
                        );
                    }
                    _ => {}
                }
            }
            lines
        };
        let d = apart(&drawn(&sa), &samples(&sb));
        // an arc and a circle are cubics, and cubics quadratics, so
        // within the export's tolerance and rounding to whole units
        assert!(d <= 1.5, "glyph {n} moved {d}");
    }
    // the ligature comes back as a ligature
    assert!(b.glyphs.iter().any(|g| g.name == "TI"));
}

#[test]
fn a_stroke_built_font_exports_as_outlines_of_its_strokes() {
    let source = include_str!("fixtures/text.ilx");
    let ttf = export("plain", source, "plain", &["--style", "bold"]).unwrap();
    let back = import(&ttf, &["--chars", "AV1-"]).unwrap();
    let f = outline(&back, "plain", &[]);
    // the hyphen, a line from (0.5, 3) to (3.5, 3) stroked 1.8 wide in
    // bold, with butt caps: a box, in an em of 10 written at 1000 units
    let hyphen = f.glyphs.iter().find(|g| g.name == "-").unwrap();
    let pts: Vec<[f64; 2]> = samples(&hyphen.shapes).concat();
    let b = pts
        .iter()
        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        });
    assert_eq!(b, [50.0, 210.0, 350.0, 390.0]);
    // the ligature of two hyphens survives as one
    assert!(f.glyphs.iter().any(|g| g.name == "--"), "{back}");
}

#[test]
fn export_is_deterministic_and_writes_a_ufo() {
    let a = std::fs::read(export("det-a", SYNTHETIC, "synth", &[]).unwrap()).unwrap();
    let b = std::fs::read(export("det-b", SYNTHETIC, "synth", &[]).unwrap()).unwrap();
    assert_eq!(a, b);

    let dir = scratch("ufo");
    std::fs::create_dir_all(dir.join("lex")).unwrap();
    std::fs::write(dir.join("lex/font.ilx"), SYNTHETIC).unwrap();
    let ufo = dir.join("synth.ufo");
    let run = || {
        bin()
            .args(["font", "export"])
            .arg(dir.join("lex"))
            .args(["--font", "synth", "--ufo"])
            .arg(&ufo)
            .output()
            .unwrap()
    };
    let out = run();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut files: Vec<String> = walk(&ufo);
    files.sort();
    assert_eq!(
        files,
        [
            "features.fea",
            "fontinfo.plist",
            "glyphs/_notdef.glif",
            "glyphs/contents.plist",
            "glyphs/uni0020.glif",
            "glyphs/uni0044.glif",
            "glyphs/uni0049.glif",
            "glyphs/uni004F_.glif",
            "glyphs/uni0054.glif",
            "glyphs/uni0054_uni0049.glif",
            "kerning.plist",
            "layercontents.plist",
            "lib.plist",
            "metainfo.plist",
        ]
        .map(str::to_string)
    );
    let fea = std::fs::read_to_string(ufo.join("features.fea")).unwrap();
    assert!(
        fea.contains("sub uni0054 uni0049 by uni0054_uni0049;"),
        "{fea}"
    );
    let info = std::fs::read_to_string(ufo.join("fontinfo.plist")).unwrap();
    assert!(info.contains("<key>openTypeNameLicense</key>\n  <string>Licensed under the Apache License, Version 2.0</string>"), "{info}");
    let o = std::fs::read_to_string(ufo.join("glyphs/uni004F_.glif")).unwrap();
    assert!(
        o.contains("type=\"curve\"") && o.contains("type=\"qcurve\""),
        "{o}"
    );
    // it never writes over what is there
    let out = run();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already exists"));
}

fn walk(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(
                walk(&p)
                    .into_iter()
                    .map(|f| format!("{}/{f}", p.file_name().unwrap().to_string_lossy())),
            );
        } else {
            out.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    out
}

#[test]
fn other_weights_and_variable_axes_become_styles() {
    // a bold from its own file, and from the variable font's wght axis
    let src = import(
        &noto("noto-sans-regular.ttf"),
        &[
            "--chars",
            "AVo1",
            "--style",
            &format!("weight.bold={}", noto("noto-sans-bold.ttf").display()),
        ],
    )
    .unwrap();
    assert!(src.contains("axes \"noto-sans-weight\""), "{src}");
    assert!(
        src.contains("axis \"noto-sans-weight\" {\n  variant \"bold\" {"),
        "{src}"
    );
    let (regular, bold) = (
        outline(&src, "noto-sans", &[]),
        outline(&src, "noto-sans", &["bold"]),
    );
    let adv = |f: &isolex::FontOutline| f.glyphs.iter().find(|g| g.name == "A").unwrap().advance;
    assert!(adv(&bold) > adv(&regular));

    let vf = import(
        &noto("noto-sans-vf.ttf"),
        &["--chars", "AVo1", "--style", "weight.bold=wght:700"],
    )
    .unwrap();
    let (regular, bold) = (
        outline(&vf, "noto-sans", &[]),
        outline(&vf, "noto-sans", &["bold"]),
    );
    assert!(
        adv(&bold) > adv(&regular),
        "{} {}",
        adv(&bold),
        adv(&regular)
    );

    let err = import(
        &noto("noto-sans-vf.ttf"),
        &["--style", "weight.bold=wgth:700"],
    )
    .unwrap_err();
    assert!(
        err.contains("has no axis 'wgth' (its axes: wght, wdth)"),
        "{err}"
    );
    let err = import(
        &noto("noto-sans-regular.ttf"),
        &["--style", "weight.bold=wght:700"],
    )
    .unwrap_err();
    assert!(err.contains("is not a variable font"), "{err}");
}

/// A font file with its OS/2 fsType set to `bits`.
fn with_fs_type(tag: &str, bits: u16) -> PathBuf {
    let mut data = std::fs::read(noto("noto-sans-regular.ttf")).unwrap();
    let n = usize::from(u16::from_be_bytes([data[4], data[5]]));
    let os2 = (0..n)
        .map(|i| 12 + 16 * i)
        .find(|at| &data[*at..*at + 4] == b"OS/2")
        .map(|at| u32::from_be_bytes(data[at + 8..at + 12].try_into().unwrap()) as usize)
        .unwrap();
    data[os2 + 8..os2 + 10].copy_from_slice(&bits.to_be_bytes());
    let path = scratch(tag).join("font.ttf");
    std::fs::write(&path, data).unwrap();
    path
}

#[test]
fn a_font_whose_license_forbids_it_is_not_converted() {
    let err = import(&with_fs_type("restricted", 0x0002), &[]).unwrap_err();
    assert!(err.contains("is marked restricted (OS/2 fsType)"), "{err}");
    let err = import(&with_fs_type("bitmaps", 0x0200), &[]).unwrap_err();
    assert!(
        err.contains("allows only its bitmaps to be embedded"),
        "{err}"
    );
    let whole = with_fs_type("whole", 0x0100);
    let err = import(&whole, &[]).unwrap_err();
    assert!(
        err.contains("may not be subset (OS/2 fsType); convert it whole with --chars all"),
        "{err}"
    );
    assert!(import(&whole, &["--chars", "all"]).is_ok());
    // editable and print-and-preview fonts may be converted
    assert!(import(&with_fs_type("editable", 0x0008), &["--chars", "A"]).is_ok());
}

#[test]
fn what_cannot_be_converted_says_why() {
    let dir = scratch("woff2");
    let woff2 = dir.join("f.woff2");
    std::fs::write(&woff2, b"wOF2 and nothing else").unwrap();
    let err = import(&woff2, &[]).unwrap_err();
    assert!(err.contains("WOFF2 is not read yet"), "{err}");
    let err = import(&noto("noto-sans-regular.ttf"), &["--chars", "あ"]).unwrap_err();
    assert!(
        err.contains("has no glyphs for the characters asked for"),
        "{err}"
    );
    let err = export("nofont", SYNTHETIC, "sinth", &[]).unwrap_err();
    assert!(
        err.contains("the lexicon has no font 'sinth' (did you mean 'synth'?)"),
        "{err}"
    );
    let err = export("nostyle", SYNTHETIC, "synth", &["--style", "bold"]).unwrap_err();
    assert!(err.contains("font 'synth' has no style 'bold'"), "{err}");
}

#[test]
fn every_imported_glyph_is_text_the_font_can_set() {
    // the converted font sets text, its kerning and ligatures included
    let src = import(&noto("noto-sans-regular.ttf"), &[]).unwrap();
    let lexicon = format!(
        "{src}\nicon \"t\" {{ category \"c\"; canvas 400 60\n  part \"p\" {{ at 0 0 0 {{ text 4 44 \"AVATAR fi To é\" font=\"noto-sans\" size=36 }} }}\n}}\n"
    );
    let c = isolex::compile(&[("t.ilx".to_string(), lexicon)]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    let groups = c.doc.icons[0]
        .shapes
        .iter()
        .filter(|s| matches!(s, IrShape::Group { .. }))
        .count();
    // A V A T A R, fi as one, T o, é: the spaces draw nothing
    assert_eq!(groups, 10);
}
