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
fn repeat_when_and_the_cap() {
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

    let (_, msgs) = compile(
        "icon \"y\" { category \"c\"; footprint 1 1 1; part \"p\" { repeat 10001 \"i\" { box 0 0 0 1 1 1 } } }",
    );
    assert!(
        msgs.iter().any(|m| m.contains("exceeds the cap")),
        "{msgs:?}"
    );

    let (_, msgs) = compile(
        "icon \"z\" { category \"c\"; footprint 1 1 1; part \"p\" { repeat 2.5 \"i\" { box 0 0 0 1 1 1 } } }",
    );
    assert!(
        msgs.iter().any(|m| m.contains("non-negative integer")),
        "{msgs:?}"
    );
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
