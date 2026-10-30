# isolex studio: the interface

This is the specification every studio interface unit is held to (#59).
It writes up the working prototype approved in review, which runs the
isolex compiler as WebAssembly in a web page and draws from the IR it
returns. Where the prototype shows a behavior, this describes it; where
it doesn't (the first run, a library changed on disk, an edit the
studio cannot make on its own), this decides it.

The studio edits a library: a folder of `.ilx` files compiled as one
lexicon (SPEC §1). It reads and writes nothing else.

## Principles

- **Drawing comes first.** The studio draws everything the language
  does. Panels serve the canvas, not the other way round.
- **Every edit is an edit of the source.** A gesture on the canvas
  becomes a change to the text, shown in the Source tab, undone and
  redone as one step. Comments, order and formatting outside the
  numbers and nodes an edit means to change are kept.
- **What you see is what isolex draws.** The canvas, the thumbnails
  and the previews are the IR the compiler returns, drawn as the
  reference SVG renderer draws it. The editor draws only outlines,
  handles and guides of its own.
- **The library is one thing.** An edit can change other icons (those
  that extend, insert, badge or set text in what changed), and the
  Tree shows them change. One undo history covers the library; saving
  writes every changed file at once.
- **Nothing is replaced silently.** When a gesture would overwrite an
  expression, a param or an inherited part, the studio asks first
  (see *When an edit needs a decision*).

## Look

- **Themes.** The window is dark by default, with a light canvas. Each
  is a switch in the top bar, and remembered per user:
  - the window: Dark, Light, or the system's;
  - the canvas: Light or Dark. A dark canvas draws with the palette
    block's dark colors (SPEC §2), a light one with its light colors.
- **Type.** Geist for the interface and Geist Mono for source,
  coordinates and names. Both ship with the app.
- **Color.** Interface colors are tokens of the app's own, one set per
  theme, never literals in components. A single accent marks the
  selection, the current tool, handles and the active tab. Drawing
  colors come only from the library's palette.
- **Density.** Controls are compact, as in a design tool: 38 px tool
  buttons, 13 px text, 11 px uppercase section labels.

