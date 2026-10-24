//! One test per resolution/expansion rule. Each is written so a single
//! resolver or expander branch flipping breaks it (and each was observed
//! failing during development by breaking that branch).

use isolex::expand::{expand, ExpandedIcon, Shape};
use isolex::parser::parse_lexicon;
use isolex::resolve::resolve;

fn compile(src: &str) -> (Vec<ExpandedIcon>, Vec<String>) {
    let outcome = parse_lexicon(&[("t.ilx".into(), src.into())]);
    assert_eq!(outcome.diags, vec![], "fixture must parse clean");
    let resolved = resolve(&outcome.lexicon);
    let (icons, mut diags) = expand(&resolved);
    let mut msgs: Vec<String> = resolved.diags.iter().map(|d| d.message.clone()).collect();
    msgs.extend(diags.drain(..).map(|d| d.message));
    (icons, msgs)
}

fn compile_ok(src: &str) -> Vec<ExpandedIcon> {
    let (icons, msgs) = compile(src);
    assert_eq!(msgs, Vec::<String>::new(), "expected a clean compile");
    icons
}

fn boxes(icon: &ExpandedIcon) -> Vec<(f64, f64, f64, f64, f64, f64)> {
    icon.shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Box3 {
                x, y, z, w, d, h, ..
            } => Some((*x, *y, *z, *w, *d, *h)),
            _ => None,
        })
        .collect()
}

const CHASSIS: &str = r#"
base "rack-unit" {
  category "net"
  param u 1
  footprint 2 1 "u * 0.28"
  part "chassis" { box 0 0 0 2 1 "u * 0.28" }
}
"#;

#[test]
fn late_binding_parent_geometry_sees_child_params() {
    let src = format!("{CHASSIS}\nicon \"big\" {{ extends \"rack-unit\"\n  param u 2\n}}");
    let icons = compile_ok(&src);
    assert_eq!(icons.len(), 1);
    assert_eq!(icons[0].footprint, Some([2.0, 1.0, 0.56]));
    assert_eq!(
        boxes(&icons[0])[0].5,
        0.56,
        "chassis height uses the child's u"
    );
}

#[test]
fn params_reference_earlier_params_in_order() {
    let src = r#"
icon "x" {
  category "c"
  param a 3
  param b "a * 2"
  canvas 10 10
  part "p" { at 0 0 0 { circle "b" 0 1 } }
}
"#;
    compile_ok(src);
}

#[test]
fn child_parts_append_after_inherited_parts() {
    let src = format!(
        "{CHASSIS}\nicon \"kid\" {{ extends \"rack-unit\"\n  part \"extra\" {{ box 9 9 9 1 1 1 }}\n}}"
    );
    let icons = compile_ok(&src);
    let b = boxes(&icons[0]);
    assert_eq!(b.len(), 2);
    assert_eq!(b[0].0, 0.0, "inherited chassis renders first");
    assert_eq!(b[1].0, 9.0, "appended part renders after");
}

#[test]
fn replace_keeps_the_inherited_z_slot() {
    let src = r#"
base "b" {
  category "c"
  footprint 1 1 1
  part "first" { box 1 0 0 1 1 1 }
  part "second" { box 2 0 0 1 1 1 }
  part "third" { box 3 0 0 1 1 1 }
}
icon "kid" {
  extends "b"
  replace "second" { box 20 0 0 1 1 1 }
}
"#;
    let icons = compile_ok(src);
    let b = boxes(&icons[0]);
    assert_eq!(
        vec![b[0].0, b[1].0, b[2].0],
        vec![1.0, 20.0, 3.0],
        "replacement stays in the middle slot"
    );
}

#[test]
fn remove_deletes_an_inherited_part() {
    let src = format!(
        "{CHASSIS}\nicon \"gone\" {{ extends \"rack-unit\"\n  remove \"chassis\"\n  part \"only\" {{ box 5 0 0 1 1 1 }}\n}}"
    );
    let icons = compile_ok(&src);
    let b = boxes(&icons[0]);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].0, 5.0);
}

#[test]
fn shadowing_without_replace_is_an_error() {
    let src = format!(
        "{CHASSIS}\nicon \"shadow\" {{ extends \"rack-unit\"\n  part \"chassis\" {{ box 0 0 0 1 1 1 }}\n}}"
    );
    let (_, msgs) = compile(&src);
    assert!(
        msgs.iter()
            .any(|m| m.contains("already exists on an ancestor") && m.contains("silent shadowing")),
        "{msgs:?}"
    );
}

