# The Isolex guide

A task-oriented walk through writing an icon lexicon. For the exact rules
see [SPEC.md](../SPEC.md); for every geometry node see
[primitives.md](primitives.md); for worked patterns see
[cookbook.md](cookbook.md).

## What a lexicon is

A **lexicon** is a directory of `.ilx` files. Isolex reads the whole
directory as one namespace (no imports in v0), resolves inheritance,
expands variants, projects the geometry, and emits a **shape-IR** (JSON):
resolved polygons, paths, ellipses, and groups with theme-token fills and
fixed-precision coordinates. A renderer turns that IR into themed SVG (or
anything else). Same input bytes → same IR bytes, on any platform.

```
your-lexicon/
  bases.ilx
  fragments.ilx
  compute.ilx
  ...
```

Nothing about a file name matters to the compiler — split definitions
across files however reads best.

## Getting the compiler

Build from source (Rust ≥ 1.97):

```sh
git clone https://github.com/axonibyte/isolex && cd isolex
cargo build --release        # binary at target/release/isolex
```

or download a release binary from the project's Downloads and verify its
checksum.

## Your first icon

An `icon` needs a `category`, a size (`footprint` for an isometric tile
volume, or `canvas` for a flat frame), and geometry in named `part`s:

```kdl
icon "server-1u" {
  category "compute"
  footprint 2 1 0.3
  describe "A one-rack-unit server."
  part "chassis" {
    box 0 0 0 2 1 0.3
  }
}
```

`box x y z w d h` places a rectangular volume at `(x,y,z)` sized
`w×d×h` in tile units. Save it as `demo/server.ilx` and compile:

```sh
isolex check demo/               # parse + resolve + lint
isolex compile demo/ --ir out.json   # write the shape-IR
```

`check` reports problems with `file:line:col`, the expected/found, and a
did-you-mean when a name is close. `compile --ir -` writes the JSON to
stdout.

## Parts are the handles

All geometry lives inside named `part`s. The name is not decoration: it
is how a child definition reaches in to modify a parent (below), and the
document order of parts is the **painter order** — later parts draw on
top.

```kdl
icon "server-1u" {
  category "compute"
  footprint 2 1 0.3
  part "chassis" { box 0 0 0 2 1 0.3 }
  part "bezel"   { facerect "front" 0.1 1 0.05 0.3 0.2 }   // drawn over the chassis
}
```

## Inheritance and late binding

A `base` is an abstract definition — never emitted, and allowed to be
incomplete. Concrete icons `extends` it and inherit its parts and params.
The powerful part is **late binding**: a parameter the child overrides is
visible to the *parent's* geometry.

```kdl
base "rack-unit" {
  category "compute"
  param u 1                         // rack units
  footprint 2 1 "u * 0.28"          // a string in a number slot is an expression
  part "chassis" { box 0 0 0 2 1 "u * 0.28" }
}

icon "server-1u" { extends "rack-unit" }          // u = 1
icon "server-2u" { extends "rack-unit" param u 2 } // parent's box now 2u tall
```

`server-2u` writes no geometry of its own; it just sets `u = 2`, and the
inherited `chassis` box grows because its height expression `u * 0.28`
sees the child's value.

Child-side part operations let a child modify what it inherits:

- add a new `part "name"` — appends in z-order;
- `replace "name" { … }` — swaps a part's body, keeping its z-slot;
- `remove "name"` — deletes an inherited part.

Redeclaring an inherited part name *without* `replace` is an error
(silent shadowing is how a pack rots). `extends` is a single chain; a
cycle is reported with the whole chain.

An **alias** is just an empty `extends` — same glyph, second name:

```kdl
icon "svc-idp" { extends "svc-ldap" describe "Alias of ldap." }
```

## Fragments: reusable geometry

A `fragment` is a parameterized group of geometry you stamp in with
`insert`. Fragments cannot extend or be extended — they are pure building
blocks (a disk stack, a wrench, a figure).

```kdl
fragment "disk-stack" {
  param cx 0.5
  param cy 0.5
  param n 3
  part "stack" {
    repeat "n" "k" {
      cylinder "cx" "cy" "k * 0.25" 0.26 0.2
    }
  }
}

icon "stor-pool" {
  category "stor"
  footprint 2 2 1
  part "platform" { box 0 0 0 2 2 0.12 }
  part "disks"    { insert "disk-stack" cx=0.55 cy=0.55 n=4 }
}
```

`insert "name" arg=value …` binds the fragment's params. Insert the same
fragment several times with different args to build a family.

## Screen-space work: `at` and paths

Volumes live on the isometric grid. Fine detail — a glyph, a badge, a
label — is drawn in 2-D screen space inside an `at x y z { … }` block.
`at` projects the tile point `(x,y,z)` to a screen point and binds `x`
and `y` for its children, so you draw *relative to that anchor*:

