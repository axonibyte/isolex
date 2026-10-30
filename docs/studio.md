# isolex studio

isolex studio is a desktop app to draw and manage an isolex library: a
folder of `.ilx` files, edited as one thing, with every icon in it
redrawn live as you work. Every gesture is an edit of the source, which
the studio keeps as you wrote it: comments, order and formatting
outside what an edit changes are left alone. What you see is what the
isolex compiler draws. Its interface is specified in
[studio-interface.md](studio-interface.md); this is how to use it.

## Installing it

Each release of isolex on the forge carries the studio's installers
beside the compiler's binaries, with `SHA256SUMS-studio`:

| Platform | File |
|---|---|
| Linux | `isolex-studio-<version>-linux-x86_64.AppImage`, or `.deb` |
| Windows | `isolex-studio-<version>-windows-x86_64.msi`, or `.setup.exe` |
| macOS (Apple silicon and Intel) | `isolex-studio-<version>-macos-universal.dmg` |

They are not signed yet, so each system asks once whether to trust the
app:

- **Windows:** SmartScreen says it protected your PC; *More info*, then
  *Run anyway*.
- **macOS:** the first time, right-click (or Control-click) the app and
  choose *Open*, then *Open* again.
- **Linux:** make the AppImage executable (`chmod +x`) and run it, or
  install the deb (`sudo apt install ./isolex-studio-…deb`).

Check a download against `SHA256SUMS-studio` with `sha256sum -c`
(`Get-FileHash` on Windows).

## Opening a library

The welcome panel opens a library folder (Ctrl+O), makes a new one
(Ctrl+N: a lexicon with the standard tokens, a palette and one box, a
copy of another library, or a library exported as a zip), and lists the libraries you opened lately;
a double-click opens one. With *Show this at startup* off, the library
you opened last opens instead. `isolex-studio <folder>` opens that
folder as it starts.

The first icon the library emits opens on the canvas.

## The window

- **Top bar:** what is being edited, undo and redo, the undo history
  (every step, a click goes back or forward to it), the Library,
  Lexicon and Export dialogs, and the switches for a light or dark canvas and
  window.
