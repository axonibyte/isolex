# Primitives reference

Every geometry node, grouped by where it is legal. Coordinates for 3-D
nodes are **tile units**; for 2-D nodes they are **output (screen)
units**. In any numeric slot a bare number is a literal and a quoted
string is an [expression](guide.md#the-expression-language). Painter
order is document order after resolution.

Style properties, where applicable: `fill`, `stroke` (a theme token or
`none`), `width`, `dash` (e.g. `dash="3 2.5"`), `opacity`, and the
`when="…"` guard.

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
Composites another icon's compiled shapes into a viewBox corner (`ne`
default; `nw`/`se`/`sw`; `scale` default 0.5). The target must not itself
contain a badge. This is how the state axis drops a status marker onto an
object.

### `pt x y [z]`
A polygon vertex — only inside `poly` (2 coords) or `poly3d` (3 coords),
and may be produced by a `repeat`.

## Tokens, not hex

`fill` and `stroke` take **theme tokens**, resolved to colors by the
renderer — never raw hex. Common tokens: `ink`, `ink-muted`, `outline`,
`body-top`/`body-left`/`body-right`, `bg-zone`, `virt`, `accent`,
`accent-dim`, and any others your `lexicon { tokens … }` declares. `none`
means no fill/stroke.