```kdl
icon "svc-database" {
  category "svc"
  footprint 1 1 1
  part "slab" { box 0.08 0.08 0 0.84 0.84 0.09 }
  part "glyph" {
    at 0.5 0.5 0.55 {                       // hover a glyph above the slab centre
      path d="M{x-8},{y-5.5} V{y+5.5} A8 3.6 0 0 0 {x+8},{y+5.5} V{y-5.5}" stroke="ink" width=1.4
      ellipse "x" "y-5.5" 8 3.6 fill="none" stroke="ink" width=1.4
    }
  }
}
```

Inside a `path` string, `{ … }` interpolates a numeric expression with
`x`/`y` in scope. The other 2-D nodes (`circle`, `ellipse`, `rect`,
`arc`, `line`, `poly`) take their coordinates as expressions too — a bare
number is a literal, a quoted string is an expression: `circle "x" "y-5.5"
8`.

## `repeat` and `when`

`repeat count "var" { … }` emits its body `count` times with `var` bound
to `0 … count-1`. It works on 3-D nodes, 2-D nodes, and `pt` lists.

```kdl
part "slots" {
  repeat 4 "i" {
    facerect "front" "0.14 + i * 0.19" 0.95 0.12 0.13 0.62
  }
}
```

`when="expr"` guards any node — it is emitted only when the boolean
expression holds:

```kdl
insert "disk-stack" n=2 when="ports > 4"
```

The count of a `repeat` is evaluated up front; guards and counts are
pure, so compilation always terminates.

Nested repeats multiply, so a two-line typo can ask for a hundred
million shapes. Generous limits catch that and point at the line: an icon
stops at 100 000 shapes and points (`shape-limit`) and a million repeat
iterations (`repeat-limit`), fragments nest at most 32 deep
(`insert-depth`), and a lexicon emits at most 100 000 icons
(`icon-limit`). If you mean it, say so, for one definition (inherited by
whatever extends it) or for the whole lexicon, and `#null` switches a
limit off:

```kdl
lexicon {
  shape-limit 250000
}

base "dense-grid" {
  repeat-limit 5000000
  shape-limit #null
}
```

## Axes: variant families

An `axis` is an overlay set. Each `variant` carries a definition body
(usually a `badge` or a `replace`), and an icon opts in with `axes`. The
compiler expands the cartesian product and emits `name-<variant>` slugs;
the bare name is the all-absent combination.

```kdl
axis "state" {
  emit-base #true
  variant "down"     { part "state-badge" { badge "state-down" corner="ne" } }
  variant "degraded" { part "state-badge" { badge "state-degraded" corner="ne" } }
}

icon "compute-server-1u" {
  extends "rack-unit"
  axes "state"        // emits -1u, -1u-down, -1u-degraded
}
```

`badge "icon-name"` composites another icon's compiled shapes into a
viewBox corner — the state badges are ordinary flat `canvas` icons reused
as overlays.

## The expression language

Wherever a number is expected, a quoted string is evaluated. It is a
small, total, pure language — no user functions, no recursion, no I/O:

- arithmetic `+ - * / %`, unary minus, parentheses;
- names in scope: params, enclosing `repeat` variables, and `x`/`y`
  inside `at`;
- functions `min max abs round floor ceil`, `cos sin` (degrees),
  `cosr sinr` (radians), and `if(cond, a, b)`;
- comparisons `< <= > >= == !=` and `and or not` — but only inside an
  `if` condition or as the root of a `when` guard.

`cos`/`sin`/`cosr`/`sinr` are quantized to 1e-9 so the IR is byte-identical
across platforms. Division or `%` by zero, a result too large to be a
number (or NaN), a negative `repeat` count, and a boolean where a number
is required are all compile-time errors.

## Colours, stroke widths, and governance

Fills and strokes are **theme tokens** (`ink`, `ink-muted`, `outline`,
`body-top`, `accent`, …), never raw hex — the renderer maps tokens to a
palette. Declare your vocabulary once in a `lexicon` block:

```kdl
lexicon {
  tokens "ink" "ink-muted" "outline" "body-top" "body-left" "body-right" "accent" "accent-dim"
  stroke-widths 1.2 1.4 1.5 2.2
}
```

If a token is reserved (e.g. a brand red), a definition that uses it
declares the right with `grant "accent" reason="…"`; the grant travels
into the IR so governance tooling can assert usage matches intent.

## The workflow

```sh
isolex check src/                 # while authoring — fast, catches everything
isolex compile src/ --ir out.json # feed a renderer / build
isolex docs src/ > reference.md   # generate the lexicon's reference tables
```

`check` early and often; the diagnostics are precise and are treated as
API (they have golden tests). When two builds of the same source differ,
that is a bug in isolex, not in your lexicon — determinism is a promise.
