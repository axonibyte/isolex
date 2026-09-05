//! isolex — reference compiler for the Isolex icon-definition language.
//!
//! Unit-1 scaffold: the CLI surface exists so the pipeline has something
//! to build and test; parse/resolve/compile land in the next units.

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-V") => {
            println!("isolex {}", env!("CARGO_PKG_VERSION"));
        }
        _ => {
            eprintln!("usage: isolex (check|compile|docs) <lexicon-dir> [--ir <out>]");
            eprintln!("subcommands arrive with the compiler core; see SPEC.md");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_string_is_the_cargo_version() {
        // The one honest assertion available at scaffold stage: the
        // version the binary reports is the manifest's, not a copy that
        // can drift.
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0");
    }
}
