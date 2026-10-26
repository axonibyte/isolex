# Fonts and text

An icon can carry words: a port count on a faceplate, `U12` on a rack
unit, a bin's label. In isolex a font is not a file a renderer looks
up. It is part of the lexicon, its glyphs drawn in isolex like any
other geometry, and text is set in it when the lexicon compiles. The IR
holds only finished shapes, so every renderer draws text exactly as the
reference SVG does, with no fonts, layout or text code of its own.

## A font

```kdl
font "plain" {
  describe "A small stroke-built face for labels."
  em 10          // the design grid: a text of size=10 is drawn 1:1
  ascent 7       // how far it reaches above its baseline
  descent 3      // and below it
  kind "stroke"  // drawn along its glyphs' middles
  param weight 1
  glyph " " advance=3
  glyph "A" advance=5.5 { path d="M0,0 L2,7 L4,0 M0.8,2.5 H3.2" width="weight" }
  glyph "V" advance=5.5 { path d="M0,7 L2,0 L4,7" width="weight" }
  kern "A" "V" -1
}
```

Each glyph is drawn in the font's **em grid**: its origin is on the
baseline where the pen stands, x runs right and y runs **up**, as fonts
are drawn. It takes the 2-D nodes (`path`, `poly`, `line`, `rect`,
`circle`, `ellipse`, `arc`, `repeat`, `insert`, `transform`) and sees
the font's params, nothing else. `advance` is how far it moves the pen.

A font is one of two kinds:

- **stroke**, whose glyphs are lines drawn at a width, usually a param
  like `weight` above. These suit small sizes, thermal printers and
  plotters.
- **outline**, whose glyphs are filled shapes, as a converted TrueType
  or OpenType font's are. Draw a hole in a letter as a contour wound
  the other way round from its outside.

A font needs `em`, `ascent`, `descent` and `kind`.

### Ligatures, kerning and notdef

- A glyph named for several characters is a **ligature**. Wherever that
  run occurs, it is drawn in their place; at each place the longest
  name that matches wins. `glyph "--" advance=9 { … }` turns two
  hyphens into one dash.
- `kern "A" "V" -1` closes up the space between two glyphs, by their
  names.
- `notdef advance=5.5 { … }` is drawn for a character the font has no
  glyph for. Without one, such a character is an error.

## Bold and italic are styles

A font's weights and slopes are not separate fonts. They are variants
of axes it opts into, overlaying it the way an icon's variants overlay
the icon:

```kdl
axis "weight" {
  variant "bold" { param weight 1.8 }
}

axis "slope" {
  variant "italic" { slant -12 }
}

font "plain" {
  // …
  axes "weight" "slope"
}
```

- A **bold** that restates `weight` works for any stroke-built font,
  even one drawn only in its regular weight. An outline font can do the
  same by stroking its glyphs in their own color at a width param that
  is 0 until bold sets it.
- `slant` leans the glyphs as `skew-x` would (`-12` leans right) about
  the baseline.
- A style can also set `em`, `ascent` or `descent`, and replace or add
  glyphs and kerns, which is how a font converted from separate weights
  keeps each weight's own outlines.

A text asks for styles by name, one from each axis: `style="bold
italic"` needs `bold` and `italic` on different axes, as above.

## Setting text

```kdl
icon "port-label" {
  category "net"
  canvas 48 24
  param ports 48
  part "plate" {
    at 0 0 0 {
      rect 2 2 44 20 rx=3 fill="body-top" stroke="outline" width=1.2
      text 24 12 "PoE {ports}" font="plain" size=10 anchor="middle" baseline="middle"
    }
  }
}
```

- `text x y "…"` stands among 2-D nodes, inside `at`.
- `size` is the em in output units. Stroke widths scale with it, so a
  stroke-built font keeps its proportions at every size.
- `anchor` sets where `x` falls along the text: `start`, `middle` or
  `end`.
