//! Exporting (#112): icons as SVG, the IR as JSON, and the library as a
//! zip, which *New library…* reads back.
//!
//! SVG and IR are what the command line writes for the same library:
//! `isolex render` (its light ground) and `isolex compile --ir`. A dark
//! SVG is drawn with the palette block's dark colors over the studio's
//! own dark defaults, as the canvas paints a dark ground. PNG is drawn
//! from these SVGs by the interface, in the webview, so nothing here
//! rasterizes.
//!
//! The zip holds the library's files as the studio has them, saved or
//! not, each at its path in the folder, deflated. Reading one takes only
//! library files, at safe paths, and drops one folder that wraps them
//! all, as a zip made of the folder itself has.

use std::collections::BTreeMap;
use std::path::Path;

use isolex::ir::IrDocument;
use isolex::render::{missing_colors, render_icon, safe_file_name, Palette};
use isolex::Compilation;
use serde::{Deserialize, Serialize};

use crate::library::{is_library_path, FilePath};

/// The ground an SVG is drawn for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ground {
    Light,
    Dark,
}

/// An exported icon's name and its SVG.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Svg {
    pub name: String,
    pub svg: String,
}

/// The tokens the studio paints a dark ground with when the palette block
/// does not say: the dark half of the interface's `DEFAULT_PALETTE`
/// (studio/ui/src/lib/theme.ts), whose light half is isolex's built-in
/// palette.
const DARK: [(&str, &str); 10] = [
    ("body-top", "#52606d"),
    ("body-left", "#3e4c59"),
    ("body-right", "#323f4b"),
    ("outline", "#0b0d10"),
    ("ink", "#e4e7eb"),
    ("ink-muted", "#9aa5b1"),
    ("shadow", "#000000"),
    ("virt", "#3e4c59"),
    ("accent", "#ef8a8a"),
    ("accent-dim", "#a05a5a"),
];

/// The library compiled, refused if it has errors: an export is of a
/// library that compiles, as the command line's is.
fn compiled(sources: &[(String, String)]) -> Result<Compilation, String> {
    let c = isolex::compile(sources);
    match c.errors.len() {
        0 => Ok(c),
        1 => Err("the library has an error; the Problems list says where".into()),
        n => Err(format!(
            "the library has {n} errors; the Problems list says where"
        )),
    }
}

/// The IR of `icons`, in the library's order, or of every icon; refused
/// naming an icon the library does not emit.
fn chosen(doc: &IrDocument, icons: Option<&[String]>) -> Result<IrDocument, String> {
    let Some(names) = icons else {
        return Ok(doc.clone());
    };
    if let Some(gone) = names
        .iter()
        .find(|n| !doc.icons.iter().any(|i| &i.name == *n))
    {
        return Err(format!("the library emits no icon '{gone}'"));
    }
    Ok(IrDocument {
        ir_version: doc.ir_version,
        icons: doc
            .icons
            .iter()
            .filter(|i| names.contains(&i.name))
            .cloned()
            .collect(),
    })
}

/// The palette for `ground`: on light, isolex's built-in palette with the
/// block's light colors over it, as `isolex render` draws; on dark, the
/// studio's dark defaults with the block's dark colors over them.
fn palette(c: &Compilation, ground: Ground) -> Palette {
    match ground {
        Ground::Light => match &c.palette {
            Some(colors) => Palette::with_colors(colors.iter().map(|(t, c)| (t, &c.light))),
            None => Palette::default(),
        },
        Ground::Dark => {
            let mut colors: BTreeMap<String, String> = DARK
                .iter()
                .map(|(t, c)| (t.to_string(), c.to_string()))
                .collect();
            for (t, c) in c.palette.iter().flatten() {
                colors.insert(t.clone(), c.dark.clone());
            }
            Palette::of_tokens(&colors)
        }
    }
}

