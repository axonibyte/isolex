//! Fonts in the studio (#84): a font's glyphs, metrics and kerning in its
//! outline, its styles in the tree, a glyph shown by a sketch whose shapes
//! say where in the font they are written, and fonts imported and
//! exported through a session (the `font-tools` feature).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use isolex_studio_core::compile::sketch;
use isolex_studio_core::outline::outline;
use isolex_studio_core::refactor::{add_font, RefactorError};
use isolex_studio_core::session::{Answer, Notice, Session, Sink};
use isolex_studio_core::tree::tree;
use isolex_studio_core::Library;

const TEXT: &str = include_str!("../../tests/fixtures/text.ilx");

fn files() -> Vec<(String, String)> {
    vec![("text.ilx".to_string(), TEXT.to_string())]
}

#[test]
fn a_font_lists_its_glyphs_as_parts_with_its_metrics_and_kerning() {
    let o = outline(&files(), "plain").unwrap();
    let names: Vec<(&str, &str)> = o
        .parts
        .iter()
        .map(|p| (p.op.as_str(), p.name.as_str()))
        .collect();
    assert_eq!(names[0], ("glyph", " "));
    assert!(names.contains(&("glyph", "--")), "a ligature, by its name");
    assert_eq!(names.last(), Some(&("notdef", "notdef")));
    let a = o.parts.iter().find(|p| p.name == "A").unwrap();
    assert_eq!(
        &TEXT[a.node.start..a.node.start + 21],
        "glyph \"A\" advance=5.5"
    );
    assert_eq!(a.shapes.len(), 1);
    assert_eq!(a.shapes[0].kind, "path");
    let font = o.font.unwrap();
    let metrics: Vec<(&str, &str)> = font
        .metrics
        .iter()
        .map(|m| (m.name.as_str(), m.value.as_str()))
        .collect();
    assert_eq!(
        metrics,
        [
            ("em", "10"),
            ("ascent", "7"),
            ("descent", "3"),
            ("kind", "stroke")
        ]
    );
    let kerns: Vec<(&str, &str, Option<f64>)> = font
        .kerns
        .iter()
        .map(|k| (k.left.as_str(), k.right.as_str(), k.value))
        .collect();
    assert_eq!(kerns, [("A", "V", Some(-1.0)), ("T", "o", Some(-0.8))]);
    // an icon has no font rows
    assert!(outline(&files(), "styles").unwrap().font.is_none());
}

#[test]
fn a_font_lists_its_styles_under_it_in_the_tree() {
    let t = tree(&files());
    let styles = |font: &str| -> Vec<(String, String)> {
        t.fonts
            .iter()
            .find(|f| f.name == font)
            .unwrap()
            .children
            .iter()
            .map(|s| (s.name.clone(), s.definition.clone()))
            .collect()
    };
    assert_eq!(
        styles("plain"),
        [
            ("plain bold".to_string(), "weight".to_string()),
            ("plain italic".to_string(), "slope".to_string())
        ]
    );
    assert_eq!(
        styles("block"),
        [("block bold".to_string(), "block-weight".to_string())]
    );
}

#[test]
fn a_sketch_sets_glyphs_whose_shapes_point_into_the_font() {
    let source = r#"icon "studio-sketch" {
  category "studio"
  canvas 40 20
  part "glyphs" {
    at 0 0 0 {
      text 2 15 "AV" font="plain" size=10
    }
  }
}
"#;
    let d = sketch(&files(), source).unwrap();
    let glyph_a = TEXT.find("glyph \"A\"").unwrap();
    let path_a = TEXT[glyph_a..].find("path").unwrap() + glyph_a;
    assert!(
        d.sources
            .iter()
            .any(|o| o.node.file == "text.ilx" && o.node.start == path_a && o.kind == "path"),
        "the A's path, where the font writes it"
    );
    // the map from the glyph's em grid to the canvas: y up, 1 em to 10
    let a = d.sources.iter().find(|o| o.node.start == path_a).unwrap();
    assert_eq!(a.map[0], 1.0);
    assert_eq!(a.map[3], -1.0);
    assert_eq!((a.map[6], a.map[7]), (2.0, 15.0));
    // a sketch that does not compile says why
    assert!(sketch(&files(), "icon \"studio-sketch\" {}").is_err());
}

#[test]
fn a_font_is_added_as_a_file_of_its_own_unless_its_name_is_taken() {
    let lib = Library::new("/nowhere", files().into_iter().collect());
    let tx = add_font(&lib, "font \"mono\" {\n  em 10\n}\n").unwrap();
    assert_eq!(tx.label, "Import font mono");
    let err = add_font(&lib, "font \"plain\" {\n  em 10\n}\n").unwrap_err();
    assert!(
        matches!(&err, RefactorError::NameTaken { name, free } if name == "plain" && free == "plain-2"),
        "{err:?}"
    );
}

#[derive(Default)]
struct Window(Mutex<Vec<Notice>>);

impl Sink for Window {
    fn notify(&self, n: Notice) {
        self.0.lock().unwrap().push(n);
    }
}

fn noto(file: &str) -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/fonts/noto-sans"
    ))
    .join(file)
}

