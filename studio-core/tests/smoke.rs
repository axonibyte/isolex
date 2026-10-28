//! The core builds against the compiler and compiles through it.

#[test]
fn it_names_itself_and_the_compiler_it_is_built_with() {
    let about = isolex_studio_core::about();
    assert_eq!(about.studio, env!("CARGO_PKG_VERSION"));
    assert_eq!(about.isolex, isolex::VERSION);
    // as the shell sends it to the interface
    let json = serde_json::to_value(&about).unwrap();
    assert_eq!(json["isolex"], isolex::VERSION);
    assert_eq!(json["studio"], about.studio);
}

#[test]
fn it_compiles_a_library_through_the_compiler() {
    let icon = "icon \"one\" { category \"c\"; footprint 1 1 1; part \"p\" { box 0 0 0 1 1 1 } }\n";
    let c = isolex::compile(&[("one.ilx".to_string(), icon.to_string())]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert_eq!(c.doc.icons[0].name, "one");
}
