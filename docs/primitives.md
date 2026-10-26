# Primitives reference

Every geometry node, grouped by where it is legal. Coordinates for 3-D
nodes are **tile units**; for 2-D nodes they are **output (screen)
units**. In any numeric slot a bare number is a literal and a quoted
string is an [expression](guide.md#the-expression-language). Painter
order is document order after resolution.

Style properties: `fill`, `stroke` (a theme token or `none`), `width`,
`dash` (e.g. `dash="3 2.5"`), `opacity`, and the `when="…"` guard. Most
nodes take all of these; the exceptions are named below. A property a
node does not take is an error, never ignored:

- `box` takes `fills=`, `opacity=` and `when=`, not `fill=`.
- `cylinder` takes `top=`, `side=`, `opacity=` and `when=`.
- `facerect` takes `fill=`, `opacity=` and `when=`.
- `shadow`, `at` and `repeat` take only `when=`; `pt` takes none.
- `badge` takes `corner=`, `scale=` and `when=`.
- `transform` takes `origin=`, its steps (`translate=`, `rotate=`,
  `scale=`, `skew-x=`, `skew-y=`, `mirror=`, or `matrix=`), `onto=` and
  `of=`, `bounds=`, `strokes=` and `when=`.
- `turn` and `mirror-diagonal` take `origin=` and `when=`.
- `text` takes `font=`, `size=`, `style=`, `anchor=`, `baseline=`,
  `max-width=`, `fill=`, `bounds=` and `when=`.

## 3-D nodes (on the isometric grid, outside `at`)

### `box x y z w d h`
A rectangular volume at `(x,y,z)` sized `w×d×h`. Emits its left, right,
and top faces (fills `body-left`/`body-right`/`body-top`, stroke
`outline` at 1.2). `fills="virt"` swaps in the translucent-guest tokens;
`fills="none"` outlines only. `opacity=` sets face opacity.
```kdl
box 0 0 0 2 1 0.3
box 0 0 0 2 1 1 fills="virt" opacity=0.18
```

### `tile x y z w d`
A flat diamond (zero height) — a slab or floor. Default fill `body-top`,
stroke `outline` 1.2.
```kdl
tile 0 0 0 1 1 fill="bg-zone"
```

### `cylinder cx cy z r h`
A disc/drum centred at `(cx,cy)` radius `r`, height `h`: body path then
top ellipse (defaults `body-right` / `body-top`). `top=`/`side=` set the
fills.
```kdl
cylinder 0.5 0.5 0 0.4 0.18
```

### `facerect "front"|"right" a b z extent h`
A rectangle mounted flat on the front or right wall of a volume — drive
bays, bezels, LEDs. Default fill `ink`, stroke `none`; `opacity=` fades
it.
```kdl
facerect "front" 0.14 0.95 0.12 0.13 0.62 opacity=0.3
```

### `poly3d { pt x y z … }`
An arbitrary projected polygon; children are `pt x y z` (and `repeat`).
```kdl
poly3d fill="ink" stroke="none" {
  pt 0.17 0.5 0.06
  pt 0.43 0.5 0.06
  pt 0.30 0.5 0.26
}
```

### `shadow cx cy rx ry`
A soft ground ellipse (fill token `shadow`, opacity 0.35, no stroke) to
sit an object on.
```kdl
shadow 0.5 0.5 0.48 0.48
```

### `seg ax ay az bx by bz`
A straight segment between two tile-space points; both endpoints project
and it is emitted as one path. For links, mortar lines, joins.
```kdl
seg 0.5 0.5 0.66 1.5 0.5 0.66 stroke="ink-muted" width=1.4 dash="3 3"
```

### `turn a { …3-D nodes… }`
Turns its children in tile space, before projection, by a quarter-turn
about a vertical axis through `origin="x y"` (tile units, default
`"0 0"`): `a` is 0, 90, 180 or 270 degrees (any multiple of 90, or an
expression that is one); 90 carries +x onto +y. Boxes stay boxes, their
width and depth trading places; points, anchors and centers turn. A
`facerect` moves to the wall it now faces, and is dropped if that wall
faces away, since isolex draws only the faces you can see. Painter
order stays the document's.
```kdl
turn 90 origin="0.5 0.5" {
  box 0 0 0 2 1 1
  facerect "right" 2 0.2 0.2 0.6 0.6   // now on the front wall
}
```

### `mirror-diagonal { …3-D nodes… }`
Reflects its children across the diagonal x = y through `origin="x y"`:
x and y trade places, front and right walls too, and both stay in view.
For the other hand of a symmetric part.

### `at x y z { …2-D nodes… }`
Projects the tile point `(x,y,z)` to a screen point and binds `x`/`y` for
its children — the bridge from the grid into screen space. `at 0 0 0`
projects to the origin and emits no wrapper, which is how flat `canvas`
icons place their 2-D artwork.

## 2-D nodes (inside `at`; coordinates are screen-absolute)

The anchor gives you `x`/`y`; you draw relative to them.

### `line x1 y1 x2 y2` — stroke `ink` 1.4.
### `circle cx cy r` — stroke `outline` 1.2, fill `none`.
```kdl
circle "x" "y" 9 fill="none" stroke="ink" width=1.4
```
### `ellipse cx cy rx ry` — stroke `outline` 1.2, fill `none`.
### `rect x y w h` (`rx=` for rounded) — stroke `none` 1.4, fill `none`.
```kdl
rect "x-10" "y-7" 20 14 rx=1.5 fill="none" stroke="ink" width=1.4
```
### `arc cx cy r a0 a1`
A circular arc, `a0→a1` in **degrees**, counter-clockwise from +x
(y-flipped to screen). The large-arc flag is set automatically when the
sweep exceeds 180°. Stroke `ink` 1.4.
```kdl
arc "x" "y" 8 30 150 stroke="ink" width=1.4
```
### `path d="…"`
A raw SVG path. Inside `d`, `{expr}` interpolates a numeric expression
with `x`/`y` in scope — everything outside the braces is passed through
verbatim, so the full SVG path grammar is available.
```kdl
path d="M{x-9},{y+4} v-9 h6 l2,2.5 h10 v6.5 z" stroke="ink" width=1.4 fill="none"
```
### `poly { pt x y … }` — fill `ink`, stroke `none`.
```kdl
poly fill="state-up" stroke="outline" width=1.2 {
  pt 12 2.5
  pt 21.5 20.5
  pt 2.5 20.5
}
```
### `text x y "…"`
A line of text, set from `(x, y)` in a [font](fonts.md) of the
lexicon's, and compiled to that font's glyphs: shapes like any other,
each in a group that places it. `{…}` interpolates a number or a text
param (`{{` is a brace).

- `font="name"`, `size=s` (the em, in output units): required.
- `style="bold italic"`: the font's styles, one from each of its axes.
- `anchor="start"|"middle"|"end"` (default `start`): where `x` falls.
- `baseline="alphabetic"|"top"|"middle"|"bottom"` (default
  `alphabetic`): where `y` falls, by the font's ascent and descent.
- `max-width=w`: scaled down to fit, if it is wider.
- `fill="token"`: its glyphs drawn in this token, fill and stroke.
- `bounds=`: as a transform's; text must stay inside the icon's frame.
```kdl
text "x" "y + 3" "PoE {ports}" font="plain" size=8 anchor="middle" style="bold"
```

## Works in either dimension

### `insert "fragment" arg=value … { }`
Stamps a [fragment](guide.md#fragments-reusable-geometry), binding its
params. Legal on the grid and inside `at` — the fragment's body decides
validity at the insertion site.
```kdl
insert "disk-stack" cx=0.55 cy=0.55 n=4
```

### `repeat count "var" { … }`
Emits the body `count` times with `var` bound `0 … count-1` (count
evaluated up front; every iteration counts toward the icon's
`repeat-limit`, and every shape toward its `shape-limit`). Nest them for
grids.
```kdl
repeat 3 "row" {
  repeat 4 "col" {
    facerect "front" "0.36 + col * 0.38" 2 "0.9 - row * 0.28" 0.28 0.18
  }
}
```

### `badge "icon-name" corner="ne" scale=0.5`
Composites another icon's compiled shapes into a corner of the icon's
frame (`ne` default; `nw`/`se`/`sw`; `scale` default 0.5). The target
must not itself contain a badge. This is how the state axis drops a
status marker onto an object.

### `transform … { … }`
Draws its children through a 2-D map, after projection, in either
dimension: the IR carries the map around them as a group, and their
coordinates and path data are untouched.

- `origin="cx cy"`: what rotate, scale, skew and mirror turn about
  (output units; inside `at`, `x` and `y` are in scope).
- Steps as properties, applied in this order: `mirror="across"` or
  `"down"`, `scale=2` or `scale="2 1"`, `skew-y=deg`, `skew-x=deg`,
  `rotate=deg` (clockwise), then `translate="tx ty"`.
- Or a `steps { … }` block, applied in the order written:
  `rotate 90`, `skew-x -12`, `translate 2 0`, `scale 2 1`, `skew-y 5`,
  `mirror "across"`, `matrix a b c d e f`.
- Or `matrix="a b c d e f"` alone, as SVG's `matrix()`.
- `onto="top"|"front"|"right" of="box x y z w d h"`: after the steps,
  lays flat artwork, drawn in output units from (0, 0), onto that face
  of that box, its top-left corner as seen.
- `strokes="fixed"` keeps stroke widths as written; by default they
  scale with the map, as in SVG.
- `bounds=`: what it draws must stay inside the icon's frame. Past it is
  an error, unless `bounds="grow"` (widen the viewBox), `"scale"` (shrink
  it to fit) or `"clip"` (cut it at the frame), or the icon's
  `transform-bounds` says so for all of its transforms.

Numbers in one property are apart by spaces; put an expression with
spaces of its own in parentheses: `origin="(w / 2) 0"`.
```kdl
transform rotate=90 origin="12 12" { insert "fan-blade" }
transform mirror="across" origin="16 0" { insert "port-bank" }
transform skew-x=-12 origin="x y" { path d="M{x-4},{y+6} L{x},{y-6}" }
transform origin="12 12" {
  steps { rotate 90; skew-x -12; translate 1 0 }
  at 0 0 0 { rect 8 6 8 12 }
}
transform onto="top" of="box 0 0 0 1 1 0.5" {
  at 0 0 0 { rect 8 8 24 24 fill="accent" }
}
```

## Fonts

A `font` is a top-level definition, not a node; see [the fonts
guide](fonts.md). Its body holds `em`, `ascent`, `descent`, `kind`,
and optionally `slant`, its glyphs, `notdef`, kerns, params, `axes` and
`describe`.

### `glyph "A" advance=6 { …2-D nodes… }`
The glyph for a character, or for a run of them (a ligature). Drawn in
the font's em grid from its origin on the baseline, y up. Takes only
`advance=`.
```kdl
glyph "A" advance=5.5 { path d="M0,0 L2,7 L4,0 M0.8,2.5 H3.2" width="weight" }
```

### `notdef advance=6 { … }`
What is drawn for a character no glyph draws.

### `kern "A" "V" -0.6`
Space added between two glyphs side by side, in the em grid.

### `pt x y [z]`
A polygon vertex — only inside `poly` (2 coords) or `poly3d` (3 coords),
and may be produced by a `repeat`.

## Tokens, not hex

`fill` and `stroke` take **theme tokens**, resolved to colors by the
renderer — never raw hex. Common tokens: `ink`, `ink-muted`, `outline`,
`body-top`/`body-left`/`body-right`, `bg-zone`, `virt`, `accent`,
`accent-dim`, and any others your `lexicon { tokens … }` declares. `none`
means no fill/stroke.