fn scratch(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("isolex-studio-fonts-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session over the text fixture, in memory.
fn session() -> Session {
    let lib = Library::new("/nowhere", files().into_iter().collect());
    Session::in_memory(lib, Arc::new(Window::default()))
}

#[cfg(feature = "font-tools")]
mod tools {
    use super::*;
    use isolex_studio_core::session::FontImport;

    fn import(path: PathBuf, chars: &str) -> FontImport {
        FontImport {
            path: path.display().to_string(),
            name: Some("noto".into()),
            chars: chars.into(),
            ..FontImport::default()
        }
    }

    #[test]
    fn each_fixture_imports_as_a_font_that_compiles_and_undoes_as_one_step() {
        for file in [
            "noto-sans-regular.ttf",
            "noto-sans-regular.otf",
            "noto-sans-regular.woff",
            "noto-sans-vf.ttf",
        ] {
            let mut s = session();
            // the characters every fixture's subset has
            let answer = s.import_font(&import(noto(file), "AVT"));
            assert!(
                matches!(answer, Answer::Applied { .. }),
                "{file}: {answer:?}"
            );
            let lib = s.library();
            assert!(lib.files().contains_key("fonts/noto.ilx"), "{file}");
            let c = isolex::compile(&lib.sources());
            assert!(c.errors.is_empty(), "{file}: {:?}", c.errors);
            let o = outline(&lib.sources(), "noto").unwrap();
            let mut glyphs: Vec<&str> = o
                .parts
                .iter()
                .filter(|p| p.op == "glyph")
                .map(|p| p.name.as_str())
                .collect();
            glyphs.sort();
            assert_eq!(glyphs, ["A", "T", "V"], "{file}");
            assert_eq!(s.undo().as_deref(), Some("Import font noto"));
            assert!(!s.library().files().contains_key("fonts/noto.ilx"));
        }
    }

    #[test]
    fn a_style_comes_from_another_file_or_the_variable_font() {
        let mut s = session();
        let mut f = import(noto("noto-sans-vf.ttf"), "AB");
        f.styles = vec![("weight.bold".into(), "wght:700".into())];
        let answer = s.import_font(&f);
        assert!(matches!(answer, Answer::Applied { .. }), "{answer:?}");
        let t = tree(&s.library().sources());
        let noto = t.fonts.iter().find(|r| r.name == "noto").unwrap();
        assert_eq!(noto.children[0].name, "noto bold");
    }

    #[test]
    fn a_restricted_font_is_refused_saying_why() {
        let mut data = std::fs::read(noto("noto-sans-regular.ttf")).unwrap();
        let n = usize::from(u16::from_be_bytes([data[4], data[5]]));
        let os2 = (0..n)
            .map(|i| 12 + 16 * i)
            .find(|at| &data[*at..*at + 4] == b"OS/2")
            .map(|at| u32::from_be_bytes(data[at + 8..at + 12].try_into().unwrap()) as usize)
            .unwrap();
        data[os2 + 8..os2 + 10].copy_from_slice(&0x0002u16.to_be_bytes());
        let path = scratch("restricted").join("font.ttf");
        std::fs::write(&path, data).unwrap();
        let mut s = session();
        let Answer::Refused { reason } = s.import_font(&import(path, "A")) else {
            panic!("a restricted font is not converted")
        };
        assert!(reason.contains("restricted"), "{reason}");
        assert_eq!(s.library().revision(), 0, "nothing changed");
        // nor a font whose name is taken
        let mut f = import(noto("noto-sans-regular.ttf"), "A");
        f.name = Some("plain".into());
        let Answer::Refused { reason } = s.import_font(&f) else {
            panic!()
        };
        assert!(reason.contains("plain-2"), "{reason}");
    }

    #[test]
    fn an_exported_font_imports_again_drawing_the_same_glyphs() {
        let s = session();
        let out = scratch("export").join("plain.ttf");
        s.export_font("plain", &["bold".into()], &out.display().to_string(), false)
            .unwrap();
        let mut again = session();
        let mut f = import(out.clone(), "AVT0");
        f.name = Some("plain-again".into());
        let answer = again.import_font(&f);
        assert!(matches!(answer, Answer::Applied { .. }), "{answer:?}");
        let o = outline(&again.library().sources(), "plain-again").unwrap();
        let glyphs: BTreeMap<&str, usize> = o
            .parts
            .iter()
            .filter(|p| p.op == "glyph")
            .map(|p| (p.name.as_str(), p.shapes.len()))
            .collect();
        for g in ["A", "V", "T", "0"] {
            assert_eq!(
                glyphs.get(g),
                Some(&1),
                "{g} drawn as one outline: {glyphs:?}"
            );
        }
        // its kerning came along
        assert!(!o.font.unwrap().kerns.is_empty());
        // a UFO too, and a font the library does not have is refused
        let ufo = scratch("export").join("plain.ufo");
        s.export_font("plain", &[], &ufo.display().to_string(), true)
            .unwrap();
        assert!(ufo.join("fontinfo.plist").exists());
        assert!(s
            .export_font("nope", &[], &ufo.display().to_string(), true)
            .is_err());
    }
}