#[test]
fn replace_missing_part_suggests_a_name() {
    let src = format!(
        "{CHASSIS}\nicon \"typo\" {{ extends \"rack-unit\"\n  replace \"chasis\" {{ box 0 0 0 1 1 1 }}\n}}"
    );
    let (_, msgs) = compile(&src);
    assert!(
        msgs.iter()
            .any(|m| m.contains("no ancestor defines it") && m.contains("did you mean 'chassis'?")),
        "{msgs:?}"
    );
}

#[test]
fn extends_cycles_report_the_chain() {
    let src = r#"
base "a" { extends "b" }
base "b" { extends "a" }
"#;
    let (_, msgs) = compile(src);
    assert!(
        msgs.iter()
            .any(|m| m.contains("extends cycle") && m.contains("a -> b -> a")),
        "{msgs:?}"
    );
}

#[test]
fn unknown_parent_suggests_a_name() {
    let src = format!("{CHASSIS}\nicon \"kid\" {{ extends \"rack-unti\" }}");
    let (_, msgs) = compile(&src);
    assert!(
        msgs.iter()
            .any(|m| m.contains("unknown definition 'rack-unti'")
                && m.contains("did you mean 'rack-unit'?")),
        "{msgs:?}"
    );
}

#[test]
fn bases_are_never_emitted_and_incomplete_icons_error() {
    let icons = compile_ok(CHASSIS);
    assert_eq!(icons.len(), 0, "a lone base emits nothing");

    let (_, msgs) = compile("icon \"naked\" { part \"p\" { box 0 0 0 1 1 1 } }");
    assert!(
        msgs.iter()
            .any(|m| m.contains("resolves without category and footprint or canvas")),
        "{msgs:?}"
    );
}

#[test]
fn extending_a_fragment_is_an_error() {
    let src = r#"
fragment "f" { part "p" { box 0 0 0 1 1 1 } }
icon "x" { extends "f"; category "c"; footprint 1 1 1 }
"#;
    let (_, msgs) = compile(src);
    assert!(
        msgs.iter()
            .any(|m| m.contains("only base and icon definitions can be extended")),
        "{msgs:?}"
    );
}

const STATE_AXIS: &str = r#"
axis "state" {
  variant "down" { part "badge-down" { box 90 0 0 1 1 1 } }
  variant "planned" { part "badge-planned" { box 91 0 0 1 1 1 } }
}
"#;

#[test]
fn axes_expand_products_in_declared_order() {
    let src = format!(
        "{STATE_AXIS}
axis \"vendor\" {{
  emit-base #false
  variant \"acme\" {{ part \"mark\" {{ box 95 0 0 1 1 1 }} }}
}}
icon \"srv\" {{
  category \"c\"
  footprint 1 1 1
  part \"body\" {{ box 0 0 0 1 1 1 }}
  axes \"state\" \"vendor\"
}}"
    );
    let icons = compile_ok(&src);
    let names: Vec<&str> = icons.iter().map(|i| i.name.as_str()).collect();
    // vendor has emit-base false: every combo carries -acme; state offers
    // a bare option — so no bare "srv"
    assert_eq!(
        names,
        vec!["srv-acme", "srv-down-acme", "srv-planned-acme"],
        "product order follows axis then variant declaration order"
    );
}

#[test]
fn axes_union_down_the_inheritance_chain() {
    let src = format!(
        "{STATE_AXIS}
base \"primary\" {{ category \"c\"; footprint 1 1 1; axes \"state\"
  part \"body\" {{ box 0 0 0 1 1 1 }} }}
icon \"srv\" {{ extends \"primary\" }}"
    );
    let icons = compile_ok(&src);
    let names: Vec<&str> = icons.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["srv", "srv-down", "srv-planned"]);
    assert_eq!(boxes(&icons[1]).len(), 2, "variant part lands on the child");
}

#[test]
fn repeat_when_and_counts() {
    let src = r#"
icon "x" {
  category "c"
  footprint 1 1 1
  param n 4
  part "row" {
    repeat "n" "i" {
      box "i" 0 0 1 1 1 when="i != 2"
    }
  }
}
"#;
    let icons = compile_ok(src);
    let xs: Vec<f64> = boxes(&icons[0]).iter().map(|b| b.0).collect();
    assert_eq!(xs, vec![0.0, 1.0, 3.0], "the guard skipped i == 2");

    // a repeat has no cap of its own any more: past the old 10 000 is fine
    // while the icon stays within its shape-limit and repeat-limit
    let icons = compile_ok(
        "icon \"y\" { category \"c\"; footprint 1 1 1; part \"p\" { repeat 10001 \"i\" { tile 0 0 0 1 1 } } }",
    );
    assert_eq!(icons[0].shapes.len(), 10_001);

    let (_, msgs) = compile(
        "icon \"z\" { category \"c\"; footprint 1 1 1; part \"p\" { repeat 2.5 \"i\" { box 0 0 0 1 1 1 } } }",
    );
    assert!(
        msgs.iter().any(|m| m.contains("non-negative integer")),
        "{msgs:?}"
    );
}

