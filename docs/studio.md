# isolex studio

isolex studio is a desktop app to draw and manage an isolex library: a
folder of `.ilx` files, edited as one thing, with every icon in it
redrawn live as you work. It is being built (#44); this guide grows
with it. Its interface is specified in
[studio-interface.md](studio-interface.md).

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
npm run check        # svelte-check
npm test             # vitest
npm run e2e          # Playwright with axe (once: npx playwright install chromium)
npm run tauri build  # an installer for this platform
```

`npm run dev` serves the interface alone in a browser, over the
compiler's sample library (`tests/fixtures/sample.ilx`) instead of a
folder, with the studio's core built as a WebAssembly module
(`studio-core/wasm`, which `npm run core` builds and `dev`, `test` and
`e2e` build first; it needs the `wasm32-unknown-unknown` target). It
edits, undoes and compiles as the app does, in memory: saving keeps the
library as it is. vitest and the browser tests run against it.

`cargo test --manifest-path studio/src-tauri/Cargo.toml` runs the
shell's own tests. The same checks run on every push to the GitHub
mirror (`.github/workflows/studio.yml`).

The app's icon is drawn in isolex: `studio/src-tauri/icons/icon.ilx`,
rendered to `icon.svg`, from which `npm run tauri -- icon
src-tauri/icons/icon.svg -o src-tauri/icons` makes the rest.