- **Tools** down the left, each with its key.
- **Parts** (or a font's glyphs): the definition's parts and their
  shapes, the topmost first; a click selects (Shift or Ctrl+click
  selects several), *drawing here* marks the
  part new shapes go into, a double-click renames a part, a drag or
  PageUp, PageDown, Home and End reorders, and the eye and the lock
  hide and lock a part in the editor only. Parts
  the definition inherits are listed apart, each opening its own.
- **The canvas:** the wheel zooms at the pointer, Ctrl+wheel pans north
  and south, Shift+wheel east and west, Space+drag or the middle button
  pans, and 0 fits the icon. The snap step is at its top left, and a
  hint at its foot says what the tool does.
- **Tabs:** Design, Arrange, Source, Tree and Preview.
- **Status bar:** whether the library compiles (a click lists its
  problems, each opening its place in the Source tab), and where the
  pointer is.

## Drawing

Everything is drawn on the floor (z 0) or on the top of what is under
the pointer: a box started on another stands on it.

| Tool | Key | How |
|---|---|---|
| Select | V | Click to select (Shift+click to select several), drag to move, Alt+drag to lift; handles resize. Arrows nudge (Shift for a whole tile), PageUp and PageDown raise and lower, Ctrl+D duplicates, Delete deletes. |
| Node | A | Drag a point or a curve's handle of a path, a polygon or a line; Alt keeps a smooth point's handles apart, Alt+click switches corner and smooth, a double-click adds a point, Delete removes the one touched last. |
| Pan | H | Drag to pan. |
| Box | B | Drag a footprint, move up for the height, click. |
| Tile | T | Drag a footprint. |
| Cylinder | C | Drag from the center to the radius, move up for the height, click. |
| Face rect | F | Drag on the front or right side of a box. |
| Shadow | Shift+S | Drag from the center out. |
| Segment | Shift+L | Click one end, then the other. |
| 3-D polygon | Shift+G | Click each point; click the first, or Enter, to close. |
| Line, Rectangle, Ellipse | L, R, E | Drag; Shift keeps 15° steps, a square or a circle. |
| Arc | Shift+A | Drag the radius from the center, move round for the sweep, click. |
| Polygon | G | Click each corner; click the first, or Enter, to close. |
| Pen | P | Click for a corner, drag for a smooth point; click the first point to close, Enter to leave it open. |
| Text | X | Click to set words in the library's font. |

Esc stops what is being drawn. Flat shapes are drawn on one anchor at
the origin, so a flat icon's numbers are its canvas's.

**When an edit needs a decision,** the studio asks rather than
overwriting: a value written as an expression offers to edit its param
or replace it with a number; a part or a param inherited from a base
offers to change it there (naming the icons that follow) or to override
it here.

## Design, Arrange, Source, Tree, Preview

- **Design**, with a shape selected: its numbers as the source writes
  them (a number, or an expression typed as one), its paint from the
  lexicon's tokens and stroke widths only, its opacity, a rectangle's
  corners, a path open or closed, a text's words, font, style, size and
  alignment, a repeat along an axis (written as one `repeat`), and its
  order. With nothing selected: the icon's footprint or canvas, and its
  params, each with a slider that previews a value on the canvas and
  *Set as default*, which writes it.
- **Arrange** (Ctrl+Shift+A): Align and Distribute the shapes selected
  (Shift+click for several), along x, y and z, or a flat shape's x and
  y, relative to the first or last chosen, the selection, or the
  footprint (a flat icon's canvas); or spread three or more evenly, by
  middles or gaps. Below it, Transform: move by tiles; rotate, scale,
  skew or mirror on the screen, written as a `transform` round the
  selection; or turn a 3-D selection a quarter, written as a `turn`. A
  transform selected takes another step of its kind. Each is one undo
  step. On the canvas, click the selection again for its turn handles:
  drag a corner to rotate, or a side to scale.
- **Source:** the file, as you type it; it compiles as you type, and the
  selection follows the caret.
- **Tree:** every icon as a live thumbnail, nested by `extends`, its
  variants under it, and the fonts with their styles. An edit marks what
  it redrew; a broken icon keeps its last drawing and says why. Hovering
  says how an icon relates to what is open; a click opens it. *Focus*
  shows instead what is open in the middle, what it depends on to its
  left and what depends on it to its right, a column a step, to a depth
  of 1 to 3, with a line for each reference: extends, inserts, badges,
  opts into an axis, sets text in a font.
- **Preview:** the icon at 16 to 128 px, light and dark.

## Fonts

A font opens from the Tree: its glyphs set out as text sets them, in the
styles you switch on, its metrics and export in Design. A glyph opens
from its cell or the Glyphs panel on its em grid, y up, with its
baseline, ascent, descent and advance; it is drawn with the flat tools,
the pen and the node tool, its advance is a handle, and its kerning
pairs are set in Design by number or by dragging the right-hand glyph.
Every icon that sets text in the font is redrawn as you draw.

*Import…* in the Tree reads a TrueType, OpenType or WOFF font into the
library (choosing its characters, its em and further styles); a font
whose license forbids it is refused, saying why. *Export font…*, in a
font's Design, writes it as TrueType or a UFO.

## The library and its lexicon

- **Library** lists the folder's files and their definitions: a click
  opens one; F2, a double-click or *Rename* renames it everywhere it is
  named, or moves a file; drag a definition onto a file to move it
  there; new files and definitions are made here.
- **Lexicon** edits the `lexicon` and `palette` blocks: the tokens, each
  token's color on a light and a dark ground, the stroke widths, the
  reserved tokens and the limits.

## Exporting

*Export…* (Ctrl+Shift+E) writes the open icon, chosen icons or every
icon:

- as **SVG**, exactly as `isolex render` draws them, on a light ground,
  a dark one or both;
- as **PNG** at the sizes you tick (the longer side), on either ground;
- as **the IR**, exactly as `isolex compile --ir` writes it;
- or the whole library as **a zip**, unsaved changes included, which
  *New library…* can start from.

Files are named `x.svg`, `x@dark.svg`, `x@16.png` and `x@16-dark.png`.
One file is saved where you choose; several go into a folder you
choose. A library with errors is not exported until they are fixed.

## Saving, undo and the disk

Ctrl+S writes every changed file at once, never half; `●` in the status
bar means there are unsaved changes. One undo history (Ctrl+Z,
Ctrl+Shift+Z) covers the whole library, a gesture one step.

When the folder changes on disk (git, another editor), with nothing
unsaved the change is loaded and marked as any edit is; with unsaved
changes a strip offers to reload from disk (after asking, since your
changes would go) or to save over it. The studio's own saves never
raise it.

## Building it

The studio is two parts:

- **`studio-core/`,** what the studio does without a window: the
  library, its edits, saving, compiling and watching the folder. It is
  pure Rust, in the compiler's workspace, and `cargo test --workspace`
  tests it with everything else.
- **`studio/`,** the desktop shell (`src-tauri/`, Tauri 2) and its
  interface (`ui/`, Svelte 5, Vite and TypeScript). It is a workspace of
  its own, so building the compiler never needs a webview.

To work on the shell and interface you need Node 24 and npm, Rust, and
Tauri's system libraries for your platform (on Linux, webkit2gtk 4.1;
on Windows, the WebView2 runtime that Windows 10 and 11 ship).

```sh
cd studio
npm ci               # the interface's dependencies
npm run tauri dev    # the app, with the interface reloading as you edit
npm run check        # svelte-check, and the app test's types
npm test             # vitest
npm run e2e          # Playwright with axe (once: npx playwright install chromium)
npm run e2e:app      # the built app, end to end (needs tauri-driver; see below)
npm run tauri build  # an installer for this platform
```

`npm run dev` serves the interface alone in a browser, over the
compiler's sample library (`tests/fixtures/sample.ilx`; with
`?library=text`, the text fixture, which has fonts) instead of a
folder, with the studio's core built as a WebAssembly module
(`studio-core/wasm`, which `npm run core` builds and `dev`, `test` and
`e2e` build first; it needs the `wasm32-unknown-unknown` target). It
edits, undoes and compiles as the app does, its disk in memory.
vitest and the browser tests run against it.