- `baseline` sets where `y` falls on it:
  - `alphabetic` puts `y` on the baseline;
  - `top` puts it on the ascent line;
  - `bottom` puts it on the descent line;
  - `middle` puts it halfway between those two.
- `max-width` shrinks a text that would be wider than it.
- `fill` draws the glyphs in one token, every paint but `none`, so one
  font serves every color.
- Text must stay inside the icon's frame, as a transform must. If it
  can't, `bounds="grow"`, `"scale"` or `"clip"` says what to do.

To lay text on a face of a box, set it inside a `transform onto=`:

```kdl
icon "rack-unit" {
  category "net"
  footprint 2 1 0.5
  param u 12
  part "chassis" {
    box 0 0 0 2 1 0.5
    transform onto="front" of="box 0 0 0 2 1 0.5" {
      at 0 0 0 { text 6 15 "U{u}" font="plain" size=10 }
    }
  }
}
```

### What text can say

`{…}` in text is either a number or a text param:

- An expression is printed with the zeros that add nothing left off,
  so 48 reads `48` and 2.5 reads `2.5`.
- A text param's name prints its text.
- `{{` and `}}` are literal braces.

A text param is marked `(text)`, since an unmarked string is an
expression:

```kdl
base "tagged" {
  category "net"
  canvas 48 24
  param n 7
  param tag (text)"A{n}"
  part "tag" {
    at 0 0 0 { text 4 16 "{tag}-{n * 2}" font="plain" size=8 }
  }
}

icon "tagged-b" {
  extends "tagged"
  param tag (text)"P{n + 3}"     // late-bound: the base's text says P10-14
}
```

Pass text to a fragment the same way: `insert "plate"
label=(text)"U{u}"`.

Text is set left to right, a glyph after another. Text in scripts that
need more than that is refused with an error rather than drawn wrong:
right-to-left scripts, combining marks, and scripts whose letters change
shape with their neighbors. So are control characters, since text is
one line.

## Converting fonts

A TrueType, OpenType or WOFF font becomes an isolex font, and an isolex
font becomes one of those, or a UFO source for a font editor:

```sh
isolex font import NotoSans-Regular.ttf --out fonts/noto-sans.ilx \
    --style weight.bold=NotoSans-Bold.ttf
isolex font import NotoSans[wdth,wght].ttf --chars ascii --style weight.bold=wght:700
isolex font export src/ --font plain --style "bold italic" --otf plain-bold-italic.ttf
isolex font export src/ --font plain --ufo plain.ufo
```

- **Import** keeps Latin-1 unless `--chars` says otherwise (`ascii`,
  `all`, or the characters themselves). Glyph outlines become filled
  paths on the font's own grid. Advances, kerning and `liga`
  ligatures come along.
  - Each `--style` makes a style from another file, or from the same
    variable font at other axis values. It draws only what differs from
    the regular.
- **Licenses come first.** A font whose embedding bits say it is
  restricted is not converted. One that may not be subset is converted
  only whole (`--chars all`). Its copyright, trademark and license are
  copied into the file, as comments and as `describe` lines, and export
  writes those back into the font it makes. **The converted file is a
  derivative of the font, under the font's license**, not under your
  lexicon's.
- **Export** draws each glyph as text would.
  - A stroke-built font's strokes become outlines, as SVG draws them. A
    slant is sheared in.
  - TrueType holds only quadratic curves, so cubics are approximated,
    to within an em/2000 unless `--tolerance` says otherwise.
  - A UFO keeps every curve as drawn.
  - Each style is its own face: export bold and italic one at a time.

## Values known only when one thing is drawn

A host drawing an icon for one asset can set its params for that one
compile:

```sh
isolex compile src/ --ir - --icon port-label --param ports=24
```

From a program, call `isolex::compile_one(files, "port-label",
params)`, or add `"icon"` and `"params"` to the WebAssembly module's
JSON. The same inputs always give the same IR, so the result can be
cached by them.
