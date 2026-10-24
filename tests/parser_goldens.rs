//! Parser contract tests: one rich fixture whose serialized AST is the
//! golden, plus table-driven diagnostics whose text and positions are
//! part of the compiler's compatibility surface.
//!
//! Regenerate the golden deliberately with UPDATE_GOLDENS=1 after a
//! reviewed change; the diff is the review artifact.

use isolex::parser::parse_lexicon;

fn parse_one(name: &str, text: &str) -> isolex::parser::ParseOutcome {
    parse_lexicon(&[(name.to_string(), text.to_string())])
}

#[test]
fn sample_lexicon_matches_golden() {
    let src = include_str!("fixtures/sample.ilx");
    let outcome = parse_one("sample.ilx", src);
    assert_eq!(outcome.diags, vec![], "the sample fixture must parse clean");
    let got = serde_json::to_string_pretty(&outcome.lexicon).unwrap();
    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sample.golden.json"
    );
    if std::env::var("UPDATE_GOLDENS").is_ok() {
        std::fs::write(golden_path, &got).unwrap();
    }
    let want = std::fs::read_to_string(golden_path)
        .expect("golden missing: run with UPDATE_GOLDENS=1 once, then review the diff");
    assert_eq!(got, want, "parser AST drifted from the golden");
}

/// The goldens compare bytes and count span offsets, so a checkout that
/// turned the fixtures into CRLF fails them all with an unhelpful diff;
/// .gitattributes pins LF, and this says so plainly if it ever stops.
#[test]
fn fixtures_are_checked_out_with_lf() {
    for (name, text) in [
        ("sample.ilx", include_str!("fixtures/sample.ilx")),
        (
            "sample.golden.json",
            include_str!("fixtures/sample.golden.json"),
        ),
        (
            "sample.ir.golden.json",
            include_str!("fixtures/sample.ir.golden.json"),
        ),
    ] {
        assert!(
            !text.contains('\r'),
            "tests/fixtures/{name} has CRLF line endings; check .gitattributes"
        );
    }
}

#[test]
fn double_parse_is_identical() {
    let src = include_str!("fixtures/sample.ilx");
    let a = serde_json::to_string(&parse_one("sample.ilx", src).lexicon).unwrap();
    let b = serde_json::to_string(&parse_one("sample.ilx", src).lexicon).unwrap();
    assert_eq!(a, b);
}