`npm run e2e:app` builds the app (debug, unbundled) and drives it
through [tauri-driver](https://v2.tauri.app/develop/tests/webdriver/)
with WebdriverIO: it opens a copy of the sample library, draws a box and
a path, opens another icon from the Tree, saves, and `isolex check`s the
folder. It needs `cargo install tauri-driver --locked`, and on Linux
WebKitWebDriver (`webkit2gtk-driver`) and a display (`xvfb-run`), on
Windows an msedgedriver matching the installed WebView2 on the PATH;
macOS has no WebDriver for its webview.

`cargo test --manifest-path studio/src-tauri/Cargo.toml` runs the
shell's own tests. The same checks, and the app end to end on Linux and
Windows, run on every push to the GitHub mirror
(`.github/workflows/studio.yml`); the forge's CI reports what GitHub
says of the same commit, without failing on it.

**Releases.** A bare version tag (`0.6.0`) makes the forge's CI create
the release with the compiler's binaries.
`.github/workflows/studio-release.yml` builds the installers for the
same tag on GitHub, once the mirror carries it there, and leaves them
as one artifact with `SHA256SUMS-studio`. The forge's CI waits for that
run, downloads the artifact, checks it against the sums, and attaches
it to the release itself. Downloading an artifact needs a GitHub token,
so the forge holds the secret `GH_ARTIFACTS_TOKEN`: a fine-grained
token with read-only access to public repositories.

The app's icon is drawn in isolex: `studio/src-tauri/icons/icon.ilx`,
rendered to `icon.svg`, from which `npm run tauri -- icon
src-tauri/icons/icon.svg -o src-tauri/icons` makes the rest.
