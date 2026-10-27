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
errors; there are no imports in v0. Files are processed directory by
directory, depth first: each directory's entries in byte order of their
names, a subdirectory's files at the place its name sorts. So `a/x.ilx`
comes before `a-b/x.ilx`, where a sort of whole path strings would put
it after (`-` is 0x2D, `/` is 0x2F). Definitions are processed in file
order; every downstream ordering (emission, axis expansion, diagnostics)
derives from these, which is what makes compilation byte-deterministic.

Each file is a KDL 2.0 document, read in full (§11): a syntax error is
reported where it is, with what is wrong, as `KDL syntax error: …`. Its
blocks may nest to any depth.

**Defaults.** Isolex has two kinds of default, and the test that tells
them apart is whether the default reaches the IR.

- A *compiler allowance* governs how compilation proceeds: the limits of
  §9. It never appears in the IR; a lexicon compiled under a different
  allowance compiles to the same IR or to an error, never to different
  IR. It may therefore be implicit, and may even differ between builds.
- A *program default* is something the compiled lexicon says: a paint
  token or stroke width filled in for a node that sets none (§4). It
  appears in the IR exactly as if the author had written it, so it is
  held to everything the lexicon declares. Where a `lexicon` block
  declares a vocabulary (§2), a program default outside it is an error,
  as the same value written by hand would be.

New defaults follow the same rule.

## 2. Top-level nodes

- `lexicon { … }` — at most one per lexicon. Settings: `tokens "a" "b" …`
  (the theme token vocabulary) and `stroke-widths 1.2 …` (the permitted
  stroke widths, positive numbers). Each one declared is a contract with
  the lexicon's consumers, checked against the IR after projection has
  filled in every default (§1, Defaults): every `fill` and `stroke` must
  be a declared token, and every width drawn with a stroke must equal a
  declared width as output formats them (§7: 1.401 is 1.40). `none` is
  no paint rather than a token and needs no declaring, and a width with
  no stroke is not checked. A violation is reported at the node, with a
  nearest-name suggestion for a token. A lexicon that declares neither
  is not checked. `reserved "a" …` marks tokens (each also in `tokens`,
  when that is declared) that an icon may paint with, written or
  defaulted, only if its definition chain grants them (§3). The limits
  of §9 can be set here for every icon.
- `base "name" { … }` — an abstract definition. Never emitted; may be
  incomplete.
- `icon "name" { … }` — a concrete definition. After resolution it must
  have `category` and either `footprint` or `canvas`; anything else is an
  error naming what is missing.
- `fragment "name" { … }` — a parameterized geometry group instantiated
  with `insert`. Fragments cannot extend or be extended.
- `axis "name" { … }` — a variant overlay set (§6).
- `font "name" { … }` — glyphs, metrics and kerning that `text` is set
  in (§4, Fonts and text). Never emitted; a font cannot extend or be
  extended, and its styles are variants of the axes it opts into.

`extends` may be written as a property (`icon "x" extends="y"`) or as a
child node (`extends "y"`); declaring both is an error.

## 3. Definition contents

- `category "compute"` — grouping key; child value wins.
- `footprint w d h` — the isometric tile volume; drives the viewBox.
- `canvas w h` — a flat screen-space frame (badges, chrome).
- `describe "…"` — documentation; chains **concatenate** ancestor→child.
- `param name default` — a named number; the default may be an
  expression. Params evaluate in declaration order, and earlier params
  are in scope for later defaults. `param name (text)"…"` is a text
  param, whose default is text with interpolations (§5, Text values).
- `part "name" { …geometry… }` — all geometry lives in named parts; the
  name is the handle children use to modify it.
- `replace "name" { … }`, `remove "name"` — child-side part operations
  (§6).
- `axes "a" "b"` — opt into variant axes; unions down the chain.
- `grant "token" reason="…"` — declares this definition's right to use a
  reserved color token; carried into the IR for governance tooling.
  Painting with a reserved token needs a grant somewhere in the icon's
  definition chain, or the variant that paints it; a badge's shapes are
  granted by its target. Granting a token the lexicon does not reserve
  is an error when it reserves any, with a nearest-name suggestion, and
  a warning when it reserves none (the grant then has no effect).
  Fragments cannot grant: their shapes belong to the icon they are
  inserted into, which grants for them.
