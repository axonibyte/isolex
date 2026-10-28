<!--
  The studio's window (docs/studio-interface.md, Layout): the top bar, the
  tool rail, the Parts panel, the canvas, the Design, Source, Tree and
  Preview tabs, and the status bar; or, with no library open, the welcome.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { Backend } from './lib/backend';
  import { Studio } from './lib/state.svelte';
  import { paletteCss } from './lib/isolex/palette';
  import { DEFAULT_PALETTE, canvasTheme, setCanvasTheme, setTheme, theme, type CanvasTheme, type Theme } from './lib/theme';
  import Canvas from './components/Canvas.svelte';
  import SourceTab from './components/SourceTab.svelte';
  import IsoIcon from './components/IsoIcon.svelte';

  interface Props {
    backend: Backend;
    /** Offered on the welcome, where there is a sample to open. */
    sample?: boolean;
  }

  let { backend, sample = false }: Props = $props();

  // the backend is the window's for its whole life
  // svelte-ignore state_referenced_locally
  const studio = new Studio(backend);
  let windowTheme = $state<Theme>(theme());
  let canvas = $state<CanvasTheme>(canvasTheme());
  let tab = $state<'design' | 'source' | 'tree' | 'preview'>('source');
  let pointer = $state('');
  let versions = $state<{ studio: string; isolex: string } | null>(null);
  let canvasView: ReturnType<typeof Canvas> | undefined = $state();

  onMount(() => {
    setTheme(windowTheme);
    setCanvasTheme(canvas);
    let stop: (() => void) | undefined;
    let gone = false;
    void studio.start().then((s) => (gone ? s() : (stop = s)));
    void backend.about().then((v) => (versions = v));
    return () => {
      gone = true;
      stop?.();
    };
  });

  const view = $derived(studio.view);
  const library = $derived(view.library);
  const icon = $derived(studio.icon ? (view.icons[studio.icon] ?? null) : null);
  const def = $derived(studio.definition);
  const errors = $derived(view.compiled?.errors.length ?? 0);
  const warnings = $derived(view.compiled?.warnings.length ?? 0);

  /** The palette as the canvas paints it: the library's, or isolex's own. */
  const paletteStyle = $derived.by(() => {
    const p = view.palette ?? DEFAULT_PALETTE;
    return paletteCss(Object.entries(p).map(([token, c]) => ({ token, light: c.light, dark: c.dark })));
  });

  /** The tools, as the spec lists them; those not built yet are shown
      disabled, so the rail does not change shape as they arrive. */
  const TOOLS = [
    { id: 'select', key: 'V', label: 'Select and move', d: 'M5 3v13l3.5-3.5 2.6 5 2-1-2.6-5H16z', ready: true },
    { id: 'node', key: 'A', label: 'Node', d: 'M4 15C6 6 14 6 16 15M2.5 13.5h3v3h-3zM14.5 13.5h3v3h-3zM8.5 4.5h3v3h-3z', ready: false },
    {
      id: 'hand',
      key: 'H',
      label: 'Pan',
      d: 'M7 10V5.5a1.3 1.3 0 0 1 2.6 0V9m0-4.5V4a1.3 1.3 0 0 1 2.6 0v5m0-3.5a1.3 1.3 0 0 1 2.6 0V11m0-3a1.3 1.3 0 0 1 2.6 0v4a6 6 0 0 1-6 6h-.5a6 6 0 0 1-4.6-2.2L3.5 12a1.3 1.3 0 0 1 2-1.7L7 12',
      ready: true,
    },
    null,
    { id: 'box', key: 'B', label: 'Box', d: 'M10 2 17 6v8l-7 4-7-4V6zM3 6l7 4 7-4M10 10v8', ready: false },
    { id: 'tile', key: 'T', label: 'Tile', d: 'M10 5 18 10 10 15 2 10z', ready: false },
    { id: 'cyl', key: 'C', label: 'Cylinder', d: 'M4 6a6 2.5 0 1 0 12 0a6 2.5 0 1 0-12 0M4 6v8a6 2.5 0 0 0 12 0V6', ready: false },
    { id: 'face', key: 'F', label: 'Face rect', d: 'M3 6l7 4v8l-7-4zM5 9.5l3 1.7v3.5l-3-1.7z', ready: false },
    null,
    { id: 'line', key: 'L', label: 'Line', d: 'M4 16 16 4', ready: false },
    { id: 'rect', key: 'R', label: 'Rectangle', d: 'M3 5h14v10H3z', ready: false },
    { id: 'ellipse', key: 'E', label: 'Ellipse', d: 'M3 10a7 5 0 1 0 14 0a7 5 0 1 0-14 0', ready: false },
    { id: 'poly', key: 'G', label: 'Polygon', d: 'M10 3 17 8 14 17H6L3 8z', ready: false },
    { id: 'pen', key: 'P', label: 'Pen', d: 'M4 16 12 4l4 4L6 18zM12 4l2-2 4 4-2 2', ready: false },
  ];
  let tool = $state('select');

  const TABS = [
    ['design', 'Design'],
    ['source', 'Source'],
    ['tree', 'Tree'],
    ['preview', 'Preview'],
  ] as const;

  async function openFolder() {
    const path = await backend.pickFolder();
    if (path) await studio.open(path);
  }

  function onkeydown(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement;
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    if (mod && key === 's') {
      e.preventDefault();
      if (library) void studio.save();
      return;
    }
    // in a text field, its own undo and keys apply
    if (typing || !library) return;
    if (mod && key === 'z') {
      e.preventDefault();
      void (e.shiftKey ? studio.redo() : studio.undo());
    } else if (mod && key === 'y') {
      e.preventDefault();
      void studio.redo();
    } else if (!mod && !e.altKey && e.key === '0') {
      canvasView?.fitIcon();
    } else if (!mod && !e.altKey) {
      const t = TOOLS.find((t) => t?.key.toLowerCase() === key);
      if (t?.ready) tool = t.id;
    }
  }

  function chooseTheme(t: Theme) {
    windowTheme = t;
    setTheme(t);
  }
  function chooseCanvas(t: CanvasTheme) {
    canvas = t;
    setCanvasTheme(t);
  }