#[test]
fn nested_repeats_stop_at_the_shape_limit() {
    // two repeats, each well under any per-repeat cap, multiply to 101 000
    // tiles; before limits this ran on to 100 million and out of memory
    let (icons, msgs) = compile(
        "icon \"big\" { category \"c\"; footprint 1 1 1; part \"p\" {\n  repeat 1000 \"a\" { repeat 101 \"b\" { tile 0 0 0 1 1 } } } }",
    );
    assert_eq!(
        msgs,
        vec![
            "icon 'big' goes here past its shape-limit of 100000 (shapes and points, the \
             default); if that is intended, raise it in this icon's definition or the lexicon \
             block, or set `shape-limit #null` for no limit"
        ]
    );
    assert_eq!(
        icons[0].shapes.len(),
        100_000,
        "expansion stopped at the limit"
    );
}

#[test]
fn the_shape_limit_can_be_raised_lowered_or_switched_off() {
    let six = "part \"p\" { repeat 6 \"i\" { tile 0 0 0 1 1 } }";
    let lexicon = "lexicon { shape-limit 5 }\n";

    let (_, msgs) = compile(&format!(
        "{lexicon}icon \"i\" {{ category \"c\"; footprint 1 1 1; {six} }}"
    ));
    assert!(
        msgs.len() == 1
            && msgs[0].contains("shape-limit of 5 (shapes and points, set in the lexicon block)"),
        "{msgs:?}"
    );

    // a definition's value beats the lexicon block's
    compile_ok(&format!(
        "{lexicon}icon \"i\" {{ category \"c\"; footprint 1 1 1; shape-limit 10; {six} }}"
    ));
    // #null switches it off
    compile_ok(&format!(
        "{lexicon}icon \"i\" {{ category \"c\"; footprint 1 1 1; shape-limit #null; {six} }}"
    ));

    // inherited from a base, and a child's own value beats its parent's
    let base = "base \"b\" { category \"c\"; footprint 1 1 1; shape-limit 3 }\n";
    let (_, msgs) = compile(&format!("{base}icon \"i\" {{ extends \"b\"; {six} }}"));
    assert!(
        msgs.len() == 1 && msgs[0].contains("shape-limit of 3 (shapes and points, set by 'b')"),
        "{msgs:?}"
    );
    compile_ok(&format!(
        "{base}icon \"i\" {{ extends \"b\"; shape-limit 6; {six} }}"
    ));
}

#[test]
fn polygon_points_count_toward_the_shape_limit() {
    // the polygon and its four points are five
    let poly = "part \"p\" { poly3d { pt 0 0 0; pt 1 0 0; pt 1 1 0; pt 0 1 0 } }";
    compile_ok(&format!(
        "icon \"i\" {{ category \"c\"; footprint 1 1 1; shape-limit 5; {poly} }}"
    ));
    let (_, msgs) = compile(&format!(
        "icon \"i\" {{ category \"c\"; footprint 1 1 1; shape-limit 4; {poly} }}"
    ));
    assert!(
        msgs.len() == 1 && msgs[0].contains("shape-limit of 4"),
        "{msgs:?}"
    );
}

#[test]
fn loops_that_emit_nothing_stop_at_the_repeat_limit() {
    // no shapes at all, so only the iteration count can stop it
    let empty =
        "part \"p\" { repeat 1001 \"a\" { repeat 1000 \"b\" { tile 0 0 0 1 1 when=\"b < 0\" } } }";
    let (icons, msgs) = compile(&format!(
        "icon \"spin\" {{ category \"c\"; footprint 1 1 1; {empty} }}"
    ));
    assert!(
        msgs.len() == 1
            && msgs[0].contains(
                "icon 'spin' goes here past its repeat-limit of 1000000 (repeat iterations, the default)"
            ),
        "{msgs:?}"
    );
    assert!(icons[0].shapes.is_empty());
    compile_ok(&format!(
        "icon \"spin\" {{ category \"c\"; footprint 1 1 1; repeat-limit 2000000; {empty} }}"
    ));
}