- `badge "icon-name" corner="ne" scale=0.5` — places another icon's
  compiled shapes at a corner of the icon's frame, the viewBox of its
  footprint or canvas before any transform grows it (`ne` default;
  `nw`/`se`/`sw`; scale default 0.5). A badge target must not itself
  contain a badge; it may contain transforms, which are multiplied into
  the badge's map (§7).
- `view 90` — the corner the icon is drawn from: everything in its
  footprint turned in place, as `turn` turns it (§4), by a quarter-turn
  (0, 90, 180 or 270 degrees, or an expression of params that is one),
  so one source draws an object from more than one side. The footprint
  is turned with it: on a quarter-turn its width and depth trade places,
  and the IR's `footprint` is the footprint as drawn. It needs a
  footprint; on a canvas icon it is an error. Child value wins; an axis
  variant can set it. Not in fragments.
- `transform-bounds "grow"` — what this definition's transforms do when
  what they draw leaves its frame, unless a transform says (§4):
  `error`, `grow`, `scale` or `clip`. Child value wins. Not in
  fragments.
- `shape-limit n`, `repeat-limit n`, `insert-depth n`,
  `expression-depth n`, `transform-chain n`, `text-length n`,
  `text-limit n` — this definition's limits (§9); child value wins. Not
  in fragments, whose work counts toward the icon they are inserted
  into.

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
| `text x y "…"` | `font`, `size`, … | a line of text set in a font (below) |

Context-polymorphic: `repeat count "var" { … }` and
`insert "fragment" arg=… { }` work in both dimensions and inside poly
bodies; `transform … { … }` works in both dimensions; `pt` only inside
`poly`/`poly3d`. Fragment part bodies parse permissively in either
dimension; the insertion site decides validity at expansion.

### Screen-space transforms

`transform … { … }` draws its children through a 2-D affine map, after
projection: rotated, mirrored, skewed, scaled unevenly, or laid flat on
a face of a box. It stands in either dimension, around whatever is legal
where it stands. Its properties:

- `origin="cx cy"` — the point the steps turn, scale, skew and mirror
  about, in output units (inside `at`, `x` and `y` are in scope).
  Default `"0 0"`.
- The steps, as properties: `mirror="across"` (left for right) or
  `"down"` (top for bottom), `scale=s` or `scale="sx sy"`, `skew-y=deg`,
  `skew-x=deg`, `rotate=deg` and `translate="tx ty"`, applied to the
  shapes in that order, each but `translate` about the origin. Angles
  are degrees, clockwise on the screen, as SVG turns them (y runs down).
- Or a `steps { … }` block: the same steps as nodes with arguments
  (`rotate 90`, `skew-x -12`, `translate 2 0`, `scale 2` or `scale 2 1`,
  `skew-y 5`, `mirror "across"`, and `matrix a b c d e f`), applied in
  the order written. A transform takes steps as properties or as a
  block, never both.
- `matrix="a b c d e f"` — the whole map at once, as SVG's `matrix()`:
  a point (x, y) goes to (a·x + c·y + e, b·x + d·y + f). It takes no
  other step, and no origin, which it would ignore.
- `onto="top"|"front"|"right" of="box x y z w d h"` — after the steps,
  the map that lays flat content onto that face of that box (tile
  units), so artwork drawn flat, in output units, lies on the face, 40 to
  a tile along its edges. The flat origin is the face's top-left corner
  as seen; `front` is the +y wall and `right` the +x wall, as for
  `facerect`. The maps, with P of §7:
  `top` is `[COS, SIN, −COS, SIN, P(x, y, z+h)]` (u along +x, v along
  +y); `front` is `[COS, SIN, 0, 1, P(x, y+d, z+h)]` (u along +x, v
  down); `right` is `[COS, −SIN, 0, 1, P(x+w, y+d, z+h)]` (u along −y,
  v down).
- `bounds="error"|"grow"|"scale"|"clip"` and `strokes="scale"|"fixed"`,
  below; and `when`.

A property that holds several numbers holds them in one string, apart
by spaces, each a number or an expression; an expression with spaces of
its own is written in parentheses: `origin="(w / 2) 0"`.