</script>

<svelte:window {onkeydown} />
<svelte:head>{@html `<style>${paletteStyle}</style>`}</svelte:head>

{#if !library}
  <main class="welcome">
    <div class="card">
      <h1>isolex studio</h1>
      <p>Draw and manage a library of isolex icons: a folder of <code>.ilx</code> files, edited as one thing.</p>
      <button class="primary" onclick={openFolder}>Open library folder…</button>
      {#if sample}<button onclick={() => studio.open('sample')}>Open the sample library</button>{/if}
    </div>
  </main>
{:else}
  <div class="app">
    <header class="top">
      <h1 class="brand">isolex studio</h1>
      <span class="what">
        <span class="kind">{def ? def.kind.toLowerCase() : 'icon'}</span>
        <span class="name">{studio.icon ?? '—'}</span>
      </span>
      <button class="icon-btn" onclick={() => studio.undo()} disabled={!library.canUndo} aria-label="Undo" title="Undo (Ctrl+Z)"
        ><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M7 5 3 9l4 4" /><path d="M3 9h9a5 5 0 0 1 0 10h-2" /></svg></button
      >
      <button class="icon-btn" onclick={() => studio.redo()} disabled={!library.canRedo} aria-label="Redo" title="Redo (Ctrl+Shift+Z)"
        ><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m13 5 4 4-4 4" /><path d="M17 9H8a5 5 0 0 0 0 10h2" /></svg></button
      >
      <span class="spacer"></span>
      <div class="seg" role="group" aria-label="Canvas">
        <button aria-pressed={canvas === 'light'} onclick={() => chooseCanvas('light')}>Light canvas</button>
        <button aria-pressed={canvas === 'dark'} onclick={() => chooseCanvas('dark')}>Dark canvas</button>
      </div>
      <div class="seg" role="group" aria-label="Window">
        <button aria-pressed={windowTheme === 'dark'} onclick={() => chooseTheme('dark')}>Dark</button>
        <button aria-pressed={windowTheme === 'light'} onclick={() => chooseTheme('light')}>Light</button>
        <button aria-pressed={windowTheme === 'system'} onclick={() => chooseTheme('system')}>System</button>
      </div>
    </header>

    <div class="main">
      <nav class="rail" aria-label="Tools">
        {#each TOOLS as t, i (i)}
          {#if t}
            <button
              class="tool"
              aria-pressed={tool === t.id}
              disabled={!t.ready}
              aria-label={t.label}
              title={t.ready ? `${t.label} (${t.key})` : `${t.label} (${t.key}): comes with the drawing tools`}
              onclick={() => (tool = t.id)}
              ><svg viewBox="0 0 20 20" aria-hidden="true"><path d={t.d} /></svg><kbd aria-hidden="true">{t.key}</kbd></button
            >
          {:else}
            <span class="sep"></span>
          {/if}
        {/each}
      </nav>

      <aside class="side" aria-label="Parts">
        <h2>Parts</h2>
        <p class="empty">The open definition’s parts and shapes appear here with selection.</p>
      </aside>

      <main class="stage {canvas === 'dark' ? 'iso-dark' : 'iso-light'}" aria-label="Canvas area">
        <Canvas bind:this={canvasView} {icon} onpointer={(p) => (pointer = p)} />
        {#if !icon}
          <p class="note">{view.compiled ? (studio.icon ? `${studio.icon} does not compile.` : 'This library emits no icon.') : 'Compiling…'}</p>
        {/if}
        <div class="hint" role="status">{studio.message || 'Scroll to zoom · Ctrl+scroll pans north–south · Shift+scroll east–west · 0 fits'}</div>
      </main>

      <aside class="side right" aria-label="Inspector">
        <div class="tabs" role="tablist">
          {#each TABS as [id, label] (id)}
            <button role="tab" id="tab-{id}" aria-controls="pane" aria-selected={tab === id} onclick={() => (tab = id)}>{label}</button>
          {/each}
        </div>
        <div class="pane" id="pane" role="tabpanel" aria-labelledby="tab-{tab}">
          {#if tab === 'source'}
            <SourceTab {studio} />
          {:else if tab === 'tree'}
            <ul class="icons {canvas === 'dark' ? 'iso-dark' : 'iso-light'}" aria-label="Icons">
              {#each view.compiled?.icons ?? [] as name (name)}
                <li>
                  <button aria-current={name === studio.icon} onclick={() => studio.openIcon(name)}>
                    <span class="thumb">{#if view.icons[name]}<IsoIcon icon={view.icons[name]} height={36} />{/if}</span>
                    <span class="iname">{name}</span>
                    {#if view.broken[name]}<span class="broken" title={view.broken[name].map((d) => d.message).join('\n')}>does not compile</span>{/if}
                  </button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="empty">{tab === 'design' ? 'The selection’s numbers and paint appear here with selection.' : 'The open icon at real sizes appears here.'}</p>
          {/if}
        </div>
      </aside>
    </div>

    {#if view.outOfSync.length}
      <div class="sync" role="alert">
        <span>The library changed on disk ({view.outOfSync.join(', ')}), and you have unsaved changes.</span>
        <button onclick={() => backend.reload()}>Reload from disk</button>
        <button onclick={() => studio.save()}>Save over it</button>
      </div>
    {/if}

    <footer class="status">
      <span>
        {#if !view.compiled}<span class="dot"></span>Compiling…
        {:else if errors}<span class="dot bad"></span>{errors} error{errors === 1 ? '' : 's'}
        {:else}<span class="dot ok"></span>Compiled revision {view.compiled.revision}{/if}
        {#if warnings}<span class="warn"> · {warnings} warning{warnings === 1 ? '' : 's'}</span>{/if}
      </span>
      <span class="num">{pointer}</span>
      <span>{library.modified ? '● unsaved changes' : 'saved'}</span>
      <span class="spacer"></span>
      <span>{versions ? `studio ${versions.studio} · isolex ${versions.isolex}` : ''}</span>
    </footer>
  </div>
{/if}

<style>
  .welcome {
    height: 100%;
    display: grid;
    place-items: center;
    padding: 16px;
    box-sizing: border-box;
  }
  .card {
    max-width: 420px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 28px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
  }
  h1 {
    margin: 0;
    font-size: 20px;
  }
  .card p {
    margin: 0 0 8px;
    color: var(--muted);
  }
  .card button {
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--line);
    background: var(--raised);
    cursor: pointer;
  }
  .card button.primary {
    background: var(--accent);
    color: var(--accent-fg);
    border-color: var(--accent);
  }
  .app {
    height: 100%;
    display: grid;
    grid-template-rows: 48px minmax(0, 1fr) auto 30px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    min-width: 0;
  }
  .brand {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .what {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .kind {
    color: var(--muted);
  }
  .name {
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 4px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--raised);
  }
  .spacer {
    flex: 1;
  }
  .icon-btn {
    display: inline-grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 7px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--raised);
    color: var(--fg);
  }
  .icon-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .icon-btn svg,
  .tool svg {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .seg {
    display: inline-flex;
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 2px;
    gap: 2px;
  }
  .seg button {
    background: transparent;
    border: 0;
    border-radius: 5px;
    padding: 4px 9px;
    color: var(--muted);
    white-space: nowrap;
    cursor: pointer;
  }
  .seg button[aria-pressed='true'] {
    background: var(--panel);
    color: var(--fg);
    box-shadow: 0 0 0 1px var(--line);
  }
  .main {
    display: grid;
    grid-template-columns: 52px 220px minmax(0, 1fr) 360px;
    min-height: 0;
  }
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 0;
    background: var(--panel);
    border-right: 1px solid var(--line);
  }
  .sep {
    width: 24px;
    height: 1px;
    background: var(--line);
    margin: 6px 0;
  }
  .tool {
    position: relative;
    width: 38px;
    height: 38px;
    display: grid;
    place-items: center;
    border-radius: 9px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .tool:hover:not(:disabled) {
    background: var(--raised);
    color: var(--fg);
  }
  .tool[aria-pressed='true'] {
    background: var(--accent);
    color: var(--accent-fg);
  }
  .tool:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .tool kbd {
    position: absolute;
    right: 3px;
    bottom: 1px;
    font: 9px var(--font-mono);
    opacity: 0.75;
  }
  .side {
    background: var(--panel);
    border-right: 1px solid var(--line);
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .side h2 {
    margin: 0;
    padding: 10px 12px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  .right {
    border-right: 0;
    border-left: 1px solid var(--line);
  }
  .empty {
    margin: 4px 12px;
    color: var(--muted);
  }
  .stage {
    position: relative;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .note {
    position: absolute;
    left: 50%;
    top: 40%;
    transform: translateX(-50%);
    color: var(--frame);
  }
  .hint {
    position: absolute;
    left: 50%;
    bottom: 14px;
    transform: translateX(-50%);
    background: var(--panel);
    color: var(--fg);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 6px 14px;
    font-size: 12px;
    white-space: nowrap;
    max-width: calc(100% - 32px);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 8px 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    background: transparent;
    border: 0;
    border-bottom: 2px solid transparent;
    padding: 6px 10px;
    color: var(--muted);
    cursor: pointer;
  }
  .tabs button[aria-selected='true'] {
    color: var(--fg);
    border-bottom-color: var(--accent);
  }
  .pane {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .icons {
    list-style: none;
    margin: 0;
    padding: 8px;
  }
  .icons button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 4px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .icons button:hover {
    background: var(--raised);
  }
  .icons button[aria-current='true'] {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .thumb {
    width: 52px;
    height: 42px;
    display: grid;
    place-items: center;
    background: var(--canvas);
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .iname {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .broken {
    margin-left: auto;
    color: var(--danger);
    font-size: 11px;
  }
  .sync {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: color-mix(in srgb, var(--warn) 20%, var(--panel));
    border-top: 1px solid var(--line);
  }
  .sync span {
    flex: 1;
  }
  .sync button {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    cursor: pointer;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 12px;
    background: var(--panel);
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
  }
  .num {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .warn {
    color: var(--warn);
  }
  .dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    margin-right: 6px;
    background: var(--muted);
  }
  .dot.ok {
    background: var(--ok);
  }
  .dot.bad {
    background: var(--danger);
  }
  @media (max-width: 900px) {
    .main {
      grid-template-columns: 52px minmax(0, 1fr);
      grid-template-rows: 60vh auto auto;
    }
    .side {
      grid-column: 1 / -1;
      border-right: 0;
      border-top: 1px solid var(--line);
    }
    .right {
      border-left: 0;
    }
  }
</style>