#[test]
fn fragment_nesting_stops_at_the_insert_depth() {
    let chain = r#"
fragment "a" { part "p" { insert "b" } }
fragment "b" { part "p" { insert "c" } }
fragment "c" { part "p" { tile 0 0 0 1 1 } }
"#;
    compile_ok(&format!(
        "{chain}icon \"i\" {{ category \"c\"; footprint 1 1 1; part \"p\" {{ insert \"a\" }} }}"
    ));
    let (_, msgs) = compile(&format!(
        "{chain}icon \"i\" {{ category \"c\"; footprint 1 1 1; insert-depth 2; part \"p\" {{ insert \"a\" }} }}"
    ));
    assert!(
        msgs.len() == 1
            && msgs[0].contains(
                "icon 'i' nests fragment insertions here past its insert-depth of 2 (levels, set by 'i')"
            ),
        "{msgs:?}"
    );
}

#[test]
fn axis_products_stop_at_the_icon_limit() {
    // ten axes of nine variants plus the bare option: 10^10 icons, which
    // must be refused by arithmetic, never built
    let mut src = String::new();
    for a in 0..10 {
        src.push_str(&format!("axis \"a{a}\" {{"));
        for v in 0..9 {
            src.push_str(&format!(
                " variant \"v{a}{v}\" {{ part \"p{a}{v}\" {{ tile 0 0 0 1 1 }} }};"
            ));
        }
        src.push_str(" }\n");
    }
    let axes: Vec<String> = (0..10).map(|a| format!("\"a{a}\"")).collect();
    src.push_str(&format!(
        "icon \"many\" {{ category \"c\"; footprint 1 1 1; axes {} }}\n",
        axes.join(" ")
    ));
    let (icons, msgs) = compile(&src);
    assert!(
        msgs.len() == 1
            && msgs[0].contains(
                "'many' takes the lexicon past its icon-limit of 100000 (icons, the default); \
                 if that is intended, raise it in the lexicon block"
            ),
        "{msgs:?}"
    );
    assert!(icons.is_empty());

    // and from the lexicon block: four icons against a limit of three, then four
    let small = "axis \"s\" { variant \"x\" { part \"x\" { tile 0 0 0 1 1 } } }\n\
                 icon \"one\" { category \"c\"; footprint 1 1 1; axes \"s\" }\n\
                 icon \"two\" { category \"c\"; footprint 1 1 1; axes \"s\" }\n";
    let (_, msgs) = compile(&format!("lexicon {{ icon-limit 3 }}\n{small}"));
    assert!(
        msgs.len() == 1 && msgs[0].contains("'two' takes the lexicon past its icon-limit of 3"),
        "{msgs:?}"
    );
    compile_ok(&format!("lexicon {{ icon-limit 4 }}\n{small}"));
}

#[test]
fn fragments_are_hygienic_and_check_their_args() {
    let src = r#"
fragment "stack" {
  param n 2
  param cx 0
  part "s" { repeat "n" "k" { box "cx" 0 "k * 0.25" 1 1 0.2 } }
}
icon "pool" {
  category "c"
  footprint 2 2 1
  param outer 9
  part "stacks" { insert "stack" cx="outer / 9" n=3 }
}
"#;
    let icons = compile_ok(src);
    assert_eq!(boxes(&icons[0]).len(), 3, "arg n=3 overrode the default 2");
    assert_eq!(boxes(&icons[0])[0].0, 1.0, "caller-scope arg evaluated");

    let (_, msgs) = compile(
        r#"
fragment "f" { param a 1 part "p" { box "a" 0 0 1 1 1 } }
icon "x" { category "c"; footprint 1 1 1; part "p" { insert "f" b=2 } }
"#,
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("no parameter 'b'") && m.contains("did you mean 'a'?")),
        "{msgs:?}"
    );

    // hygiene: the fragment body must not see the caller's params
    let (_, msgs) = compile(
        r#"
fragment "f" { part "p" { box "outer" 0 0 1 1 1 } }
icon "x" { category "c"; footprint 1 1 1; param outer 5; part "p" { insert "f" } }
"#,
    );
    assert!(
        msgs.iter().any(|m| m.contains("unknown name 'outer'")),
        "{msgs:?}"
    );
}

#[test]
fn fragment_cycles_are_reported() {
    let src = r#"
fragment "a" { part "p" { insert "b" } }
fragment "b" { part "p" { insert "a" } }
icon "x" { category "c"; footprint 1 1 1; part "p" { insert "a" } }
"#;
    let (_, msgs) = compile(src);
    assert!(
        msgs.iter()
            .any(|m| m.contains("fragment insertion cycle") && m.contains("a -> b -> a")),
        "{msgs:?}"
    );
}