A transform needs at least one step or `onto`. Everything about it is
worked out when the lexicon compiles: every property, expression and
param is evaluated, `cos` and `sin` are the portable ones of §7
(quantized, so a right angle is exact), a skew's tangent is their
quotient, and the IR carries six finished numbers (§7). A map that is
not finite (a skew of ±90°) is an error at the transform. Nothing is
rewritten: shapes keep their coordinates and path data, and the map
is carried around them.

**Strokes** scale with the map, as in SVG; `strokes="fixed"` keeps them
at the width written, whatever the map does. The nearest transform that
says decides. The stroke-width allowlist (§2) checks widths as written.

**Bounds.** What a transform draws must stay in its icon's frame, the
viewBox of the footprint or canvas: the geometry of its shapes (their
points, centers and radii, and path data with its curves and arcs, not
stroke widths), drawn through every map around them, at the precision
output is printed at (§7). It is checked at each outermost transform,
with everything in it, nested transforms, fragments and badges
included; a transform inside another is held by the outer one. One that
leaves the frame is an error at the transform, unless `bounds=` on it,
or else the icon's `transform-bounds` (§3), says what to do:

- `grow` widens the viewBox to take what it draws;
- `scale` folds into its map a uniform scale, never above 1, about the
  middle of what it draws, and the least shift that brings it inside the
  frame;
- `clip` cuts what it draws to the frame;
- `error`, the default, may be written to override an icon's default.

So the IR's viewBox contains everything drawn, whichever applies:
grown by `grow`, and unchanged by `scale` and `clip`, whose drawing
stays inside it. Hosts size and place icons by the viewBox alone.

### Tile-space turns

`turn 90 { … }` and `mirror-diagonal { … }` turn their children's
geometry in tile space, before projection, so a turned part shows the
faces it now turns toward. They stand among 3-D nodes, around 3-D
nodes.

- `turn a origin="x y"` turns about a vertical axis through `origin`
  (tile units, default `"0 0"`) by a quarter-turn: `a` is 0, 90, 180 or
  270 degrees, or any multiple of 90, or an expression that is one; a
  quarter-turn carries +x onto +y. Anything else is an error.
- `mirror-diagonal origin="x y"` reflects across the diagonal through
  `origin`, x − ox = y − oy, trading x for y.
