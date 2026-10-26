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
        ("transforms.ilx", include_str!("fixtures/transforms.ilx")),
        (
            "transforms.ir.golden.json",
            include_str!("fixtures/transforms.ir.golden.json"),
        ),
        (
            "transforms.svg.golden",
            include_str!("fixtures/transforms.svg.golden"),
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
        // properties a node does not read were dropped without a word (#39)
        (
            "icon \"x\" {\n  part \"p\" {\n    box 0 0 0 1 1 1 fill=\"brand\"\n  }\n}",
            3,
            &["unknown property 'fill' on box; box takes fills, opacity and when", "did you mean 'fills'?"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    cylinder 0 0 0 1 1 fill=\"ink\"\n  }\n}",
            3,
            &["unknown property 'fill' on cylinder; cylinder takes top, side, opacity and when"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    shadow 0 0 1 1 opacity=0.5\n  }\n}",
            3,
            &["unknown property 'opacity' on shadow; shadow takes only when"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    poly3d {\n      pt 0 0 0 when=\"1\"\n    }\n  }\n}",
            4,
            &["unknown property 'when' on pt; pt takes no properties"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    at 0 0 0 {\n      circle \"x\" \"y\" 9 strok=\"ink\"\n    }\n  }\n}",
            4,
            &["unknown property 'strok' on circle", "did you mean 'stroke'?"],
        ),
        (
            "icon \"x\" {\n  part \"p\" when=\"1\" { }\n}",
            2,
            &["unknown property 'when' on part; part takes no properties"],
        ),
        (
            "icon \"x\" {\n  grant \"accent\" reason=\"red\" why=\"red\"\n}",
            2,
            &["unknown property 'why' on grant; grant takes only reason"],
        ),
        (
            "lexicon {\n  tokens \"ink\" fill=\"ink\"\n}",
            2,
            &["unknown property 'fill' on tokens; tokens takes no properties"],
        ),
        // and so was a string property given something else
        (
            "icon \"x\" {\n  part \"p\" {\n    tile 0 0 0 1 1 fill=3\n  }\n}",
            3,
            &["fill takes a string"],
        ),

        (
            "icon \"x\" {\n  part \"p\" {\n    transform rotat=90 { box 0 0 0 1 1 1 }\n  }\n}",
            3,
            &["unknown property 'rotat' on transform", "did you mean 'rotate'?"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform { box 0 0 0 1 1 1 }\n  }\n}",
            3,
            &["transform needs something to do"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform 90 { box 0 0 0 1 1 1 }\n  }\n}",
            3,
            &["transform takes properties, not arguments"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform rotate=90 {\n      steps { scale 2 }\n    }\n  }\n}",
            4,
            &["its properties or a steps block, not both"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform {\n      steps { spin 90 }\n    }\n  }\n}",
            4,
            &["unknown step 'spin'"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform {\n      steps { translate 1 }\n    }\n  }\n}",
            4,
            &["translate takes two numbers"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform matrix=\"1 0 0 1 0 0\" origin=\"1 1\" { }\n  }\n}",
            3,
            &["origin has nothing to do"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform matrix=\"1 0 0 1 0 0\" rotate=4 { }\n  }\n}",
            3,
            &["matrix stands alone"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform origin=\"1\" rotate=4 { }\n  }\n}",
            3,
            &["origin takes 2 number(s)"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform origin=\"w / 2 0\" rotate=4 { }\n  }\n}",
            3,
            &["origin takes 2 number(s)", "in parentheses"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform onto=\"top\" { }\n  }\n}",
            3,
            &["onto needs of"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform onto=\"side\" of=\"box 0 0 0 1 1 1\" { }\n  }\n}",
            3,
            &["onto is \"top\", \"front\" or \"right\""],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform onto=\"top\" of=\"box 0 0 0 1 1\" { }\n  }\n}",
            3,
            &["of names a box"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform rotate=4 bounds=\"crop\" { }\n  }\n}",
            3,
            &["bounds is \"error\", \"grow\", \"scale\" or \"clip\""],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform rotate=4 strokes=\"thin\" { }\n  }\n}",
            3,
            &["strokes is \"scale\" (the default) or \"fixed\""],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    transform mirror=\"sideways\" { }\n  }\n}",
            3,
            &["mirror is \"across\""],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    at 0 0 0 {\n      turn 90 { }\n    }\n  }\n}",
            4,
            &["'turn' is a 3-D node", "cannot appear inside 'at'"],
        ),
        (
            "icon \"x\" {\n  part \"p\" {\n    mirror-diagonal 1 { }\n  }\n}",
            3,
            &["mirror-diagonal takes 0 positional argument(s)"],
        ),
        (
            "icon \"x\" {\n  transform-bounds \"wrap\"\n}",
            2,
            &["transform-bounds is \"error\""],
        ),
        (
            "icon \"x\" {\n  view 90 180\n}",
            2,
            &["view takes one angle"],
        ),
        (
            "fragment \"f\" {\n  view 90\n}",
            2,
            &["set view on that icon"],
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

/// The other side of #39: every property a node reads is accepted, and
/// an unknown node is reported once, not once more per property.
#[test]
fn every_property_a_node_reads_is_accepted() {
    let src = r#"icon "x" extends="y" {
  part "p" {
    box 0 0 0 1 1 1 fills="virt" opacity=0.5 when="1 == 1"
    tile 0 0 0 1 1 fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1"
    cylinder 0 0 0 1 1 top="ink" side="ink" opacity=0.5 when="1 == 1"
    facerect "front" 0 0 0 1 1 fill="ink" opacity=0.5 when="1 == 1"
    poly3d fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1" {
      pt 0 0 0
    }
    shadow 0 0 1 1 when="1 == 1"
    seg 0 0 0 1 1 1 fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1"
    badge "b" corner="sw" scale=0.5 when="1 == 1"
    insert "f" anything=2 when="1 == 1"
    repeat 2 "i" when="1 == 1" { tile 0 0 0 1 1 }
    at 0 0 0 when="1 == 1" {
      line 0 0 1 1 fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1"
      circle 0 0 1 fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1"
      ellipse 0 0 1 1 fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1"
      rect 0 0 1 1 rx=1 fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1"
      arc 0 0 1 0 90 fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1"
      path d="M0,0 h1" fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1"
      poly fill="ink" stroke="ink" width=1.2 dash="2 1" opacity=0.5 when="1 == 1" {
        pt 0 0
      }
      transform origin="x y" rotate=1 when="1 == 1" { circle 0 0 1 }
    }
    transform origin="1 2" translate="1 2" rotate=3 scale="1 2" skew-x=1 skew-y=1 mirror="down" onto="front" of="box 0 0 0 1 1 1" bounds="clip" strokes="fixed" when="1 == 1" {
      tile 0 0 0 1 1
    }
    transform matrix="1 0 0 1 (w / 2) 0" { tile 0 0 0 1 1 }
    transform origin="1 1" {
      steps {
        translate 1 2
        rotate 3
        scale 2
        scale 1 2
        skew-x 1
        skew-y 1
        mirror "across"
        matrix 1 0 0 1 0 0
      }
      tile 0 0 0 1 1
    }
    turn 90 origin="1 1" when="1 == 1" { box 0 0 0 1 1 1 }
    mirror-diagonal origin="1 1" when="1 == 1" { box 0 0 0 1 1 1 }
  }
  view 90
  transform-bounds "grow"
  transform-chain 8
  grant "accent" reason="it is a warning"
}"#;
    let outcome = parse_one("t.ilx", src);
    assert_eq!(outcome.diags, vec![], "every property here is read");

    let outcome = parse_one(
        "t.ilx",
        "icon \"x\" {\n  part \"p\" {\n    bx 0 0 0 1 1 1 fill=\"ink\"\n  }\n}",
    );
    assert_eq!(outcome.diags.len(), 1, "{:?}", outcome.diags);
    assert!(outcome.diags[0]
        .message
        .contains("unknown geometry node 'bx'"));
}

/// The error is at the property, where an editor underlines it.
#[test]
fn an_unknown_property_is_reported_where_it_is_written() {
    let outcome = parse_one(
        "t.ilx",
        "icon \"x\" {\n  part \"p\" {\n    box 0 0 0 1 1 1 fill=\"brand\"\n  }\n}",
    );
    assert_eq!(outcome.diags.len(), 1, "{:?}", outcome.diags);
    assert_eq!((outcome.diags[0].line, outcome.diags[0].col), (3, 21));
}

#[test]
fn kdl_syntax_errors_are_located() {
    let outcome = parse_one("t.ilx", "icon \"x\" {\n  part \"p\" {\n");
    assert!(!outcome.diags.is_empty());
    assert!(outcome.diags[0].message.contains("KDL syntax error"));
    // the innermost block left open, and what is wrong with it
    assert_eq!(
        (outcome.diags[0].line, outcome.diags[0].col),
        (2, 12),
        "{}",
        outcome.diags[0]
    );
    assert_eq!(
        outcome.diags[0].message,
        "KDL syntax error: a { block is never closed with }"
    );
}

/// Blocks nested far past what the call stack could take are read,
/// built, cloned and dropped (#28): the kdl crate overflowed parsing a
/// few thousand.
#[test]
fn nesting_of_any_depth_is_parsed_cloned_and_dropped() {
    use isolex::ast::Node;
    const N: usize = 100_000;
    let src = format!(
        "icon \"deep\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{\n{}tile 0 0 0 1 1\n{} }} }}\n",
        "repeat 1 \"i\" {\n".repeat(N),
        "}\n".repeat(N)
    );
    let outcome = parse_one("deep.ilx", &src);
    assert_eq!(outcome.diags, vec![]);
    let mut node = &outcome.lexicon.defs[0].body.parts[0].nodes[0];
    let mut depth = 1;
    while let Some(child) = node.children().first() {
        node = child;
        depth += 1;
    }
    assert_eq!(depth, N + 1, "N repeats and the tile in the last");
    assert!(matches!(node, Node::Tile { .. }));
    let copy = outcome.lexicon.clone();
    assert_eq!(copy.defs[0].body.parts[0].nodes.len(), 1);
    drop(copy);
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