/// Each case: source, expected line, expected message fragments.
#[test]
fn diagnostics_name_the_problem_and_the_place() {
    let cases: &[(&str, usize, &[&str])] = &[
        (
            "icom \"x\" { }",
            1,
            &["unknown top-level node 'icom'", "did you mean 'icon'?"],
        ),
        (
            "icon \"x\" {\n  patr \"p\" { }\n}",
            2,
            &["unknown node 'patr'", "did you mean 'part'?"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    circle 0 0 4\n  }\n}",
            3,
            &["'circle' is a 2-D node", "'at' anchor group"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    at 0 0 0 {\n      box 0 0 0 1 1 1\n    }\n  }\n}",
            4,
            &["'box' is a 3-D node", "cannot appear inside 'at'"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    pt 1 2 3\n  }\n}",
            3,
            &["'pt' is only valid inside poly"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    box 0 0 0 1 1 \"u ** 2\"\n  }\n}",
            3,
            &["in expression 'u ** 2'", "expected a value"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    box 0 0 0 1 1 \"h = 2\"\n  }\n}",
            3,
            &["assignment does not exist"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    box 0 0 0 1 1\n  }\n}",
            3,
            &["box takes 6 positional argument(s), found 5"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    tile 0 0 0 1 1 when=\"i + 1\"\n  }\n}",
            3,
            &["in when guard 'i + 1'", "requires a Bool"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    at 0 0 0 {\n      path d=\"M{x,{y}\"\n    }\n  }\n}",
            4,
            &["in path template expression"],
        ),
        (
            "icon \"x\" {\n  param bays\n}",
            2,
            &["param takes a name and a default"],
        ),
        (
            "axis \"a\" {\n  emit-base \"yes\"\n}",
            2,
            &["emit-base takes #true or #false"],
        ),
        // both used to be skipped without a word
        (
            "lexicon {\n  stroke-widths 1.2 \"thick\"\n}",
            2,
            &["stroke-widths takes positive numbers"],
        ),
        (
            "lexicon {\n  stroke-widths 0\n}",
            2,
            &["stroke-widths takes positive numbers"],
        ),
        (
            "lexicon {\n  tokens \"ink\" \"accent\"\n  reserved \"acent\"\n}",
            3,
            &[
                "'acent' is reserved but is not one of the lexicon's tokens",
                "did you mean 'accent'?",
            ],
        ),
        (
            "fragment \"f\" {\n  grant \"accent\" reason=\"red\"\n}",
            2,
            &["a fragment cannot grant", "grant on that icon"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    box 0 0 0 1 1 1 fills=\"bodie\"\n  }\n}",
            3,
            &["fills is \"body\", \"virt\" or \"none\"", "did you mean 'body'?"],
        ),
        (
            "icon \"x\" extendz=\"y\" { }",
            1,
            &["unknown property 'extendz'", "did you mean 'extends'?"],
        ),
        ("icon { }", 1, &["needs a name"]),
        (
            "icon \"x\" {\n  part \"p\" {\n    box 0 0 0 1 1 #inf\n  }\n}",
            3,
            &["#inf is not a finite number"],
        ),
        (
            "icon \"x\" {\n  param a #-inf\n}",
            2,
            &["#-inf is not a finite number"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    tile 0 0 0 1 1 opacity=#nan\n  }\n}",
            3,
            &["#nan is not a finite number"],
        ),
        (
            "icon \"x\" {\n  shape-limit 0\n}",
            2,
            &["shape-limit takes a positive whole number, or #null for no limit"],
        ),
        (
            "lexicon {\n  repeat-limit 2.5\n}",
            2,
            &["repeat-limit takes a positive whole number, or #null for no limit"],
        ),
        (
            "icon \"x\" {\n  insert-depth \"deep\"\n}",
            2,
            &["insert-depth takes a positive whole number, or #null for no limit"],
        ),
        (
            "icon \"x\" {\n  shape-limit 5\n  shape-limit 6\n}",
            3,
            &["shape-limit is set twice here"],
        ),
        (
            "icon \"x\" {\n  icon-limit 5\n}",
            2,
            &["icon-limit counts the whole lexicon; set it in the lexicon block"],
        ),
        (
            "fragment \"f\" {\n  shape-limit 5\n}",
            2,
            &["a fragment's work counts toward the icon it is inserted into"],
        ),
        (
            "icon \"x\" {\n  shape-limimt 5\n}",
            2,
            &["unknown node 'shape-limimt'", "did you mean 'shape-limit'?"],
        ),
        (
            "lexicon {\n  icon-limt 5\n}",
            2,
            &[
                "unknown lexicon setting 'icon-limt'",
                "did you mean 'icon-limit'?",
            ],
        ),
        // these four were accepted and then ignored
        (
            "axis \"a\" {\n  variant \"v\" {\n    extends \"b\"\n  }\n}",
            3,
            &["a variant cannot extend"],
        ),
        (
            "axis \"a\" {\n  category \"c\"\n}",
            2,
            &["an axis holds only variant and emit-base", "'category'"],
        ),
        (
            "axis \"a\" {\n  part \"p\" { tile 0 0 0 1 1 }\n}",
            2,
            &["an axis holds only variant and emit-base", "'part'"],
        ),
        ("axis \"a\" extends=\"b\" { }", 1, &["an axis cannot extend"]),
        // and these two were overridden by the anchor inside `at`
        (
            "icon \"x\" {\n  param x 5\n}",
            2,
            &["'x' is reserved for the anchor's screen position"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    repeat 3 \"y\" {\n      tile 0 0 0 1 1\n    }\n  }\n}",
            3,
            &["'y' is reserved for the anchor's screen position"],
        ),
        (
            "icon \"x\" {\n  grant \"accent\"\n}",
            2,
            &["grant takes a token and a reason"],
        ),
    ];

    for (src, line, fragments) in cases {
        let outcome = parse_one("t.ilx", src);
        assert!(
            !outcome.diags.is_empty(),
            "expected a diagnostic for: {src}"
        );
        let d = &outcome.diags[0];
        assert_eq!(
            d.line, *line,
            "wrong line for {src:?}: got {d} (wanted line {line})"
        );
        for frag in *fragments {
            assert!(
                d.message.contains(frag),
                "diagnostic for {src:?} should contain {frag:?}, got: {}",
                d.message
            );
        }
    }
}

#[test]
fn kdl_syntax_errors_are_located() {
    let outcome = parse_one("t.ilx", "icon \"x\" {\n  part \"p\" {\n");
    assert!(!outcome.diags.is_empty());
    assert!(outcome.diags[0].message.contains("KDL syntax error"));
}

#[test]
fn files_parse_in_given_order_and_spans_index_files() {
    let outcome = parse_lexicon(&[
        ("a.ilx".into(), "icon \"one\" { }".into()),
        ("b.ilx".into(), "icon \"two\" { }".into()),
    ]);
    assert_eq!(outcome.lexicon.defs.len(), 2);
    assert_eq!(outcome.lexicon.defs[0].name, "one");
    assert_eq!(outcome.lexicon.defs[0].span.file, 0);
    assert_eq!(outcome.lexicon.defs[1].span.file, 1);
}
