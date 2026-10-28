//! isolex — reference compiler for the Isolex icon-definition language.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

#[cfg(feature = "font-tools")]
mod fonttools;

use isolex::ir::IrDocument;
use isolex::render::{missing_colors, render_icon, safe_file_name, Palette};
use isolex::HostValue;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("--version") | Some("-V") => {
            println!("isolex {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("check") if args.len() == 2 => run(&args[1], Mode::Check, None),
        Some("compile") => match args.as_slice() {
            [_, dir, flag, out, rest @ ..] if flag == "--ir" => match one_icon(rest) {
                Ok(only) => run(dir, Mode::CompileTo(out.clone()), only),
                Err(code) => code,
            },
            _ => usage(),
        },
        Some("docs") if args.len() == 2 => run(&args[1], Mode::Docs, None),
        #[cfg(feature = "font-tools")]
        Some("font") => fonttools::run(&args[1..]),
        #[cfg(not(feature = "font-tools"))]
        Some("font") => {
            eprintln!(
                "isolex: this isolex was built without font-tools, so it cannot convert fonts"
            );
            ExitCode::from(2)
        }
        Some("render") => match args.as_slice() {
            [_, dir, flag, out, rest @ ..] if flag == "--svg" => {
                let (palette, rest) = match rest {
                    [pflag, palette, rest @ ..] if pflag == "--palette" => {
                        (Some(palette.clone()), rest)
                    }
                    rest => (None, rest),
                };
                match one_icon(rest) {
                    Ok(only) => run(
                        dir,
                        Mode::Render {
                            out: out.clone(),
                            palette,
                        },
                        only,
                    ),
                    Err(code) => code,
                }
            }
            _ => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: isolex check <lexicon-dir>");
    eprintln!("       isolex compile <lexicon-dir> --ir <out.json|-> [<one icon>]");
    eprintln!("       isolex docs <lexicon-dir>");
    eprintln!(
        "       isolex render <lexicon-dir> --svg <out-dir> [--palette <palette.json>] [<one icon>]"
    );
    #[cfg(feature = "font-tools")]
    fonttools::usage();
    eprintln!("one icon: --icon <name> [--param <name>=<number>]… [--text <name>=<text>]…");
    ExitCode::from(2)
}

/// One icon to compile, by name, with params set on it.
type OneIcon = (String, BTreeMap<String, HostValue>);

/// One icon to compile, and params to set on it: `--icon name`, then any
/// of `--param name=number` and `--text name=text`. Nothing is no icon.
fn one_icon(args: &[String]) -> Result<Option<OneIcon>, ExitCode> {
    let [flag, icon, rest @ ..] = args else {
        return if args.is_empty() {
            Ok(None)
        } else {
            Err(usage())
        };
    };
    if flag != "--icon" {
        return Err(usage());
    }
    let mut params = BTreeMap::new();
    for pair in rest.chunks(2) {
        let [kind, setting] = pair else {
            return Err(usage());
        };
        let Some((name, value)) = setting.split_once('=') else {
            eprintln!("isolex: {kind} takes name=value, got '{setting}'");
            return Err(ExitCode::from(2));
        };
        let value = match kind.as_str() {
            "--param" => match value.parse::<f64>() {
                Ok(v) if v.is_finite() => HostValue::Num(v),
                _ => {
                    eprintln!("isolex: --param {name} takes a number, got '{value}'; text is set with --text");
                    return Err(ExitCode::from(2));
                }
            },
            "--text" => HostValue::Text(value.to_string()),
            _ => return Err(usage()),
        };
        if params.insert(name.to_string(), value).is_some() {
            eprintln!("isolex: param '{name}' is set twice");
            return Err(ExitCode::from(2));
        }
    }
    Ok(Some((icon.clone(), params)))
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

fn run(dir: &str, mode: Mode, only: Option<OneIcon>) -> ExitCode {
    let files = match isolex::library::load(Path::new(dir)) {
        Ok(f) => f,
        Err(why) => {
            eprintln!("isolex: {why}");
            return ExitCode::FAILURE;
        }
    };

    let c = match &only {
        None => isolex::compile(&files),
        Some((icon, params)) => match isolex::compile_one(&files, icon, params) {
            Ok(c) => c,
            Err(why) => {
                eprintln!("isolex: {why}");
                return ExitCode::FAILURE;
            }
        },
    };
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
