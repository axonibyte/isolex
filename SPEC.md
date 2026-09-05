# The Isolex Language — Specification (v0, draft)

Isolex is a declarative language for defining icon systems: families of
glyphs described as data, layered by prototype inheritance, parameterized,
and compiled deterministically to a resolved shape intermediate
representation ("shape IR"). Source files use `.ilx` and are
[KDL 2.0](https://kdl.dev/spec) documents; this specification defines the
node vocabulary, the expression language, resolution semantics, and the
IR — everything a second implementation would need.

Sections marked *(normative text lands with the implementing change)* are
skeleton placeholders: this document grows in lockstep with the reference
compiler, and CI holds them together.

## 1. Lexicons and compilation units

A **lexicon** is a directory tree of `.ilx` files compiled as one unit:
one global namespace, duplicate names are errors, no imports in v0. Files
are processed in sorted path order; definitions in file order; every
downstream ordering is derived from these, which is what makes
compilation byte-deterministic.

## 2. Top-level nodes

`lexicon` (pack configuration) · `base` (abstract definition, never
emitted) · `icon` (concrete, emitted) · `fragment` (parameterized
geometry group) · `axis` (variant overlay). *(normative text lands with
the implementing change)*

## 3. Definition contents

`extends`, `category`, `footprint`/`canvas`, `describe`, `param`,
named `part` blocks, `badge` sugar, `grant` (colour-reservation
declarations), `axes` opt-in. *(normative text lands with the
implementing change)*

## 4. Geometry vocabulary

Isometric nodes: `box`, `tile`, `cylinder`, `facerect`, `poly3d`,
`shadow`. Screen-space anchor groups: `at x y z { … }` containing `line`,
`circle`, `ellipse`, `rect`, `arc`, `path` (with `{expr}` interpolation),
and `insert` for fragments. Projection constants and painter order are
part of the language's meaning and are specified here. *(normative text
lands with the implementing change)*

## 5. Values and expressions

Numbers are literals; a string in a numeric position is an expression.
The expression grammar (own parser, no host-language leakage):
arithmetic `+ - * / %`, unary minus, parentheses; names in scope
(params, loop variables, anchor `x`/`y`); `min`, `max`, `abs`, `round`,
`floor`, `ceil`; `if(cond, a, b)` where comparisons and boolean
operators are valid only inside `cond`. `repeat n as "i"` (count
evaluated up front, capped), `when="expr"` guards. Deliberately absent:
user-defined functions, recursion, assignment, strings-as-values, I/O,
randomness — evaluation is total and pure. *(normative text lands with
the implementing change)*

## 6. Inheritance and axes

Single `extends` chain. Params merge child-over-parent and references
are late-bound against the final set. Parts merge by name: new names
append; `replace` swaps in place preserving the inherited z-slot;
`remove` deletes; redeclaring an inherited name without `replace` is an
error. Axes expand deterministic cartesian products into
`name-<variant>` slugs. *(normative text lands with the implementing
change)*

## 7. Shape IR

JSON; schema and the normative coordinate formatting rule (fixed
two-decimal formatting; identical across implementations) land with the
projection stage. *(normative text lands with the implementing change)*

## 8. Diagnostics

Every diagnostic carries a source span (file, line, column), what was
expected and found, and a nearest-name suggestion where one exists.
Diagnostic text is part of the compiler's compatibility surface.

## 9. Conformance

A conforming compiler produces byte-identical IR for identical lexicons,
rejects everything §5 forbids, and passes the reference test suite.
