# isolex

A declarative language for icon systems — define glyph families with
parameters and prototype inheritance, compile deterministically to vector
output.

Isolex sources (`.ilx`, [KDL 2.0](https://kdl.dev) syntax) describe icons
as a tree: a `base "server"` carries the shared chassis; an
`icon "server-database" extends="server"` inherits it and modifies named
parts. Parameters are late-bound (a parent's port row sees the child's
port count), variant `axis` overlays expand into families, and the
compiler emits a resolved shape-IR (JSON) that any renderer can style —
same input, same bytes, every time.

```kdl
base "rack-unit" {
  param u 1
  footprint 2 1 "u * 0.28"
  part "chassis" { box 0 0 0 2 1 "u * 0.28" }
}

icon "switch-l3" {
  extends "switch-l2"
  part "routing-badge" {
    at 1 0.5 0.28 {
      path d="M{x-10},{y} h20 M{x+14},{y-3} l6,3 -6,3 M{x-14},{y-3} l-6,3 6,3" \
           stroke="ink" width=1.5
    }
  }
}
```

The language is small on purpose: arithmetic expressions, `repeat`,
`when` guards, fragments, transforms and quarter-turns, and text in
fonts that are themselves drawn in isolex — and nothing Turing-shaped. Compilation is
total, pure, and byte-deterministic. See [SPEC.md](SPEC.md).

```sh
isolex check src/          # parse + resolve + lint the lexicon
isolex compile src/ --ir out.json
isolex docs src/           # generate the lexicon's reference tables
isolex render src/ --svg out/ [--palette palette.json]   # one SVG per icon
isolex compile src/ --ir - --icon port-label --param ports=24   # one icon, params set
isolex font import Font.ttf --out fonts/font.ilx                 # a standard font to isolex
isolex font export src/ --font plain --otf plain.ttf             # and back
```

## Embedding

The compiler is also a library (`isolex::compile`, from source text to
IR and diagnostics, with no file system involved) and, through it, a
WebAssembly module that imports nothing:

```sh
cargo build --release --locked -p isolex-wasm --target wasm32-unknown-unknown
```

The module takes `{"files": [{"name", "text"}, …]}` as JSON and returns
the IR with the errors and warnings as data; with `"icon"` and
`"params"` too, it compiles that one icon with those params set, for a
host drawing a value of one thing (`isolex::compile_one`), and with
`"requests"`, many such icons from one parse (`isolex::compile_many`); [wasm/src/lib.rs](wasm/src/lib.rs)
documents its four exports, and `isolex::json` the shapes. Each release
publishes it as `isolex-<version>.wasm`.

A host with a file system reads a lexicon's folder with
`isolex::library::load`, as the command line does: every `.ilx` file
under it, in SPEC §1's order, which `isolex::library::order` gives for
paths it has already. A compilation also says where each shape of
the IR came from (`Compilation::provenance`): the node that drew it, the
definition and part it is written in, the inserts, repeats, badges and
texts it came through, and the map from that node's own coordinates to
the viewBox, so an editor can find the source of a shape on screen. And
`isolex::graph` gives a library's definitions, with the icons each
emits, and every reference from one to another (`extends`, `insert`,
`badge`, `axes` and a text's font), each with where the name is written.

## Documentation

- [docs/guide.md](docs/guide.md) — a task-oriented walk through authoring
  a lexicon: your first icon, parts, inheritance & late binding,
  fragments, screen-space paths, `repeat`/`when`, axes, transforms and
  turns, text, expressions.
- [docs/fonts.md](docs/fonts.md) — drawing a font, its styles, setting
  text in it, and converting fonts to and from TrueType, OpenType, WOFF
  and UFO.
- [docs/primitives.md](docs/primitives.md) — every geometry node, with an
  example of each.
- [docs/cookbook.md](docs/cookbook.md) — worked patterns from a real
  ~900-icon lexicon.
- [SPEC.md](SPEC.md) — the normative specification.

Every example in the guides is compile-checked against the current
compiler.

Copyright Axonibyte Innovations, LLC. Licensed under the Apache License,
Version 2.0 (see [LICENSE](LICENSE) and [NOTICE](NOTICE)).