#[test]
fn duplicate_names_are_an_error() {
    let (_, msgs) = compile(
        "icon \"x\" { category \"c\"; footprint 1 1 1 }\nicon \"x\" { category \"c\"; footprint 1 1 1 }",
    );
    assert!(
        msgs.iter().any(|m| m.contains("duplicate definition 'x'")),
        "{msgs:?}"
    );
}

#[test]
fn an_expanded_name_colliding_with_a_definition_is_an_error() {
    // the IR held srv, srv-down, srv-down, and compiled clean
    let src = r#"
axis "state" { variant "down" { part "b" { tile 0 0 1 1 1 } } }
icon "srv" { category "c"; footprint 1 1 1; axes "state"; part "p" { tile 0 0 0 1 1 } }
icon "srv-down" { category "c"; footprint 1 1 1; part "p" { box 0 0 0 1 1 1 } }
"#;
    let (_, msgs) = compile(src);
    assert_eq!(
        msgs,
        vec![
            "'srv-down' is emitted twice: by icon 'srv' with variant 'down' of axis 'state', \
             and by icon 'srv-down'"
        ],
    );
}

#[test]
fn expanded_names_colliding_with_each_other_are_an_error() {
    // one icon against itself: a's "x-y" and a's "x" plus b's "y" both
    // spell i-x-y
    let src = r#"
axis "a" {
  variant "x" { part "ax" { tile 0 0 1 1 1 } }
  variant "x-y" { part "axy" { tile 0 0 2 1 1 } }
}
axis "b" { variant "y" { part "by" { tile 0 0 3 1 1 } } }
icon "i" { category "c"; footprint 1 1 1; axes "a" "b"; part "p" { tile 0 0 0 1 1 } }
"#;
    let (_, msgs) = compile(src);
    assert_eq!(
        msgs,
        vec![
            "'i-x-y' is emitted twice: by icon 'i' with variant 'x' of axis 'a' and variant 'y' \
             of axis 'b', and by icon 'i' with variant 'x-y' of axis 'a'"
        ],
    );
}

#[test]
fn grants_union_down_the_chain() {
    // a base and its child granting the same token emitted it twice; it
    // is one grant, in the slot it was first given, with the child's
    // reason, the way a child's value wins everywhere else
    let src = r#"
base "b" { category "c"; footprint 1 1 1; grant "accent" reason="the base's" }
icon "i" {
  extends "b"
  grant "accent-dim" reason="only the child's"
  grant "accent" reason="the child's"
}
"#;
    let icons = compile_ok(src);
    assert_eq!(
        icons[0].grants,
        vec![
            ("accent".to_string(), "the child's".to_string()),
            ("accent-dim".to_string(), "only the child's".to_string()),
        ]
    );
}

#[test]
fn a_duplicate_definition_is_reported_once() {
    // the resolver names it; its two emissions are not a second error
    let (_, msgs) = compile(
        "icon \"x\" { category \"c\"; footprint 1 1 1 }\nicon \"x\" { category \"c\"; footprint 1 1 1 }",
    );
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    assert!(msgs[0].contains("duplicate definition 'x'"), "{msgs:?}");
}

#[test]
fn anchor_expressions_defer_and_capture_their_environment() {
    use isolex::expand::{EvNum, Shape2};
    let src = r#"
icon "x" {
  category "c"
  footprint 1 1 1
  param off 3
  part "p" {
    at 0.5 0.5 1 {
      repeat 2 "i" {
        line "x - off + i" "y" "x + off" "y" stroke="ink" width=1.4
      }
    }
  }
}
"#;
    let icons = compile_ok(src);
    let Shape::At { children, .. } = &icons[0].shapes[0] else {
        panic!("expected an At group");
    };
    assert_eq!(children.len(), 2, "repeat flattened inside the anchor");
    let Shape2::Line { args, .. } = &children[1] else {
        panic!("expected a line");
    };
    match &args[0] {
        EvNum::NeedsAnchor { env, .. } => {
            assert_eq!(env.get("off"), Some(&3.0));
            assert_eq!(env.get("i"), Some(&1.0), "loop var captured per iteration");
        }
        other => panic!("expected a deferred anchor expression, got {other:?}"),
    }
    match &args[1] {
        EvNum::NeedsAnchor { env, .. } => assert!(env.is_empty()),
        other => panic!("y alone should still defer, got {other:?}"),
    }
}
