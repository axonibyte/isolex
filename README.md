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
  footprint 2 1 "=u * 0.28"
  part "chassis" { box 0 0 0 2 1 "=u * 0.28" }
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
`when` guards, fragments — and nothing Turing-shaped. Compilation is
total, pure, and byte-deterministic. See [SPEC.md](SPEC.md).

```sh
isolex check src/          # parse + resolve + lint the lexicon
isolex compile src/ --ir out.json
isolex docs src/           # generate the lexicon's reference tables
```

Licensed under the Apache License, Version 2.0.