/// `icons` (or every icon) as SVG on `ground`, each named as its file is.
/// Refused, before anything is drawn, for a token the palette has no
/// color for or a name that cannot be a file's.
pub fn svgs(
    sources: &[(String, String)],
    icons: Option<&[String]>,
    ground: Ground,
) -> Result<Vec<Svg>, String> {
    let c = compiled(sources)?;
    let doc = chosen(&c.doc, icons)?;
    let palette = palette(&c, ground);
    let missing = missing_colors(&doc.icons, &palette);
    if let Some((token, by)) = missing.iter().next() {
        let by: Vec<&str> = by.iter().map(|s| s.as_str()).collect();
        return Err(format!(
            "the palette has no color for '{token}' (painted by {}); add it to the lexicon's palette",
            by.join(", ")
        ));
    }
    if let Some(bad) = doc.icons.iter().find(|i| !safe_file_name(&i.name)) {
        return Err(format!(
            "icon '{}' cannot be written as a file name",
            bad.name
        ));
    }
    Ok(doc
        .icons
        .iter()
        .map(|i| Svg {
            name: i.name.clone(),
            svg: render_icon(i, &palette),
        })
        .collect())
}

/// The IR of `icons` (or every icon) as JSON, as `isolex compile --ir`
/// writes it.
pub fn ir(sources: &[(String, String)], icons: Option<&[String]>) -> Result<String, String> {
    let c = compiled(sources)?;
    let doc = chosen(&c.doc, icons)?;
    let mut json = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    json.push('\n');
    Ok(json)
}

/// An exported file the interface named, written to `out`, its folder
/// made if it is not there. Only what an export makes is written: a file
/// named `.svg`, `.png`, `.json` or `.zip`, and a `.png` or `.zip` only if
/// it is one.
pub fn write(out: &Path, data: &[u8]) -> Result<(), String> {
    const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let ext = out
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let fits = match ext.as_deref() {
        Some("svg" | "json") => true,
        Some("png") => data.starts_with(&PNG),
        Some("zip") => {
            data.starts_with(&LOCAL.to_le_bytes()) || data.starts_with(&END.to_le_bytes())
        }
        _ => {
            return Err(format!(
                "{} is not an SVG, PNG, JSON or zip file",
                out.display()
            ))
        }
    };
    if !fits {
        return Err(format!("what was made for {} is not one", out.display()));
    }
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    }
    std::fs::write(out, data).map_err(|e| format!("cannot write {}: {e}", out.display()))
}

// ZIP (APPNOTE 6.3): local headers, then the central directory, then its
// end record. Deflated, UTF-8 names, every time 1980-01-01 00:00 so the
// same library always makes the same bytes. No zip64: a library of icons
// is far below 4 GiB.

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
/// Names are UTF-8.
const UTF8: u16 = 1 << 11;
/// 1980-01-01, the first day a zip can say.
const DOS_DATE: u16 = (1 << 5) | 1;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

