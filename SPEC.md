# The Isolex Language — Specification (v0)

Isolex is a declarative language for defining icon systems: families of
glyphs described as data, layered by prototype inheritance,
parameterized, and compiled deterministically to a resolved shape
intermediate representation ("shape IR"). Source files use the `.ilx`
extension and are [KDL 2.0](https://kdl.dev/spec) documents. This
specification defines the node vocabulary, the expression language,
resolution semantics, projection, and the IR — everything a second
implementation needs to produce byte-identical output.

## 1. Lexicons and compilation units

A **lexicon** is a directory tree of `.ilx` files compiled as one unit:
one global namespace across every definition kind; duplicate names are
errors; there are no imports in v0. Files are processed in sorted path
order and definitions in file order; every downstream ordering (emission,
axis expansion, diagnostics) derives from these, which is what makes
compilation byte-deterministic.

## 2. Top-level nodes

- `lexicon { … }` — at most one per lexicon. Settings: `tokens "a" "b" …`
  (the theme token vocabulary) and `stroke-widths 1.2 …` (the permitted
  stroke widths). v0 records these for consumers; enforcement hooks are
  reserved.
- `base "name" { … }` — an abstract definition. Never emitted; may be
  incomplete.
- `icon "name" { … }` — a concrete definition. After resolution it must
  have `category` and either `footprint` or `canvas`; anything else is an
  error naming what is missing.
- `fragment "name" { … }` — a parameterized geometry group instantiated
  with `insert`. Fragments cannot extend or be extended.
- `axis "name" { … }` — a variant overlay set (§6).

`extends` may be written as a property (`icon "x" extends="y"`) or as a
child node (`extends "y"`); declaring both is an error.

## 3. Definition contents

- `category "compute"` — grouping key; child value wins.
- `footprint w d h` — the isometric tile volume; drives the viewBox.
- `canvas w h` — a flat screen-space frame (badges, chrome).
- `describe "…"` — documentation; chains **concatenate** ancestor→child.
- `param name default` — a named number; the default may be an
  expression. Params evaluate in declaration order, and earlier params
  are in scope for later defaults.
- `part "name" { …geometry… }` — all geometry lives in named parts; the
  name is the handle children use to modify it.
- `replace "name" { … }`, `remove "name"` — child-side part operations
  (§6).
- `axes "a" "b"` — opt into variant axes; unions down the chain.
- `grant "token" reason="…"` — declares this definition's right to use a
  reserved colour token; carried into the IR for governance tooling.
- `badge "icon-name" corner="ne" scale=0.5` — places another icon's
  compiled shapes at a viewBox corner (`ne` default; `nw`/`se`/`sw`;
  scale default 0.5). A badge target must not itself contain a badge.

## 4. Geometry vocabulary

Coordinates are in tile units for 3-D nodes and output units for 2-D
nodes. Painter order is document order after resolution.

3-D nodes (outside `at`):

| Node | Args | Notes |
|---|---|---|
| `box x y z w d h` | `fills="body"\|"virt"\|"none"`, `opacity` | emits left, right, top faces, in that order, with fills `body-left`/`body-right`/`body-top` (or `virt`×3 / `none`×3), stroke `outline` at width 1.2 |
| `tile x y z w d` | style | flat diamond; default fill `body-top`, stroke `outline` 1.2 |
| `cylinder cx cy z r h` | `top`, `side`, `opacity` | body path then top ellipse; rx = √2·r·COS·S, ry = √2·r·SIN·S; defaults `body-right`/`body-top` |
| `facerect "front"\|"right" a b z extent h` | `fill` (default `ink`), `opacity` | wall-mounted rectangle, stroke `none` |
| `poly3d { pt x y z … }` | style | projected polygon; `pt` and `repeat` children only |
| `shadow cx cy rx ry` | | ground ellipse, fill token `shadow`, opacity 0.35, stroke none |
| `seg ax ay az bx by bz` | style | two-point tile-space segment; both endpoints project via P, emitted as one path |
| `at x y z { …2-D… }` | | anchor group: binds `x`,`y` to the projected screen point for children |

2-D nodes (inside `at`; coordinates are screen-absolute — the anchor
exists to give you `x`/`y`):

| Node | Args | Defaults |
|---|---|---|
| `line x1 y1 x2 y2` | | stroke `ink` 1.4 |
| `circle cx cy r` | | stroke `outline` 1.2, fill `none` |
| `ellipse cx cy rx ry` | | stroke `outline` 1.2, fill `none` |
| `rect x y w h` (`rx=`) | | stroke `none` 1.4, fill `none` |
| `arc cx cy r a0 a1` | | degrees, counter-clockwise from +x, y-flipped; stroke `ink` 1.4; large-arc flag set when \|a1−a0\| > 180 |
| `path d="M{x-10},{y} h20"` | style | `{…}` interpolates numeric expressions with `x`/`y` in scope |
| `poly { pt x y … }` | style | fill `ink`, stroke `none` |

Context-polymorphic: `repeat count "var" { … }` and
`insert "fragment" arg=… { }` work in both dimensions and inside poly
bodies; `pt` only inside `poly`/`poly3d`. Fragment part bodies parse
permissively in either dimension; the insertion site decides validity at
expansion.

Style properties everywhere applicable: `fill`, `stroke` (theme tokens
or `none`), `width`, `dash` ("3 2.5"), `opacity`, and the `when` guard.

## 5. Values and expressions

A number value is a literal; a **string value in a numeric position is
an expression**. Grammar (own parser; no host-language leakage):

```
or    := and ("or" and)*
and   := not ("and" not)*
not   := "not" not | cmp
cmp   := add (("<"|"<="|">"|">="|"=="|"!=") add)?
add   := mul (("+"|"-") mul)*
mul   := unary (("*"|"/"|"%") unary)*
unary := "-" unary | atom
atom  := NUMBER | NAME | call | "(" or ")"
call  := ("min"|"max") "(" or "," or ")"
       | ("abs"|"round"|"floor"|"ceil"|"cos"|"sin"|"cosr"|"sinr") "(" or ")"
       | "if" "(" or "," or "," or ")"
```

Two types exist. Comparisons and boolean operators produce `Bool`,
legal only as an `if` condition or as the root of a `when` guard; every
other position requires a `Num` root. `cos`/`sin` take degrees;
`cosr`/`sinr` take radians (the radian-native form the star, gear, and
rosette helpers are written in). `%` is
IEEE-754 remainder with the dividend's sign (Rust/C `fmod`). Division
and remainder by zero are compile-time errors when reached. `repeat`
counts must evaluate to non-negative integers and are capped at 10 000.

Every number is finite. An expression whose value is not (an overflow
to infinity, or NaN) is a compile-time error when reached; only the
value counts, so an overflow along the way that comes back to a finite
result is not one. The KDL literals `#inf`, `#-inf` and `#nan` are
errors wherever a number is expected.

Deliberately absent: user-defined functions, recursion, assignment,
strings-as-values, I/O, randomness. Evaluation is total and pure;
compilation always terminates.

Scoping: params and enclosing `repeat` variables are in scope;
fragments are **hygienic** — a fragment body sees only its own params
(caller args evaluate in caller scope, then bind). Inside `at`, the
names `x` and `y` are bound to the anchor's screen coordinates.

## 6. Inheritance and axes

Single `extends` chain, walked root-first, merged child-over-parent:

1. **Params** override in place, keeping the original declaration slot;
   references are late-bound against the final merged set — a parent's
   geometry sees the child's values.
2. **Parts** merge by name. New names append (render after inherited
   parts). `replace` swaps the body in the inherited z-slot. `remove`
   deletes. Redeclaring an inherited name with `part` is an error;
   replacing or removing an unknown name is an error with a
   nearest-name suggestion.
3. `category`/`footprint`/`canvas`: child wins. `describe` concatenates.
   `axes` and `grant`s union (first-seen order).

Cycles are errors reporting the full chain. Only `base` and `icon` can
be extended.

An `axis` holds `variant "name" { …definition body… }` overlays and an
optional `emit-base #false`. An icon (or ancestor) opts in with `axes`.
The compiler expands the cartesian product in axis-declaration order ×
variant-declaration order; each combination applies its variants as
child merges and emits `name-<v1>[-<v2>…]`. The bare name is the
all-absent combination; an axis with `emit-base #false` offers no
absent option, so every emitted combination carries one of its variants.

Emitted names are one namespace too: a name produced twice, whether by
a definition and an expansion (`srv-down` beside `srv` with variant
`down`) or by two expansions (variant `x-y` of one axis beside `x` of
it plus `y` of another), is an error naming both sources, reported at
the second in emission order.

## 7. Projection and the shape IR

Constants (normative, as literals — no libm variance may reach output):

```
S   = 40.0
COS = 0.8660254037844387     (cos 30°)
SIN = 0.5
P(x, y, z) = ((x − y)·COS·S, (x + y)·SIN·S − z·S)
PAD = 8.0                     (viewBox padding)
OUTLINE_W = 1.2
BADGE_SCALE = 0.5, BADGE_INSET = 2.0
```

The viewBox of a footprint (w, d, h) is the projected bounding box of
the volume's eight corners, padded by PAD on every side; a canvas icon's
viewBox is `[0, 0, w, h]`.

Projection keeps every number finite too: an icon whose geometry
overflows once projected to output units (its viewBox, a point, a
radius, a number in path data) is a compile-time error naming the icon.
No value in the IR is ever infinite or NaN.

The IR is JSON: `{ "ir_version": 1, "icons": [ … ] }`, each icon
carrying `name`, `category`, `footprint`/`canvas`, `viewbox`
`[minx, miny, w, h]`, `describe`, `grants`, and `shapes` — a flat,
ordered list of primitives tagged by `kind`:
`polygon` (points), `path` (d), `ellipse`, `circle`, `rect`, and
`group` (tx, ty, scale, shapes — badge composition; renderers wrap the
child shapes in a transform and never rewrite path data). Paints are
unresolved theme tokens; positional values are raw f64 (renderers apply
the formatting rule below); numbers **inside** path `d` strings are
already formatted.

**The formatting rule**: every number a conforming toolchain prints into
output text (path data at compile time; coordinates at render time) is
formatted with exactly two decimal places, round-half-to-even.

Determinism: identical lexicon bytes must produce identical IR bytes.
Runtime `cos`/`sin`/`cosr`/`sinr` (in expressions and `arc` endpoints)
are quantized to 1e-9 before use, which erases cross-platform libm last-ulp
divergence; every other arithmetic operation is exact IEEE-754. Identical
lexicon bytes therefore produce identical IR bytes on any platform.

## 8. Diagnostics

Every diagnostic carries `file:line:col` (1-based), states what was
expected and found, and offers a nearest-name suggestion (edit distance
≤ 2) where one exists. Diagnostic text is part of the compiler's
compatibility surface and is golden-tested.

## 9. CLI

`isolex check <dir>` parses, resolves, expands and projects, reporting
diagnostics and counts. `isolex compile <dir> --ir <path|->` writes the
IR JSON. `isolex docs <dir>` emits per-category markdown reference
tables from `describe` metadata. Exit status: 0 clean, 1 diagnostics or
I/O failure, 2 usage.

## 10. Conformance

A conforming compiler produces byte-identical IR for identical
lexicons, rejects everything §5 forbids with located diagnostics, and
passes the reference test suite (parser goldens, per-rule semantics
tests, e2e IR golden).
