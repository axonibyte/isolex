//! isolex — reference compiler for the Isolex icon-definition language.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use isolex::ir::IrDocument;
use isolex::render::{missing_colors, render_icon, safe_file_name, Palette};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("--version") | Some("-V") => {
            println!("isolex {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("check") if args.len() == 2 => run(&args[1], Mode::Check),
        Some("compile") => match args.as_slice() {
            [_, dir, flag, out] if flag == "--ir" => run(dir, Mode::CompileTo(out.clone())),
            _ => usage(),
        },
        Some("docs") if args.len() == 2 => run(&args[1], Mode::Docs),
        Some("render") => match args.as_slice() {
            [_, dir, flag, out] if flag == "--svg" => run(
                dir,
                Mode::Render {
                    out: out.clone(),
                    palette: None,
                },
            ),
            [_, dir, flag, out, pflag, palette] if flag == "--svg" && pflag == "--palette" => run(
                dir,
                Mode::Render {
                    out: out.clone(),
                    palette: Some(palette.clone()),
                },
            ),
            _ => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: isolex check <lexicon-dir>");
    eprintln!("       isolex compile <lexicon-dir> --ir <out.json|->");
    eprintln!("       isolex docs <lexicon-dir>");
    eprintln!("       isolex render <lexicon-dir> --svg <out-dir> [--palette <palette.json>]");
    ExitCode::from(2)
}

enum Mode {
    Check,
    CompileTo(String),
    Docs,
    /// One SVG per icon into `out`, colored by the built-in palette or
    /// the built-in one with a palette file laid over it.
    Render {
        out: String,
        palette: Option<String>,
    },
}

fn collect_ilx(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_ilx(&path, out)?;
        } else if path.extension().map(|e| e == "ilx").unwrap_or(false) {
            out.push(path);
        }
    }
    Ok(())
}

fn run(dir: &str, mode: Mode) -> ExitCode {
    let root = Path::new(dir);
    let mut paths = Vec::new();
    if let Err(e) = collect_ilx(root, &mut paths) {
        eprintln!("isolex: cannot read {dir}: {e}");
        return ExitCode::FAILURE;
    }
    if paths.is_empty() {
        eprintln!("isolex: no .ilx files under {dir}");
        return ExitCode::FAILURE;
    }
    let mut files = Vec::new();
    for p in &paths {
        match std::fs::read_to_string(p) {
            Ok(text) => files.push((p.display().to_string(), text)),
            Err(e) => {
                eprintln!("isolex: cannot read {}: {e}", p.display());
                return ExitCode::FAILURE;
            }
        }
    }

    let c = isolex::compile(&files);
    let doc = &c.doc;
    let diags = &c.errors;

    // Warnings say something has no effect; they never change the exit
    // status or the output (SPEC §8).
    for w in &c.warnings {
        eprintln!("{}:{}:{}: warning: {}", w.file, w.line, w.col, w.message);
    }
    for d in diags {
        eprintln!("{d}");
    }
    if !diags.is_empty() {
        eprintln!(
            "isolex: {} error(s); {} icon(s) would be emitted",
            diags.len(),
            doc.icons.len()
        );
        return ExitCode::FAILURE;
    }

    match mode {
        Mode::Check => {
            println!(
                "ok: {} definition(s), {} icon(s) emitted",
                c.definitions,
                doc.icons.len()
            );
            ExitCode::SUCCESS
        }
        Mode::CompileTo(out) => {
            let json = to_json(doc);
            if out == "-" {
                println!("{json}");
                ExitCode::SUCCESS
            } else {
                match std::fs::write(&out, json) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("isolex: cannot write {out}: {e}");
                        ExitCode::FAILURE
                    }
                }
            }
        }
        Mode::Docs => {
            print!("{}", docs_markdown(doc));
            ExitCode::SUCCESS
        }
        Mode::Render { out, palette } => render_svg(doc, &out, palette.as_deref()),
    }
}

/// One SVG per icon into `out`. Everything is checked first, the palette,
/// every token's color and every icon name as a file name, so a render
/// that fails writes nothing.
fn render_svg(doc: &IrDocument, out: &str, palette: Option<&str>) -> ExitCode {
    let palette = match palette {
        None => Palette::default(),
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => match Palette::with_json(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("isolex: {path}: {e}");
                    return ExitCode::FAILURE;
                }
            },
            Err(e) => {
                eprintln!("isolex: cannot read {path}: {e}");
                return ExitCode::FAILURE;
            }
        },
    };
    let missing = missing_colors(&doc.icons, &palette);
    for (token, icons) in &missing {
        let icons: Vec<&str> = icons.iter().map(|s| s.as_str()).collect();
        eprintln!(
            "isolex: the palette has no color for '{token}' (painted by {}); add it to a palette \
             file and pass --palette",
            icons.join(", ")
        );
    }
    let unsafe_names: Vec<&str> = doc
        .icons
        .iter()
        .map(|i| i.name.as_str())
        .filter(|n| !safe_file_name(n))
        .collect();
    for name in &unsafe_names {
        eprintln!(
            "isolex: icon '{name}' cannot be written as a file name; render takes names of \
             letters, digits, '-', '_' and '.', not starting with '.'"
        );
    }
    if !missing.is_empty() || !unsafe_names.is_empty() {
        return ExitCode::FAILURE;
    }
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("isolex: cannot create {out}: {e}");
        return ExitCode::FAILURE;
    }
    for icon in &doc.icons {
        let path = Path::new(out).join(format!("{}.svg", icon.name));
        if let Err(e) = std::fs::write(&path, render_icon(icon, &palette)) {
            eprintln!("isolex: cannot write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    println!("rendered {} icon(s) into {out}", doc.icons.len());
    ExitCode::SUCCESS
}

fn to_json(doc: &IrDocument) -> String {
    let mut s = serde_json::to_string_pretty(doc).expect("IR serializes");
    s.push('\n');
    s
}

/// Reference tables, one per category, from describe metadata —
/// deterministic order (category, then emission order).
fn docs_markdown(doc: &IrDocument) -> String {
    let mut categories: Vec<&str> = doc.icons.iter().map(|i| i.category.as_str()).collect();
    categories.sort_unstable();
    categories.dedup();

    let mut out = String::from("# Lexicon reference\n");
    for cat in categories {
        out.push_str(&format!(
            "\n## {cat}\n\n| Icon | Description |\n|---|---|\n"
        ));
        for icon in doc.icons.iter().filter(|i| i.category == cat) {
            out.push_str(&format!(
                "| `{}` | {} |\n",
                icon.name,
                icon.describe.join(" ").replace('|', "\\|")
            ));
        }
    }
    out
}