/// The library's files as a zip, each at its path in the folder.
pub fn zip(files: &[(String, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, text) in files {
        let data = text.as_bytes();
        let packed = miniz_oxide::deflate::compress_to_vec(data, 9);
        let crc = crc32(data);
        let offset = out.len() as u32;
        let fixed = |v: &mut Vec<u8>| {
            v.extend_from_slice(&20u16.to_le_bytes()); // version needed: 2.0
            v.extend_from_slice(&UTF8.to_le_bytes());
            v.extend_from_slice(&8u16.to_le_bytes()); // deflate
            v.extend_from_slice(&0u16.to_le_bytes()); // time
            v.extend_from_slice(&DOS_DATE.to_le_bytes());
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&(packed.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes()); // extra
        };
        out.extend_from_slice(&LOCAL.to_le_bytes());
        fixed(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);

        central.extend_from_slice(&CENTRAL.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // made by: 2.0
        fixed(&mut central);
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
        central.extend_from_slice(&0u32.to_le_bytes()); // external attributes
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&END.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0u16.to_le_bytes()); // the directory's disk
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment
    out
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The library files in a zip, by their path in the library: those whose
/// names are library paths, outside any hidden folder (`__MACOSX`'s and
/// dot-files are an archiver's, not the library's), with one folder that
/// wraps them all dropped. Refused if it is not a zip this can read, or
/// holds no library file.
pub fn unzip(data: &[u8]) -> Result<BTreeMap<FilePath, String>, String> {
    const NOT: &str = "that is not a zip this can read";
    // the end record is the last thing in a zip, before a comment of at
    // most 64 KiB
    let floor = data.len().saturating_sub(22 + 0xffff);
    let end = (floor..=data.len().saturating_sub(22))
        .rev()
        .find(|&i| u32_at(data, i) == Some(END))
        .ok_or(NOT)?;
    let count = usize::from(u16_at(data, end + 10).ok_or(NOT)?);
    let mut at = u32_at(data, end + 16).ok_or(NOT)? as usize;

    let mut files = BTreeMap::new();
    for _ in 0..count {
        if u32_at(data, at) != Some(CENTRAL) {
            return Err(NOT.into());
        }
        let method = u16_at(data, at + 10).ok_or(NOT)?;
        let crc = u32_at(data, at + 16).ok_or(NOT)?;
        let packed = u32_at(data, at + 20).ok_or(NOT)? as usize;
        let size = u32_at(data, at + 24).ok_or(NOT)? as usize;
        let name_len = usize::from(u16_at(data, at + 28).ok_or(NOT)?);
        let extra_len = usize::from(u16_at(data, at + 30).ok_or(NOT)?);
        let comment_len = usize::from(u16_at(data, at + 32).ok_or(NOT)?);
        let local = u32_at(data, at + 42).ok_or(NOT)? as usize;
        let name = data.get(at + 46..at + 46 + name_len).ok_or(NOT)?;
        at += 46 + name_len + extra_len + comment_len;

        let name = String::from_utf8_lossy(name).replace('\\', "/");
        let hidden = name
            .split('/')
            .any(|part| part.starts_with('.') || part == "__MACOSX");
        if hidden || !is_library_path(&name) {
            continue;
        }
        if packed == 0xffff_ffff || size == 0xffff_ffff {
            return Err("that zip is too large for a library (zip64)".into());
        }
        if u32_at(data, local) != Some(LOCAL) {
            return Err(NOT.into());
        }
        let start = local
            + 30
            + usize::from(u16_at(data, local + 26).ok_or(NOT)?)
            + usize::from(u16_at(data, local + 28).ok_or(NOT)?);
        let stored = data.get(start..start + packed).ok_or(NOT)?;
        let bytes = match method {
            0 => stored.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(stored, size)
                .map_err(|_| format!("{name} in the zip is damaged"))?,
            _ => {
                return Err(format!(
                    "{name} in the zip is packed in a way this cannot read"
                ))
            }
        };
        if bytes.len() != size || crc32(&bytes) != crc {
            return Err(format!("{name} in the zip is damaged"));
        }
        let text =
            String::from_utf8(bytes).map_err(|_| format!("{name} in the zip is not UTF-8 text"))?;
        files.insert(name, text);
    }
    if files.is_empty() {
        return Err("that zip holds no .ilx files".into());
    }
    // a zip of the folder itself wraps every file in the folder's name
    let first = files
        .keys()
        .next()
        .and_then(|p| p.split_once('/'))
        .map(|(d, _)| d.to_string());
    if let Some(dir) = first {
        let prefix = format!("{dir}/");
        if files.keys().all(|p| p.starts_with(&prefix)) {
            files = files
                .into_iter()
                .map(|(p, t)| (p[prefix.len()..].to_string(), t))
                .collect();
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<(String, String)> {
        vec![(
            "sample.ilx".to_string(),
            include_str!("../../tests/fixtures/sample.ilx").to_string(),
        )]
    }

    /// What `isolex render` draws, by the code its `Mode::Render` runs.
    fn rendered_as_the_cli_does(sources: &[(String, String)]) -> Vec<Svg> {
        let c = isolex::compile(sources);
        let palette = match &c.palette {
            Some(colors) => Palette::with_colors(colors.iter().map(|(t, c)| (t, &c.light))),
            None => Palette::default(),
        };
        c.doc
            .icons
            .iter()
            .map(|i| Svg {
                name: i.name.clone(),
                svg: render_icon(i, &palette),
            })
            .collect()
    }

    #[test]
    fn light_svgs_are_what_isolex_render_writes() {
        let got = svgs(&sample(), None, Ground::Light).unwrap();
        assert!(got.len() > 1);
        assert_eq!(got, rendered_as_the_cli_does(&sample()));
    }

    #[test]
    fn a_palette_block_colors_light_with_its_light_and_dark_with_its_dark() {
        let mut lib = sample();
        lib.push((
            "palette.ilx".into(),
            "palette {\n  accent light=\"#123456\" dark=\"#abcdef\"\n}\n".into(),
        ));
        let painted = |g| {
            svgs(&lib, None, g)
                .unwrap()
                .into_iter()
                .map(|s| s.svg)
                .collect::<String>()
        };
        let light = painted(Ground::Light);
        let dark = painted(Ground::Dark);
        assert!(light.contains("#123456") && !light.contains("#abcdef"));
        assert!(dark.contains("#abcdef") && !dark.contains("#123456"));
        // the tokens the block leaves out are the studio's dark defaults
        assert!(dark.contains("#52606d"), "body-top on dark");
        assert!(!dark.contains("#cbd2d9"), "no light body-left on dark");
    }

    #[test]
    fn chosen_icons_only_in_the_librarys_order() {
        let all = svgs(&sample(), None, Ground::Light).unwrap();
        let names: Vec<String> = all.iter().rev().take(2).map(|s| s.name.clone()).collect();
        let got = svgs(&sample(), Some(&names), Ground::Light).unwrap();
        let want: Vec<Svg> = all
            .into_iter()
            .filter(|s| names.contains(&s.name))
            .collect();
        assert_eq!(got, want);
        let err = svgs(&sample(), Some(&["no-such".to_string()]), Ground::Light).unwrap_err();
        assert!(err.contains("no-such"), "{err}");
    }

    #[test]
    fn the_ir_is_what_isolex_compile_writes() {
        let c = isolex::compile(&sample());
        let mut want = serde_json::to_string_pretty(&c.doc).unwrap();
        want.push('\n');
        assert_eq!(ir(&sample(), None).unwrap(), want);

        let one = c.doc.icons[0].name.clone();
        let doc: serde_json::Value =
            serde_json::from_str(&ir(&sample(), Some(std::slice::from_ref(&one))).unwrap())
                .unwrap();
        assert_eq!(doc["icons"].as_array().unwrap().len(), 1);
        assert_eq!(doc["icons"][0]["name"], one.as_str());
        assert_eq!(doc["ir_version"], isolex::ir::IR_VERSION);
    }

    #[test]
    fn a_library_with_errors_is_not_exported() {
        let lib = vec![("a.ilx".to_string(), "icon \"x\" {\n".to_string())];
        assert!(svgs(&lib, None, Ground::Light)
            .unwrap_err()
            .contains("error"));
        assert!(ir(&lib, None).unwrap_err().contains("error"));
    }

    #[test]
    fn a_token_without_a_color_is_refused_by_name() {
        let lib = vec![(
            "a.ilx".to_string(),
            "lexicon {\n  tokens \"ink\" \"outline\" \"mauve\"\n}\nicon \"x\" {\n  category \"c\"\n  footprint 1 1 1\n  part \"p\" {\n    tile 0 0 0 1 1 fill=\"mauve\"\n  }\n}\n"
                .to_string(),
        )];
        let c = isolex::compile(&lib);
        assert!(c.errors.is_empty(), "{:?}", c.errors);
        let err = svgs(&lib, None, Ground::Light).unwrap_err();
        assert!(err.contains("'mauve'") && err.contains('x'), "{err}");
    }

    #[test]
    fn a_zip_reads_back_as_the_library() {
        let lib = vec![
            ("lexicon.ilx".to_string(), "// ✓ lexicon\n".to_string()),
            ("racks/rack.ilx".to_string(), "x".repeat(5000)),
            ("empty.ilx".to_string(), String::new()),
        ];
        let z = zip(&lib);
        assert_eq!(z, zip(&lib), "the same library makes the same bytes");
        let back = unzip(&z).unwrap();
        assert_eq!(back, lib.into_iter().collect::<BTreeMap<_, _>>());
    }

    /// A zip as an archiver makes one of a folder: the folder's own name
    /// around every file, a directory entry, macOS's resource forks, a
    /// README, and some files stored rather than deflated.
    #[test]
    fn a_folders_zip_is_read_as_its_library() {
        let made = zip(&[
            ("icons/lexicon.ilx".to_string(), "a".to_string()),
            ("icons/sub/b.ilx".to_string(), "b".repeat(300)),
            ("icons/README.md".to_string(), "hi".to_string()),
            (
                "__MACOSX/icons/._lexicon.ilx".to_string(),
                "junk".to_string(),
            ),
            ("icons/.hidden.ilx".to_string(), "no".to_string()),
        ]);
        let back = unzip(&made).unwrap();
        let want: BTreeMap<String, String> = [
            ("lexicon.ilx".to_string(), "a".to_string()),
            ("sub/b.ilx".to_string(), "b".repeat(300)),
        ]
        .into_iter()
        .collect();
        assert_eq!(back, want);

        // stored, not deflated: method 0, its bytes as they are
        let stored = stored_zip(&[("x.ilx", "plain"), ("dir/", "")]);
        assert_eq!(unzip(&stored).unwrap()["x.ilx"], "plain");
    }

    fn stored_zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, text) in files {
            let offset = out.len() as u32;
            let head = |v: &mut Vec<u8>| {
                v.extend_from_slice(&20u16.to_le_bytes());
                v.extend_from_slice(&0u16.to_le_bytes());
                v.extend_from_slice(&0u16.to_le_bytes()); // stored
                v.extend_from_slice(&[0; 4]);
                v.extend_from_slice(&crc32(text.as_bytes()).to_le_bytes());
                v.extend_from_slice(&(text.len() as u32).to_le_bytes());
                v.extend_from_slice(&(text.len() as u32).to_le_bytes());
                v.extend_from_slice(&(name.len() as u16).to_le_bytes());
                v.extend_from_slice(&0u16.to_le_bytes());
            };
            out.extend_from_slice(&LOCAL.to_le_bytes());
            head(&mut out);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(text.as_bytes());
            central.extend_from_slice(&CENTRAL.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            head(&mut central);
            central.extend_from_slice(&[0; 10]);
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let at = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&END.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&at.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    #[test]
    fn a_damaged_or_foreign_zip_is_refused() {
        assert!(unzip(b"not a zip").is_err());
        let mut z = zip(&[("a.ilx".to_string(), "hello there".repeat(20))]);
        // a byte of the deflated data changed
        z[40] ^= 0xff;
        assert!(unzip(&z).is_err());
        let none = zip(&[("README.md".to_string(), "hi".to_string())]);
        assert!(unzip(&none).unwrap_err().contains(".ilx"));
        let escape = stored_zip(&[("../evil.ilx", "x")]);
        assert!(
            unzip(&escape).is_err(),
            "a path out of the folder is not taken"
        );
    }

    #[test]
    fn crc32_is_the_zip_checksum() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn only_what_an_export_makes_is_written() {
        let dir = std::env::temp_dir().join(format!("isolex-export-write-{}", std::process::id()));
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3];
        write(&dir.join("sub/a.png"), &png).unwrap();
        assert_eq!(std::fs::read(dir.join("sub/a.png")).unwrap(), png);
        write(&dir.join("a.svg"), b"<svg/>").unwrap();
        write(&dir.join("a.json"), b"{}").unwrap();
        write(&dir.join("a.zip"), &zip(&[("a.ilx".into(), "x".into())])).unwrap();
        write(&dir.join("empty.zip"), &zip(&[])).unwrap();
        for (name, data) in [
            ("b.png", &b"<svg/>"[..]),
            ("b.zip", b"text"),
            ("b.ilx", b"icon"),
            ("b.exe", b"MZ"),
            ("b", b""),
        ] {
            assert!(write(&dir.join(name), data).is_err(), "{name}");
            assert!(!dir.join(name).exists(), "{name}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