## Layout

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ isolex studio   icon [server-2u        ]  ↶ ↷        [Light|Dark canvas] [Dark|Light|System] │ top bar
├────┬─────────────┬───────────────────────────────────────────┬────────────────┤
│ V  │ PARTS     + │ [Snap ¼▾] [Drawing on top of body · z 0.5]│ Design Source   │
│ A  │ body        │                                           │ Tree   Preview  │
│ H  │   box  2×1×½│           isometric canvas                │                │
│ ── │ bays  ← here│                                           │ (the tab's     │
│ B  │   facerect  │                                           │  content)      │
│ T  │ …           │                                           │                │
│ C  │ ─────────── │                                           │                │
│ F  │ INHERITED   │      ( hint: what the tool does )         │                │
│ ── │ body · from │                                    − 100% + Fit           │
│ L R E G P  …     │                                           │                │
├────┴─────────────┴───────────────────────────────────────────┴────────────────┤
│ ● Compiled by isolex 0.5.0 in 2.1 ms   x 1.25 · y 0.5 · z 0   7 definitions … │ status bar
└───────────────────────────────────────────────────────────────────────────────┘
```

- **Top bar:** what is being edited, as its kind ("icon", "icon,
  extends server-2u", "variant down of axis") and a name field that
  renames it; undo and redo, and beside them the undo history (every
  step done, oldest first, then those undone; a click goes back or
  forward to one); the Library, Lexicon and Export dialogs; the two
  theme switches.
- **Tool rail (left):** one button per tool with its key, grouped:
  selecting (Select, Node, Pan), 3-D (Box, Tile, Cylinder, Face rect,
  Shadow, Segment, 3-D polygon), flat (Line, Rectangle, Ellipse, Arc,
  Polygon, Pen), Text. A tool that cannot draw in what is open (a 3-D
  tool on a flat icon) is disabled, and its tooltip says why.
- **Parts panel:** the open definition's parts and their shapes (see
  *Parts*).
- **Canvas** (center), with three floating controls: the snap step and,
  while drawing, the plane being drawn on (top left); a hint line saying
  what the tool does and which keys change it (bottom center); zoom
  out, the zoom level, zoom in and Fit (bottom right).
- **Tabs (right):** Design, Arrange, Source, Tree and Preview.
- **Status bar:** the compile state (compiled, with the compiler's
  version and how long it took; or *does not compile*, with the first
  error, while the last good drawing stays); the pointer's coordinates
  (tile x, y and the floor's z, or flat x and y); counts. The compile
  state opens the problems list: the library's errors and warnings by
  file, each opening its place in the Source tab.

Below 900 px wide the panels stack under the canvas.

## The canvas

- **The projection** is the compiler's (SPEC §7): tile x runs right and
  down, y left and down, z up. A 3-D icon shows the isometric grid on
  its floor at the snap step, unit lines stronger, and its footprint as
  a dashed box. A flat (`canvas`) icon shows a square grid at the flat
  snap step, every fourth line stronger, and its canvas as a dashed
  rectangle.
- **Pan and zoom:**

  | Input | Does |
  |---|---|
  | Wheel | Zoom at the pointer |
  | Ctrl+wheel (Cmd on macOS) | Pan north and south |
  | Shift+wheel | Pan east and west |
  | Space+drag, middle-drag, the Pan tool (H) | Pan |
  | 0, Fit | Fit the icon's frame |

- **Snapping.** Positions snap to the step chosen on the canvas: 1, ½,
  ¼ or ⅛ of a tile, or off. Flat positions snap to four times that
  step in output units (¼ tile is 1 unit), never finer than ½. Other
  shapes are snap targets too, within 8 pixels on screen, shown by an
  indicator naming what was snapped to: a point first, then an edge.
- **Alignment guides**, as Inkscape 1.2 draws them. A point that is
  not on another shape's point can still line up with one along the
  plane's axes (the tile x and y on a 3-D floor, across and down when
  flat), when that guide is nearer than the grid's line. The guide is
  drawn dashed from the point lined up with, and the indicator names
  it. Moving a shape lines up its own corners, edge midpoints and
  center, not only the point it was grabbed by.
- **Advanced**, beside the step, lists each kind of target, each
  switched on or off, and remembered per person:

  | Target | What |
  |---|---|
  | Grid lines and intersections | The step; off, positions are free |
  | Corners | Of boxes' and tiles' footprints, at their floor and top; of flat rectangles |
  | Edges | Of the same, anywhere along them |
  | Edge midpoints | Of those edges, and of lines |
  | Face centers | Of footprints, tops and rectangles; of cylinders, circles, ellipses and arcs |
  | Anchors | Where an `at` plane flat shapes are drawn on has its origin |
  | Path nodes | Points of polygons, nodes of paths, ends of lines; not the glyphs of words |
  | The frame | A flat icon's canvas; a 3-D icon's footprint, at its floor and top |
  | Alignment guides | As above |
- **Feedback.** While drawing or dragging, the size or position being
  set is written beside the pointer ("2 × 1 × 0.5", "z 0.75",
  "r 0.25").

## Tools

### 3-D

| Tool | Key | Gesture | Writes |
|---|---|---|---|
| Box | B | Drag a footprint on a floor, release, move up or down for the height, click. Esc cancels. | `box x y z w d h` |
| Tile | T | Drag on a floor. | `tile x y z w d` |
| Cylinder | C | Drag from the center out to the radius, release, then set the height as a box's. | `cylinder cx cy z r h` |
| Face rect | F | Hover the front or right side of a box (it is highlighted), drag a rectangle on it. | `facerect "front"\|"right" …` |
| Shadow | Shift+S | Drag an ellipse on the floor. | `shadow cx cy rx ry` |
| Segment | Shift+L | Click two points on floors. | `seg ax ay az bx by bz` |
| 3-D polygon | Shift+G | Click points on a floor or a face, click the first or press Enter to close. | `poly3d { pt … }` |

**Floors.** A floor is the ground (z 0) or the top face of a shape
under the pointer: starting a box over another box stands it on that
box, and a chip says so ("Drawing on top of body · z 0.5").

### Flat

Flat shapes stand on an `at` anchor. By default the studio writes them
on one anchor at the origin, where a flat shape's numbers are the
icon's own output units (and, in a flat icon, its canvas's).
Consecutive flat shapes in a part share the anchor.

| Tool | Key | Gesture | Writes |
|---|---|---|---|
| Line | L | Drag; Shift keeps 15° steps. | `line x1 y1 x2 y2` |
| Rectangle | R | Drag; Shift for a square. Corner radius in Design. | `rect x y w h` |
| Ellipse | E | Drag; Shift for a circle. | `circle` or `ellipse` |
| Arc | Shift+A | Drag the radius from the center, then sweep. | `arc cx cy r a0 a1` |
| Polygon | G | Click each corner; click the first, or Enter, to close; Esc cancels. | `poly { pt … }` |
| Pen | P | Click for a corner; drag for a smooth point with mirrored handles; click the first point to close (dragging there sets its handles); Enter leaves it open; Esc cancels. | `path d="M… C… Z"` |
| Text | X | Click on an anchor and type; font, style, size and alignment in Design. | `text x y "…" font=…` |
| Insert | I | Choose a fragment from the list (found by name), then click where it goes; or drag one from the list, or a fragment's card in the Tree's focus view, onto the canvas. | `insert "f" cx=… cy=…` |
| Badge | Shift+B | Choose an icon, then click near the corner it goes in; or drag one from the list or the Tree. | `badge "icon" corner=…` |

While the pen or polygon tool is drawing, the segment to the pointer is
shown dashed, the first point is filled, and each point's handles show.

**Insert and Badge** list what they place, as Inkscape's Symbols
dialog does: the library's fragments, or its icons, found by name. An
insert goes where it is dropped, at the snap step, as the fragment's own
params when it has a pair for it (`cx` and `cy`, or `x0` and `y0`; `x`
and `y` are the language's own), else moved there on the screen by a
`transform translate=` round it; a flat fragment goes on the anchor flat
shapes are drawn on. A badge goes at the corner of the frame nearest the
drop. Either is selected as one thing after, one undo step. One that
would make a cycle is refused, naming the definitions it would join
(*That would make a cycle: peg → chain → peg*), as are an icon with a
badge of its own and what is not a fragment or an icon; the list shows
those disabled, saying why.

### Selecting and shaping

| Tool | Key | Gesture |
|---|---|---|
| Select | V | Click to select, Shift+click to add to the selection or take out, click again for the turn handles; drag to move a shape on its floor (flat shapes in their plane, face rects along their face); Alt+drag lifts a 3-D shape; drag a handle to resize. |
| Node | A | Drag a point or a curve handle of a path, polygon or line. A smooth point stays smooth (its handles turn together) unless Alt is held. Alt+click a point switches it between corner and smooth. Double-click a segment adds a point. Delete removes the point last touched, while the shape keeps enough. |
| Measure | M | Drag to measure, as Inkscape's measure tool: the distance, the angle and how far along each axis, beside the pointer, in tiles on the floor (or on a top face, at its height, when started on one) or in units on a flat canvas; snapped as drawing is. Shown until the next, or Esc; it writes nothing. |
| Dropper | D | Click a shape to pick its fill, Shift+click its stroke: the token is set on everything selected whose paint is its own, as one undo step, writing only that; with nothing selected, it paints what is drawn next. |
| Pan | H | Drag to pan. |

**Handles** are drawn in screen size: corner squares on a box's or
tile's footprint, an arrow above a box's or cylinder's top for height,
a radius handle on a cylinder, corner squares on a face rect and a
flat rectangle, radius handles on an ellipse.

### Keys

| Key | Does |
|---|---|
| Arrows | Nudge by the snap step; with Shift, a whole tile (or 4 units flat). A face rect moves along its face, up and down in z. |
| PageUp, PageDown | Raise or lower a 3-D shape by the snap step |
| Ctrl+D | Duplicate beside the original |
| Delete, Backspace | Delete the selection (or, with Node, a point) |
| Ctrl+Z, Ctrl+Shift+Z (Ctrl+Y) | Undo, redo |
| Ctrl+Shift+A | Align and Distribute (the Arrange tab) |
| Ctrl+F | Find, across the library |
| Esc | Cancel what is being drawn; else clear the selection, all of it |
| Enter | Finish a pen path open, or close a polygon |
| Space (held) | Pan by dragging |

The keymap is one table in the code, with a test that fails on any key
bound twice.

## Parts

The Parts panel lists the open definition's parts, top to bottom in
source order, and under each its shapes, topmost (last drawn) first:

- each shape with its kind and a short note ("2×1×0.5", "front ×4",
  "8 points, closed");
- the part new shapes go into, marked *drawing here*; a click on a
  part makes it so; a double-click renames it;
- **+** adds a part;
- shapes reorder by drag, PgUp/PgDn and Home/End, which is painter
  order;
- show/hide and lock affect the editor only, never the source.

**Inherited.** Below a line, the parts the open definition draws but
does not own: a child's parent's (and grandparent's) parts, or, for a
variant, its icon's. Each names where it comes from and opens that
definition on click. On the canvas, inherited shapes are drawn but not
selectable; the hint line says where one comes from when hovered.

## Design

With a shape selected:

- its numbers, as the source names them (x y z w d h; cx cy r; for a
  face rect, the face's plane named as such);
- **paint:** fill and stroke as swatches of the palette's tokens (or
  *none*), the token's name under them, and the stroke width chosen
  from the lexicon's `stroke-widths`, never typed. A box's faces are
  painted body-top, body-left and body-right as the language paints
  them, and say so;
- opacity, where the shape takes one;
- **repeat:** copies and step along the shape's axis, written as one
  `repeat` (`repeat 8 "i" { facerect "front" "0.1 + i * 0.23" … }`);
- for a path: open or closed;
- for a badge: the icon, the corner and the scale;
- for an insert: the fragment, and its params, each as the insert sets
  it or the fragment's default; each opens what it places;
- Duplicate, Bring forward, Send back, Delete.

With nothing selected: the icon's footprint (from what is drawn, read
only) or its canvas (editable), and its params: a row each, own or
inherited, with a slider that previews a value and *Set as default*,
which writes it.

## Arrange

**Align and Distribute,** as Inkscape's dialog (Ctrl+Shift+A), over
the shapes selected (Shift+click for several):

- **Align** lines up each shape's lowest edge, middle or highest edge
  along the isometric x, y or z (bottom, middle, top for z); for flat
  shapes, along their plane's x and y (left, center, right; top,
  middle, bottom);
- **relative to** the first chosen, the last chosen, the selection's
  bounds, or the icon's footprint (a flat icon's canvas);
- **Distribute** spreads three or more evenly along an axis, by their
  middles or by the gaps between them, the outermost two staying.

Each is one undo step, written as a drag writes a move: a value
written as an expression asks. A face rect stays on its wall and a
shadow on the floor. What draws more than one shape (a part, a group,
a repeat), text, and shapes in another plane than the selection's are
left out, and the panel says how many.

**Transform,** as Inkscape's dialog, over what is selected (shapes,
groups, repeats, inserts), written as the language writes each (SPEC
§4), never by rewriting numbers or path data:

- **Move** by tiles (units, for flat shapes), as their numbers;
- **Rotate** by the degrees given, either way, **Scale** by percentages
  (proportions kept, or not), **Skew** by degrees, and **Mirror** across
  or down: a `transform` round what is selected, on the screen, about
  the middle of what it draws (`transform rotate=15 origin="0 60"`);
- **Turn** a 3-D selection a quarter either way, or mirror it across
  the diagonal: a `turn` or `mirror-diagonal` round it, in tiles, about
  the middle of its footprint, so it shows the faces it turns toward.

**On the canvas,** as in Inkscape, a second click on the selection
shows its turn handles in place of its own, round all of what is
selected: drag a corner round the middle to rotate (to 15°, to 1° with
Shift), or the middle of a side to scale across or down (to 5%, both
ways with Shift), the box previewed as it will be. A third click shows
its own handles again.

Neighbors chosen together go into one transform; others each into
their own; all of it one undo step. The transform is selected after,
and a step of its own kind adds to it (rotate 15 twice is `rotate=30`,
not two transforms); any other step wraps it again. What leaves the
icon's frame is an error, as the language has it, until `bounds=` on
the transform says otherwise.

## Find

Ctrl+F, as Inkscape's Find dialog: definitions, parts, tokens and text
across the library, found by a part of their name or words and listed
by kind, in the order written, with where each is written. A token is
found where it paints (`fill="accent"`), not where a value only looks
like one. A definition opens; anything else opens its definition and
its place in the Source tab. Enter opens the first.

## Source

The library's source, with the selection's line highlighted and kept in
view, line numbers, highlighting, and the compiler's errors and
warnings, each opening its place. Typing in it edits the library as any
gesture does: it recompiles as you type, and the canvas follows.
Completion offers nodes, properties, tokens and the library's names.

## Tree

Every icon of the library as a live thumbnail:

- nested by `extends` (from `isolex::graph`), each icon's axis variants
  under it ("variant down of axis state"), and fonts in a section of
  their own;
- recompiled on every edit, in the background. An icon the edit redrew
  gets a pulsing dot that fades; one it broke shows a warning mark,
  keeps its last drawing, and shows its errors on hover; one it didn't
  change is not redrawn;
- **a click opens that icon, or that variant, for editing.** The one
  being edited is marked;
- hovering an icon says how it relates to what is open.

**Focus** shows the definition being edited as Unreal's Reference
Viewer shows an asset: it in the middle, what it depends on (`extends`,
`insert`, `badge`, axes, fonts) in columns to its left and what depends
on it to its right, a column a step, to a depth of 1 to 3 that a stepper
sets. A line runs from each definition to what it refers to, drawn by
kind. It is as live as the library view, a click opens a definition
and centers the view on it, and hovering one says how it relates to the
middle. The view and its depth are remembered.

## Preview

The open icon as isolex draws it at 16, 24, 32, 64 and 128 px, each on
a light and a dark ground.

## Children, variants and renaming

- **A child** (`extends`) opens with its parent's shapes drawn and
  listed as inherited. New shapes go into the child's own parts.
- **A variant** opens in its icon's frame: the icon's shapes are
  inherited, and what is drawn goes into the axis variant's parts (a
  badge, say). Editing it changes that variant of every icon that opts
  into the axis, and the Tree shows them all redraw.
- **Renaming** what is open (the name field) rewrites every reference
  to it, `extends`, `insert`, `badge`, `axes` and a text's `font`, as
  one undoable change. A name already taken is refused, naming the
  definition that has it.

## Fonts

Fonts are part of the library and are edited in the same editor (#84):

- a font opens on the canvas as a grid of its glyphs, each drawn as
  `text` draws it, notdef and ligatures included, with its styles (the
  variants of the axes it opts into) as a switch; its metrics (em,
  ascent, descent, slant, stroke or outline) are in Design;
- a glyph opens on its em grid, with baseline, ascent, descent and
  advance guides, y up as fonts are drawn, and is drawn with the flat
  tools, the pen and the node tool; its advance is a handle; kerning
  pairs are listed beside it and set by dragging the right-hand glyph
  of a pair preview;
- editing a glyph redraws every icon that sets text in the font;
- **Import** (a TrueType, OpenType or WOFF file; the characters, the
  em and extra styles in a dialog) and **Export** (OpenType or UFO, in
  chosen styles), as `isolex font import` and `export` do, with a font
  whose license forbids it refused, saying why.

## When an edit needs a decision

- **A value written as an expression or a param.** Its handle is drawn
  hollow. Dragging it opens a popover at the handle:
  - *Edit param `ports`* (and how many icons use it), which rewrites the
    param's default where it is declared;
  - *Replace with a number*;
  - *Cancel*.
- **An inherited part,** reached through its source (Design numbers, or
  the Source tab): the first change asks *Edit in server-2u* (listing
  the icons that follow) or *Override here*, which copies the part into
  the open definition. *Remember for this drag* applies the choice to
  the rest of the gesture.
- **An inherited param,** set as its default from the Design tab: *Edit
  in switch* (listing the icons that follow) or *Set it in switch-l3*,
  which declares the param in the open definition with the new value,
  for it and what extends it.
- **A copy from a repeat:** dragging one moves them all, and the hint
  line says so ("Moves all 8 copies").
- **A refusal** always says why, in the hint line.

## A library and the disk

- **First run, and no library open:** the canvas shows a welcome panel
  with *Open library folder…* (Ctrl+O), *New library…* (Ctrl+N), and
  the recent libraries (name, path, definition count, when opened;
  double-click opens). *Show this at startup* is a checkbox.
- **New library:** a name and a folder, and what to start from (empty:
  a lexicon with the standard tokens, a palette and one box, written as
  `lexicon.ilx` and `icons.ilx`; a copy of a library; or a library
  exported as a zip, whose `.ilx` files it takes, without a folder that
  wraps them all).
- **Opening** reads the folder as the CLI does (`isolex::library`).
- **Saving** (Ctrl+S) writes every changed file at once, each to a
  temporary file and renamed into place, so the folder is never
  half-saved. The title shows `●` while the library has unsaved
  changes.
- **Changed on disk** (by git, another editor or a script):
  - with no unsaved changes, the change is loaded, compiled and drawn,
    and the Tree marks what it changed, as if it had been edited here;
  - with unsaved changes, a strip under the canvas says *The library
    changed on disk (rack.ilx), and you have unsaved changes*, with
    *Reload from disk* (after confirming that unsaved changes are
    discarded) and *Save over it*. Until one is chosen, the studio
    keeps working on its own copy;
  - the studio's own saves never raise it.

## Library and Lexicon Properties

Two dialogs from the top bar manage what is not drawn:

- **Library** lists the folder's files, each with the definitions
  written in it, as VS Code's Explorer does. A click opens a
  definition; F2, a double-click or *Rename* renames one, rewriting
  every reference to it, or moves a file; a definition dragged onto a
  file, or sent there with *Move to…*, moves there with its comments.
  New files and definitions (of any kind, in any file) are made here.
  Each is one undo step; a name or path already taken is refused,
  offering one that is free.
- **Lexicon Properties** edits the `lexicon` and `palette` blocks
  (SPEC §2, §9), as Inkscape's Document Properties does, in tabs:
  *Tokens*, *Swatches* (each token's color on a light ground and a dark
  one), *Stroke widths*, *Reserved* (tokens painted only under a
  `grant`) and *Limits* (each a number, the default, or no limit). Each
  change edits its own node where it is written; a block not yet
  written is added, a palette after the lexicon block.

## Export

*Export…* (Ctrl+Shift+E), as Inkscape's Export dialog:

- **What:** the open icon, chosen icons (a checklist, with All and
  None), or every icon the library emits.
- **As:**
  - **SVG,** as `isolex render` draws it: on a light ground with the
    palette block's light colors over isolex's own; on a dark ground
    with its dark colors over the studio's dark defaults.
  - **PNG,** drawn from that SVG by the window at each size chosen (16,
    24, 32, 48, 64, 128, 256), its longer side the size, as Preview
    fits an icon.
  - **The IR,** as `isolex compile --ir` writes it, of the icons chosen.
  - **The library as a zip:** every file as it is open, saved or not,
    which *New library…* starts from.
- **Grounds,** for SVG and PNG: light, dark or both.
- **Names:** `x.svg`, `x@dark.svg`, `x@16.png`, `x@16-dark.png`; an
  icon's name never holds `@`, so none meet.
- **Where:** one file is saved where the person picks; several go into
  a folder they pick. A library that does not compile is not exported,
  and the dialog says so.

## Accessibility

- Every tool, tab, panel and control is reachable by keyboard, with a
  visible focus.
- Tool buttons, swatches and icon-only buttons have labels; the hint
  line and the compile state are live regions.
- Colors that must be told apart differ in lightness; text meets 4.5:1
  in both themes.
- Playwright with axe runs over the interface in both themes.

## Where each part is built

| Part | Issue |
|---|---|
| The frame, themes, canvas, pan and zoom, Source | #67 |
| Selection and the Parts panel | #68 |
| Select, Box, Tile, snapping, the decision popovers | #69 |
| Flat tools, pen, node, the other 3-D tools, repeats | #83 |
| Design and Preview, Undo History, Problems | #70 |
| Tree | #71 |
| Fonts and the text tool | #84 |
| The welcome panel, Library and Lexicon Properties, the out-of-sync strip | #72 |
| Export, and a new library from an exported zip | #112 |
| The Tree's focus view | #113 |
| Align and Distribute, and selecting several | #114 |
| Transform, and the turn handles | #115 |
| Insert and Badge | #116 |
| Measure, Dropper and Find | #117 |
| Alignment guides and advanced snapping | #118 |