- Turned, a `box` or `tile` is the same box turned (on a quarter-turn or
  mirror its width and depth trade places), a `cylinder` or `shadow`
  moves (a shadow's rx and ry trade places with the axes), and `poly3d`
  and `seg` points and `at` anchors turn. A `facerect` faces +y (front)
  or +x (right); turned, it moves to the wall it now faces, and is
  dropped if that wall faces −x or −y, since only the faces isolex draws
  are seen. The 2-D content of an `at`, badges and screen transforms
  stay as they are, but for the box `onto` lays content on: that box is
  turned, and a transform whose face is turned away draws nothing.
- Turns nest and compose, through fragments too, and every point is
  turned exactly, by sums and differences alone.
- Painter order stays document order: what stands in front after a turn
  is not moved later.
- Arbitrary angles are out of scope: a box turned 30° shows faces that
  depend on the angle, which painter order cannot sort.

### Fonts and text

A font is drawn in isolex, as everything else is: each glyph is shapes,
so text compiles to shapes, and the IR, the reference SVG and every
renderer agree on it to the byte, with no font looked up at draw time.

A `font "name" { … }` holds:

- `em n` — the size of its design grid, in its own units: a text of
  `size=s` draws one em `s` output units tall. More than 0.
- `ascent n`, `descent n` — how far it reaches above its baseline and
  below it, both as distances; `baseline=` aligns by them.
- `kind "stroke"|"outline"` — whether its glyphs are drawn along their
  middles, at a stroke width (often a param), or as filled outlines.
  It says how the font is meant to be drawn, for the tools that convert
  fonts; it changes nothing about how a text compiles.
- `slant deg` — a lean, as `skew-x` leans (§4): `-12` leans right.
  Default 0; an italic style sets it.
- `glyph "A" advance=6 { …2-D nodes… }` — the glyph for the text in its
  name: one character, or several, a ligature, which replaces that run
  of characters wherever it occurs. `advance` is how far it moves the
  pen. Its nodes are drawn in the em grid: its origin on the baseline at
  the pen, x to the right and y **up**, as fonts are drawn. A glyph with
  no nodes (a space) draws nothing and still advances.
- `notdef advance=6 { … }` — the glyph drawn for a character no glyph
  draws. Without one, such a character is an error.
- `kern "A" "V" -0.6` — space added between two glyphs drawn side by
  side, in the em grid, by their names.
- `param`, `axes`, `describe` and `glyph-limit` (§9), as elsewhere.

A font needs `em`, `ascent`, `descent` and `kind`; one without them is
an error naming what it lacks, and so is a kern of a glyph the font
does not draw. A glyph's nodes see only the font's params, as a fragment's see
only its own (§5); `x` and `y` mean nothing there, and a glyph cannot
set text. Its strokes and paints are held to the lexicon as any are.

**Styles.** A font opts into axes with `axes`, and their variants are
its styles: bold, italic, condensed, each a variant that restates a
param (a stroke-built font's weight), its metrics, its slant, or glyphs
and kerns (replacing the font's by name, or adding to them). A font
that draws only its regular weight thereby has a bold all the same. A
text asks for styles by name, at most one from each axis, and they are
laid over the font in its axes' order, as an icon's variants are (§6).
A style that sets what only an icon has (a part, a footprint, a
category) is an error where it styles a font, and a variant that sets
a font's metrics, glyphs or kerning is an error where it overlays an
icon.

**Text.** `text x y "…"` stands among 2-D nodes. Its arguments are the
point it is set from and what it says, text with `{…}` interpolations
(§5, Text values); its properties:

- `font="name"` and `size=s` — required; `size` is the em in output
  units, more than 0.
- `style="bold italic"` — its styles, apart by spaces.
- `anchor="start"|"middle"|"end"` — where `x` falls along it. Default
  `start`.
- `baseline="alphabetic"|"top"|"middle"|"bottom"` — where `y` falls on
  it: on the baseline, on the ascent line, halfway between the ascent
  and descent lines, or on the descent line. Default `alphabetic`.
- `max-width=w` — wider than this, it is scaled down, height too, until
  it fits.
- `fill="token"` — every paint of its glyphs but `none`, fill and
  stroke, written or defaulted, is drawn in this token instead.
- `bounds=` and `when`, as a transform's (§4).

It is laid out when the lexicon compiles, in this order:

1. Its text is interpolated, and held to `text-length` and `text-limit`
   (§9).
2. Each character must be one text can set left to right, a glyph after
   another; these are refused, each an error naming it: control
   characters (text is one line), combining marks (U+0300–036F,
   1AB0–1AFF, 1DC0–1DFF, 20D0–20FF, FE20–FE2F), right-to-left scripts
   and the controls that order them (U+0590–08FF, FB1D–FDFF, FE70–FEFF,
   10800–10FFF, 1E800–1EFFF, 200E–200F, 202A–202E, 2066–2069), scripts
   whose letters change shape with their neighbors (U+0900–0DFF,
   0E00–0EFF, 0F00–0FFF, 1000–109F, 1100–11FF, 1780–17FF, A960–A97F,
   D7B0–D7FF), and joiners and variation selectors (U+200C–200D,
   FE00–FE0F, E0100–E01EF).
3. From its first character on, each place takes the glyph whose name
   is the longest run of characters there, else `notdef` for one
   character.
4. The pen starts at 0, and each glyph is placed at it; the pen then
   moves by the glyph's advance, and by the kern of it and the next
   glyph, if there is one (`notdef` is never kerned). Its width `W` is
   where the pen ends.
5. `k = size / em`, or less, `max-width / W`, when `W·k` is more than
   `max-width`. `dx` is 0, `−W/2` or `−W` by its anchor, and `lift` is
   0, `ascent·k`, `(ascent − descent)/2·k` or `−descent·k` by its
   baseline.

It draws through the map `T(x, y) · T(0, lift) · skewX(slant) ·
S(k, −k) · T(dx, 0)`, each glyph through that and `T(pen, 0)`: the em
grid scaled to its size, turned y-down, leaned, aligned and set at its
point, and its glyphs along the baseline. Its strokes scale with it.
These are transforms like any (§4): they stand inside whatever
transforms are around the text, count three steps toward
`transform-chain` (its own two, and a glyph's place), and are held to
the icon's frame, the text as one transform with `bounds=`. Text with
nothing in it draws nothing.

Style properties everywhere applicable: `fill`, `stroke` (theme tokens
or `none`), `width`, `dash` ("3 2.5"), `opacity`, and the `when` guard.
Each node takes only the properties docs/primitives.md lists for it; any
other property on any node is a diagnostic at the property, as is a
string property given a value that is not a string. (`insert`'s
properties are its fragment's params, checked against the fragment.)

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
NUMBER := [0-9.]+ ([eE] [+-]? [0-9]+)?    (a decimal: 2, 0.5, .5, 1e300, 2.5e-3)
NAME   := [A-Za-z_] [A-Za-z0-9_]*
```

A `NUMBER`'s digits and dots must read as one decimal (`1.2.3` is a
malformed number), and its value must be finite (`1e400` is an error
where it is written).

Two types exist. Comparisons and boolean operators produce `Bool`,
legal only as an `if` condition or as the root of a `when` guard; every
other position requires a `Num` root. `cos`/`sin` take degrees;
`cosr`/`sinr` take radians (the radian-native form the star, gear, and
rosette helpers are written in). `round` takes halves away from zero
(`round(2.5)` is 3, `round(-2.5)` is −3), unlike the half-to-even rule
for formatting output (§7); `floor`, `ceil` and `abs` are the usual
IEEE-754 operations. `%` is
IEEE-754 remainder with the dividend's sign (Rust/C `fmod`). Division
and remainder by zero are compile-time errors when reached. `repeat`
counts must evaluate to non-negative integers; every iteration counts
toward the icon's `repeat-limit` (§9).

An expression may nest to any depth; how deeply one nests costs memory,
never a crash, so a conforming compiler must not parse, check or
evaluate expressions with recursion bounded by a call stack. Its depth
is 1 for a number or a name and one more than its deepest operand for an
operator or call (parentheses add nothing), and one nesting past the
icon's `expression-depth` (§9) is an error where it is used.

Every number is finite. An expression whose value is not (an overflow
to infinity, or NaN) is a compile-time error when reached; only the
value counts, so an overflow along the way that comes back to a finite
result is not one. The KDL literals `#inf`, `#-inf` and `#nan` are
errors wherever a number is expected.

Deliberately absent: user-defined functions, recursion, assignment,
strings in expressions, I/O, randomness. Evaluation is total and pure;
compilation always terminates.

### Text values

A string is an expression wherever a number goes, so text is marked:
`param label (text)"PoE {ports}"` declares a text param, and
`insert "plate" label=(text)"U{u}"` passes text to one. Any other type
annotation is an error. Text, in a text param or a `text` node, is
literal but for `{…}`, which holds an expression or a text param's
name; `{{` and `}}` are a brace of their own.

- A text param's name in braces is its text.
- An expression in braces is a number, printed by the formatting rule
  (§7) and then without the zeros that add nothing: 48 is `48`, 2.5 is
  `2.5`, 1/3 is `0.33`, and −0.001 is `0`.
- Text is laid out when the lexicon compiles, before any anchor is
  placed, so `x` and `y` are not known to it.

Text params are late-bound as numbers are (§6): they evaluate in
declaration order, each seeing the params before it, and a child's
value takes its slot. A text param is not a number: naming one in an
expression is an error, and so is overriding a number param with text
or the reverse, anywhere down a chain, or passing a fragment text for a
number param or a number for a text one.

Scoping: params and enclosing `repeat` variables are in scope;
fragments are **hygienic** — a fragment body sees only its own params
(caller args evaluate in caller scope, then bind). Inside `at`, the
names `x` and `y` are bound to the anchor's screen coordinates. They are
reserved for that everywhere: a param or `repeat` variable named `x` or
`y` is an error, since `at` would override it.

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
3. `category`/`footprint`/`canvas`/`view`/`transform-bounds`: child
   wins. `describe` concatenates. A param keeps its type, number or
   text (§5).
   `axes` union (first-seen order). `grant`s union by token, in
   first-seen order; a child granting a token its ancestor granted keeps
   that slot and replaces the reason.

Cycles are errors reporting the full chain. Only `base` and `icon` can
be extended.

An `axis` holds `variant "name" { …definition body… }` overlays and an
optional `emit-base #false`, and nothing else; a variant body may hold
what an icon's does or what a font's does, since fonts opt into axes
too (§4, Fonts and text): any other node in an axis
body is an error, and so is `extends` on an axis or in a variant (a
variant overlays the icon that opts into its axis; it has no parent of
its own). An icon (or ancestor) opts in with `axes`.
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
radius, a number in path data) is a compile-time error naming the icon,
reported at the node that overflowed (the anchor, for 2-D nodes inside
`at`; the `badge`, for a badge placed out of range; the transform, for
a map out of range). No value in the IR is ever infinite or NaN.

The IR is JSON: `{ "ir_version": 2, "icons": [ … ] }`, each icon
carrying `name`, `category`, `footprint`/`canvas`, `viewbox`
`[minx, miny, w, h]`, `describe`, `grants`, and `shapes` — a flat,
ordered list of primitives tagged by `kind`:
`polygon` (points), `path` (d), `ellipse`, `circle`, `rect`, and
`group`. Paints are unresolved theme tokens; positional values are raw
f64 (renderers apply the formatting rule below); numbers **inside** path
`d` strings are already formatted.

A **group** is shapes drawn through an affine map: a transform's (§4),
a badge's, a text's glyph's, or several multiplied out. Text adds no
kind of shape: its glyphs are the paths, polygons and other shapes they
are drawn with, each glyph in a group of its own. It carries `matrix`, six numbers
`[a, b, c, d, e, f]` as SVG's `matrix()`; optionally `clip`, a convex
polygon (its points) in the coordinates the group is drawn in, which
cuts what is drawn to it; optionally `strokes: "fixed"`, which keeps
stroke widths as written instead of scaled by the matrix; and its
`shapes`. Groups never nest: where maps meet (a transform inside
another, or in a fragment inserted in one, or a badge in a transform,
or a badge whose target has transforms), the compiler multiplies them
out, in f64, into one matrix around each run of shapes: those of one
transform, or one badge, or one group of a badge's target. Clips are
intersected the same way. A shape drawn with no map, clip or fixed
strokes is not in a group. A badge's map is `[s, 0, 0, s, tx, ty]`.

Renderers wrap a group's shapes in its matrix and never rewrite path
data; nor does the compiler, which carries a transform around its
shapes rather than into them. A group's clip is a clip path in the
coordinates the group is drawn in, and fixed strokes are SVG's
`vector-effect="non-scaling-stroke"`, or a stroke drawn in device space
in formats without it.

**The formatting rule**: every number a conforming toolchain prints into
output text (path data at compile time; coordinates at render time) is
formatted with exactly two decimal places, round-half-to-even on the
number's exact binary value: 0.125 is `0.12` and 0.375 `0.38`, while
0.145, which is 0.14499… in binary, is `0.14`. Zero is written `0.00`,
never `-0.00`, whether the value is −0.0 or rounds to zero from below
(−0.001). `isolex render` (§10) is
the reference for the render-time half: each icon an SVG document whose
viewBox, coordinates, radii, widths, opacities, matrices and clips
follow this rule, whose path data passes through unchanged, and whose
groups are a `<g transform="matrix(a b c d e f)">` around their shapes.
A group with a clip declares `<clipPath id="clip-<icon>-<n>">` (its
polygon, numbered in document order) just before it and is wrapped in
`<g clip-path="url(#clip-<icon>-<n>)">`; in a group with fixed strokes,
every stroked shape carries `vector-effect="non-scaling-stroke"`.

Determinism: identical lexicon bytes must produce identical IR bytes.
Platform maths libraries disagree in the last bit of `cos` and `sin`
(over 40 million results, glibc's differed from musl's in 2.7% and
Windows' UCRT's in 1.5%, and the two disagreed with each other), so a
conforming compiler does not use them: runtime `cos`/`sin`/`cosr`/`sinr`
(in expressions and `arc` endpoints) are computed by one portable
implementation, musl's, as the reference compiler takes it from the
`libm` crate built without hardware-specific paths. Every other
operation is basic IEEE-754 arithmetic, which is exact and portable.
Identical lexicon bytes therefore produce identical IR bytes on any
platform. The results are then quantized to 1e-9, which makes values
that are 0 or 1 in exact arithmetic (`cosr` of π/2) exactly so.

## 8. Diagnostics

Every diagnostic carries `file:line:col` (1-based), states what was
expected and found, and offers a nearest-name suggestion (edit distance
≤ 2) where one exists. Diagnostic text is part of the compiler's
compatibility surface and is golden-tested.

A diagnostic is an error unless this specification calls it a warning.
An error fails compilation. A warning reports something that has no
effect (such as a grant in a lexicon that reserves nothing), is written
as `file:line:col: warning: …`, and never changes the exit status or the
IR.

## 9. Limits

Nothing in the language is Turing-shaped, but a few constructs multiply:
nested `repeat`s, fragments inserted inside repeats or other fragments,
and axes, whose product grows with each one an icon opts into. A small
source can therefore ask for an output too large to build, and
transforms chain through one another. These limits keep a mistake from
exhausting the machine. Each has a generous default,
and each can be raised, lowered, or switched off by an author who means
it:

| Directive | Bounds | Default | Set in |
|---|---|---|---|
| `shape-limit` | shapes and polygon points one icon emits | 100 000 | lexicon block, definition |
| `repeat-limit` | repeat iterations in one icon, all loops together | 1 000 000 | lexicon block, definition |
| `insert-depth` | how deeply fragment insertions nest | 32 | lexicon block, definition |
| `icon-limit` | icons the lexicon emits, after axis products | 100 000 | lexicon block |
| `expression-depth` | how deeply one expression nests (§5) | 256 | lexicon block, definition |
| `transform-chain` | transform steps one shape is drawn through, in every transform around it, through fragments, and a badge one more | 64 | lexicon block, definition |
| `text-length` | characters in one text, once interpolated | 1 000 | lexicon block, definition |
| `text-limit` | characters of text in one icon, all its texts together | 10 000 | lexicon block, definition |
| `glyph-limit` | glyphs one font draws in the styles it is set in, `notdef` included | 65 536 | lexicon block, font |

Each takes a positive whole number, or `#null` for no limit. A
definition's value wins over the lexicon block's, which wins over the
default; a definition's value is inherited down its `extends` chain,
child over parent, and an axis variant can set one like any other body
node. `glyph-limit` counts a font, so a font and its styles set it, not
an icon.

Expansion counts as it goes and stops at the node that goes past a
limit, with an error naming the limit, its value and where that came
from, and how to raise it or switch it off; nothing past that point in
the icon is expanded. `icon-limit` is checked by multiplying axis sizes,
before any combination is built.

The defaults are the values above unless the compiler was built with
`ISOLEX_DEFAULT_SHAPE_LIMIT`, `ISOLEX_DEFAULT_REPEAT_LIMIT`,
`ISOLEX_DEFAULT_INSERT_DEPTH`, `ISOLEX_DEFAULT_ICON_LIMIT`,
`ISOLEX_DEFAULT_EXPRESSION_DEPTH`, `ISOLEX_DEFAULT_TRANSFORM_CHAIN`,
`ISOLEX_DEFAULT_TEXT_LENGTH`, `ISOLEX_DEFAULT_TEXT_LIMIT` or
`ISOLEX_DEFAULT_GLYPH_LIMIT` set. Such a
build accepts or rejects a lexicon differently but never compiles it to
different IR; a lexicon that needs more than the values above should
say so with a directive.

## 10. CLI

`isolex check <dir>` parses, resolves, expands and projects, reporting
diagnostics and counts. `isolex compile <dir> --ir <path|->` writes the
IR JSON. `compile` and `render` take one icon too: `--icon <name>`,
then any of `--param <name>=<number>` and `--text <name>=<text>`, as
`compile_one` below. `isolex docs <dir>` emits per-category markdown reference
tables from `describe` metadata. `isolex render <dir> --svg <out>
[--palette <file>]` writes one `<icon>.svg` per icon into `<out>` (§7),
coloring tokens from a built-in palette that a palette file (a JSON
object of token to CSS color) extends or overrides; a token with no
color, or an icon name that is not a safe file name (letters, digits,
`-`, `_` and `.`, not starting with `.`), is an error, and a render
that fails writes nothing. Diagnostics go to standard error,
warnings first. Exit status: 0 clean (warnings included), 1 errors or
I/O failure, 2 usage.

**Fonts.** `isolex font import <font> [--out <file>]` writes an isolex
font (§4) from a TrueType, OpenType (TrueType or CFF outlines) or WOFF
1.0 font:

- each glyph's outline as filled path data on the font's grid (its own
  units per em, or `--em`), curves as they are, or as lines where
  within `--tolerance` of them;
- for the characters `--chars` names (`ascii`, `latin1`, the default,
  `all`, or the characters themselves), less those text cannot set
  (§4), and the ligatures GSUB `liga` makes of them, as glyphs named for
  the characters they replace;
- advances, ascent and descent, and kerning, GPOS pair adjustment of
  the `kern` feature or else a `kern` table; and notdef;
- `--style <axis>.<variant>=<source>` makes a style of another weight or
  slope: a font file, or the same variable font at other axis values
  (`wght:700,wdth:87.5`); the variant draws only what differs, on the
  axis `<font>-<axis>`. A variable font is otherwise read at its default
  instance, and its kerning always is.

The font's license decides: one its OS/2 fsType marks restricted, or
whose outlines may not be embedded, is not converted, and one that may
not be subset is converted whole (`--chars all`) or not at all. Its
copyright, trademark, license and license URL are copied, as comments
and as `describe` lines (`Copyright: …`, `Trademark: …`, `License: …`,
`License URL: …`, with `Family: …` and `Version: …`), and what is
written compiles, in each of its styles, or nothing is.

`isolex font export <dir> --font <name> [--style "<styles>"]` writes a
font of the lexicon in those styles, `--otf <file>` as TrueType or
`--ufo <dir>` as a UFO 3 source. Its glyphs are drawn as text draws
them; filled shapes keep their outlines, strokes become outlines as SVG
draws them (butt caps, miter joins to a limit of 4), the slant is
sheared in, ligatures become GSUB `liga` and kerning a `kern` table (a
UFO's `features.fea` and `kerning.plist`), and the `describe` notices
become the font's names. TrueType's curves are quadratic, so cubics
are approximated within `--tolerance` (an em/2000 by default), and its
points are whole units of `--upm` to the em (the font's em when that is
a whole number from 16 to 16384, else 1000). The same lexicon always
writes the same bytes. These two need the `font-tools` feature, which
the command line is built with by default and the WebAssembly module
without.

The same compilation is available without a file system: as the
library function `isolex::compile`, and as a WebAssembly module that
imports nothing and takes and returns JSON (`isolex::json`), whose IR is
the IR `compile` writes (compact rather than indented) and whose
diagnostics are the CLI's as data.

**One icon.** A value known only when one thing is drawn (its name, a
code, a count) is a param set on a compile of that one icon:
`isolex::compile_one(files, icon, params)`, or `"icon"` and `"params"`
beside `"files"` in the JSON. The icon is named as the lexicon emits it,
a variant's name included. Each param takes the slot of the one it
sets, as a child's value would, late-bound into everything that reads
it: a number for a number param, and text, taken as it is with no
interpolation, for a text param. The whole lexicon is parsed and
resolved, and only that icon is expanded and projected, with those it
badges, which it draws; the IR holds that icon alone. The same inputs
always compile to the same IR, so a host can keep it by them. An icon
the lexicon does not emit, or a param it does not have, or of the other
type, is refused as a request rather than reported as a diagnostic.

**Many icons.** A host drawing many things at once (a list, a sheet of
labels) asks for them together, from one parse: `isolex::compile_many
(files, requests)`, or `"requests": [{"icon", "params"}, …]` in place
of `"icon"` and `"params"` in the JSON. The lexicon is parsed and
resolved once, which is most of the work when it holds a font; each
request then compiles exactly as one icon would, to the same IR and
the same errors, and is refused on its own. The lexicon's own errors
and warnings are reported once, beside the results; with errors,
nothing is expanded.

## 11. Conformance

A conforming compiler produces byte-identical IR for identical
lexicons, rejects everything §5 forbids with located diagnostics, and
passes the reference test suite (parser goldens, per-rule semantics
tests, e2e IR golden). It reads source as the whole of KDL 2.0: every
input of the official KDL test suite (vendored in `tests/kdl-suite`)
parses exactly when that suite says it should, to the document it
says. And it takes nesting of any depth, of blocks and of expressions,
without exhausting a call stack: how deeply a lexicon nests costs
memory, and past `insert-depth` and `expression-depth` (§9), an error.
