//! `isolex font import` and `isolex font export`: standard fonts to isolex
//! fonts and back (SPEC §10). Built with the `font-tools` feature, the
//! default for the command line; the library and the WebAssembly module
//! never build it.

mod export;
mod import;
mod outline;
mod sfnt;
mod ufo;

use std::path::PathBuf;
use std::process::ExitCode;

use import::{Charset, Options, StyleSource};

pub fn usage() {
    eprintln!(
        "       isolex font import <font.ttf|.otf|.woff> [--out <file.ilx|->] [--name <name>]\n\
         \x20                         [--chars ascii|latin1|all|<characters>] [--em <n>]\n\
         \x20                         [--tolerance <n>] [--index <n>]\n\
         \x20                         [--style <axis>.<variant>=<font file|tag:value,…>]…\n\
         \x20      isolex font export <lexicon-dir> --font <name> [--style \"<styles>\"]\n\
         \x20                         (--otf <file.ttf> | --ufo <dir.ufo>) [--upm <n>] [--tolerance <n>]"
    );
}

fn bad(why: String) -> ExitCode {
    eprintln!("isolex: {why}");
    ExitCode::from(2)
}

/// `args` after `font`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(|s| s.as_str()) {
        Some("import") if args.len() >= 2 => run_import(&args[1], &args[2..]),
        Some("export") if args.len() >= 2 => run_export(&args[1], &args[2..]),
        _ => {
            eprintln!("usage:");
            usage();
            ExitCode::from(2)
        }
    }
}

fn run_import(input: &str, rest: &[String]) -> ExitCode {
    let mut opts = Options {
        input: PathBuf::from(input),
        name: None,
        chars: Charset::Latin1,
        em: None,
        tolerance: 0.0,
        index: 0,
        styles: Vec::new(),
    };
    let mut out = "-".to_string();
    for pair in rest.chunks(2) {
        let [flag, value] = pair else {
            return bad(format!("{} needs a value", pair[0]));
        };
        match flag.as_str() {
            "--out" => out = value.clone(),
            "--name" => opts.name = Some(value.clone()),
            "--chars" => {
                opts.chars = match value.as_str() {
                    "ascii" => Charset::Ascii,
                    "latin1" => Charset::Latin1,
                    "all" => Charset::All,
                    chars => Charset::Given(chars.chars().collect()),
                }
            }
            "--em" => match value.parse::<f64>() {
                Ok(v) if v > 0.0 && v.is_finite() => opts.em = Some(v),
                _ => return bad(format!("--em takes a number more than 0, got '{value}'")),
            },
            "--tolerance" => match value.parse::<f64>() {
                Ok(v) if v >= 0.0 && v.is_finite() => opts.tolerance = v,
                _ => {
                    return bad(format!(
                        "--tolerance takes a number, 0 or more, got '{value}'"
                    ))
                }
            },
            "--index" => match value.parse::<u32>() {
                Ok(v) => opts.index = v,
                _ => {
                    return bad(format!(
                        "--index takes a font's number in a collection, got '{value}'"
                    ))
                }
            },
            "--style" => {
                let parsed = value.split_once('=').and_then(|(name, source)| {
                    let (axis, variant) = name.split_once('.')?;
                    (!axis.is_empty() && !variant.is_empty()).then(|| StyleSource {
                        axis: axis.to_string(),
                        variant: variant.to_string(),
                        source: import::parse_source(source),
                    })
                });
                match parsed {
                    Some(s) => opts.styles.push(s),
                    None => {
                        return bad(format!(
                        "--style takes <axis>.<variant>=<font file or tag:value,…>, got '{value}'"
                    ))
                    }
                }
            }
            other => return bad(format!("font import takes no {other}")),
        }
    }
    let text = match import::import(&opts) {
        Ok(t) => t,
        Err(why) => {
            eprintln!("isolex: {why}");
            return ExitCode::FAILURE;
        }
    };
    // what it writes compiles, in every style it has
    let name = opts.name.clone().unwrap_or_default();
    let files = vec![("imported.ilx".to_string(), text.clone())];
    let font = text
        .lines()
        .find_map(|l| l.strip_prefix("font \"").and_then(|r| r.split('"').next()))
        .map(str::to_string)
        .unwrap_or(name);
    let mut checks: Vec<Vec<String>> = vec![Vec::new()];
    checks.extend(opts.styles.iter().map(|s| vec![s.variant.clone()]));
    for styles in checks {
        if let Err(ds) = isolex::font_outline(&files, &font, &styles) {
            for d in ds {
                eprintln!("isolex: the converted font does not compile: {d}");
            }
            return ExitCode::FAILURE;
        }
    }
    if out == "-" {
        print!("{text}");
    } else if let Err(e) = std::fs::write(&out, &text) {
        eprintln!("isolex: cannot write {out}: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_export(dir: &str, rest: &[String]) -> ExitCode {
    let (mut font, mut styles, mut otf, mut ufo, mut upm, mut tolerance) =
        (None, Vec::new(), None, None, None, None);
    for pair in rest.chunks(2) {
        let [flag, value] = pair else {
            return bad(format!("{} needs a value", pair[0]));
        };
        match flag.as_str() {
            "--font" => font = Some(value.clone()),
            "--style" => styles = value.split_whitespace().map(str::to_string).collect(),
            "--otf" => otf = Some(PathBuf::from(value)),
            "--ufo" => ufo = Some(PathBuf::from(value)),
            "--upm" => match value.parse::<u16>() {
                Ok(v) if (16..=16384).contains(&v) => upm = Some(v),
                _ => {
                    return bad(format!(
                        "--upm takes a whole number from 16 to 16384, got '{value}'"
                    ))
                }
            },
            "--tolerance" => match value.parse::<f64>() {
                Ok(v) if v > 0.0 && v.is_finite() => tolerance = Some(v),
                _ => {
                    return bad(format!(
                        "--tolerance takes a number more than 0, got '{value}'"
                    ))
                }
            },
            other => return bad(format!("font export takes no {other}")),
        }
    }
    let Some(font) = font else {
        return bad("font export needs --font <name>".to_string());
    };
    let (out, is_ufo) = match (otf, ufo) {
        (Some(o), None) => (o, false),
        (None, Some(u)) => (u, true),
        _ => return bad("font export writes one of --otf <file> or --ufo <dir>".to_string()),
    };
    let files = match crate::lexicon_files(dir) {
        Ok(f) => f,
        Err(why) => {
            eprintln!("isolex: {why}");
            return ExitCode::FAILURE;
        }
    };
    match export::export(&files, &font, &styles, &out, is_ufo, upm, tolerance) {
        Ok(said) => {
            println!("{said}");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in errors {
                eprintln!("{e}");
            }
            ExitCode::FAILURE
        }
    }
}
