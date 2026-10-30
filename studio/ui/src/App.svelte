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
  import PartsPanel from './components/PartsPanel.svelte';
  import { marks, pick, source, within } from './lib/select';
  import { Tools } from './lib/tools.svelte';
  import { TOOLS, chord, fits, type Tool } from './lib/keymap';
  import { Shortcuts } from './lib/shortcuts.svelte';
  import PreferencesDialog from './components/PreferencesDialog.svelte';
  import { FLOAT_SIZE, PANELS, dock, docked, float, floating, moved, readLayout, resized, within as inside, type Layout, type Panel } from './lib/layout';
  import DesignTab from './components/DesignTab.svelte';
  import ArrangeTab from './components/ArrangeTab.svelte';
  import PlacePicker from './components/PlacePicker.svelte';
  import FindDialog from './components/FindDialog.svelte';
  import PreviewTab from './components/PreviewTab.svelte';
  import TreeTab from './components/TreeTab.svelte';
  import LibraryDialog from './components/LibraryDialog.svelte';
  import LexiconDialog from './components/LexiconDialog.svelte';
  import NewLibrary from './components/NewLibrary.svelte';
  import ExportDialog from './components/ExportDialog.svelte';
  import { forget, folderName, recent, remember, setShowAtStartup, showAtStartup, when, type Recent } from './lib/recent';
  import { inShell } from './lib/backend';
  import HistoryMenu from './components/HistoryMenu.svelte';
  import ProblemsMenu from './components/ProblemsMenu.svelte';
  import SnapControl from './components/SnapControl.svelte';

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
  let tab = $state<Panel>('source');
  let pointer = $state('');
  let versions = $state<{ studio: string; isolex: string } | null>(null);
  let canvasView: ReturnType<typeof Canvas> | undefined = $state();

  onMount(() => {
    setTheme(windowTheme);
    setCanvasTheme(canvas);
    let stop: (() => void) | undefined;
    let gone = false;
    void studio.start().then(async (s) => {
      if (gone) return s();
      stop = s;
      // a window opened on the library open joins it, at the icon it was
      // opened on (#119)
      if (await studio.attach(await backend.openedOn())) return;
      // a folder the app was started with opens; else, with the welcome
      // panel off, the library opened last
      void backend.startup().then((folder) => {
        if (folder) void openLibrary(folder);
        else if (inShell() && !startup && recents[0]) void openLibrary(recents[0].path);
      });
    });
    void backend.about().then((v) => (versions = v));
    return () => {
      gone = true;
      stop?.();
    };
  });

  const view = $derived(studio.view);
  const library = $derived(view.library);
  /** The styles of the font open, to set its glyphs in. */
  const fontStyles = $derived(studio.font ? (view.tree?.fonts.find((f) => f.name === studio.editing)?.children ?? []).map((r) => r.variant ?? '').filter(Boolean) : []);
  /** What the canvas draws: the icon open, or a font's sketch. */
  const icon = $derived(studio.drawing?.ir ?? null);
  /** What the canvas is called: the icon's name, or the font's and its
      glyph's. */
  const canvasName = $derived(studio.font ? `${studio.editing}${studio.glyph ? `, glyph ${studio.glyph}` : ''}` : (icon?.name ?? null));
  /** A glyph's guides: its baseline, ascent, descent, origin and
      advance, on the canvas. */
  const guides = $derived.by(() => {
    const g = studio.sketch?.glyph;
    if (!g) return [];
    const { ascent, descent, em } = g.metrics;
    const pad = em * 0.25;
    const at = (x: number, y: number): [number, number] => [g.map[0] * x + g.map[2] * y + g.map[6], g.map[1] * x + g.map[3] * y + g.map[7]];
    const across = (y: number, label: string) => ({ a: at(-pad, y), b: at(g.advance + pad, y), label });
    const down = (x: number, label: string) => ({ a: at(x, -descent), b: at(x, ascent), label });
    return [across(0, 'baseline'), across(ascent, 'ascent'), across(-descent, 'descent'), down(0, ''), down(tools.advanceAt ?? g.advance, 'advance')];
  });
  const def = $derived(studio.definition);
  const flat = $derived(!!icon?.canvas);
  const sources = $derived(studio.drawing?.sources ?? []);
  /** The shapes on the canvas of every node selected. */
  const marked = $derived(new Set(studio.chosen.flatMap((n) => [...marks(sources, studio.editingNode, n)])));
  /** The parts hidden in the editor, by node. */
  const partNodes = (names: Set<string>) => studio.outline?.parts.filter((p) => names.has(p.name)).map((p) => p.node) ?? [];
  const hidden = $derived.by(() => {
    const parts = partNodes(studio.hidden);
    const out = new Set<number>();
    if (!parts.length) return out;
    sources.forEach((o, i) => {
      const n = pick(o, studio.editingNode);
      if (n && parts.some((p) => within(n, p))) out.add(i);
    });
    return out;
  });
  /** What the shape under the pointer is, when it is drawn from elsewhere. */
  let hover = $state('');

  /** Where a shape drawn from elsewhere comes from, as the hint says it. */
  function drawnBy(i: number): string {
    const o = sources[i];
    if (!o || pick(o, studio.editingNode)) return '';
    return `From ${source(o, view.graph)}${o.part ? `, part ${o.part}` : ''}: open it from Parts to edit it`;
  }

  /** A click on the canvas: the shape's node selected, if it is the edited
      definition's and its part is not locked. */
  /** A click on a shape, or on nothing; with Shift, the shape's node
      added to what is selected, or taken out. */
  function onpick(i: number | null, shift = false) {
    const o = i === null ? null : sources[i];
    // a font's glyphs set out: a click opens the glyph
    if (studio.font && !studio.glyph) {
      const glyph = o && studio.outline?.parts.find((p) => within(o.node, p.node));
      if (glyph) studio.openGlyph(glyph.name);
      return;
    }
    const n = o ? pick(o, studio.editingNode) : null;
    if (o && !n) {
      studio.message = drawnBy(i!);
      return;
    }
    const locked = n && studio.outline?.parts.find((p) => studio.locked.has(p.name) && within(n, p.node));
    if (locked) {
      studio.message = `Part ${locked.name} is locked`;
      return;
    }
    if (shift) {
      if (n) studio.add(n);
      return;
    }
    studio.select(n);
  }
  /** The icon as drawn now: as previewed with other params, or as it is. */
  const shown = $derived(studio.previewing?.drawing.ir ?? icon);
  // a place opened from the problems list is shown in the Source tab
  $effect(() => {
    if (studio.place) tab = 'source';
  });

  /** The palette as the canvas paints it: the library's, or isolex's own. */
  const paletteStyle = $derived.by(() => {
    const p = view.palette ?? DEFAULT_PALETTE;
    return paletteCss(Object.entries(p).map(([token, c]) => ({ token, light: c.light, dark: c.dark })));
  });

  /** The rail: those not built yet are shown disabled, so it does not
      change shape as they arrive, and 3-D tools in a flat icon too. */
  const usable = (t: Tool) => t.ready && fits(t, flat);
  // a 3-D tool has nothing to draw in a flat icon
  $effect(() => {
    const t = TOOLS.find((t) => t?.id === tools.tool);
    if (t && !fits(t, flat)) choose('select');
  });
  const tools = new Tools(studio);
  /** What each tool does, for the hint line. */
  const HINTS: Record<string, string> = {
    select: 'Click to select, again to rotate and scale · drag to move · Alt+drag lifts · arrows nudge, PageUp and PageDown raise and lower',
    node: 'Drag a point or a handle · Alt keeps a smooth point’s handles apart · Alt+click switches corner and smooth · double-click a side adds a point · Delete removes one',
    hand: 'Drag to pan',
    measure: 'Drag to measure: in tiles on the floor, or on a top face started on; in units flat · Esc clears',
    dropper: 'Click a shape to pick its fill, Shift+click its stroke: set on what is selected, or on what is drawn next',
    insert: 'Choose a fragment, then click where it goes, or drag it in · it snaps to the step',
    badge: 'Choose an icon, then click near the corner it goes in, or drag it in',
    box: 'Drag a footprint, move up for the height, and click · started on a top face, it stands on it',
    tile: 'Drag a footprint on the floor',
    cylinder: 'Drag from the center out to the radius, move up for the height, and click',
    facerect: 'Drag a rectangle on the front or the right side of a box',
    shadow: 'Drag from the center out to an ellipse on the floor',
    seg: 'Click one end on a floor, then the other',
    poly3d: 'Click each point on a floor; click the first, or press Enter, to close',
    line: 'Drag · Shift keeps 15° steps',
    rect: 'Drag · Shift for a square',
    ellipse: 'Drag · Shift for a circle',
    arc: 'Drag the radius from the center, then move round for the sweep, and click',
    poly: 'Click each corner; click the first, or press Enter, to close',
    pen: 'Click for a corner, drag for a smooth point; click the first point to close, Enter to leave it open',
  };

  /** The keys, as this person has bound them (#119). */
  const shortcuts = new Shortcuts();

  // --- the panels, docked or floating (#119) -----------------------------------------

  const LAYOUT_KEY = 'layout';
  function storedLayout(): string | null {
    try {
      return localStorage.getItem(LAYOUT_KEY);
    } catch {
      return null;
    }
  }
  let layout = $state<Layout>(readLayout(storedLayout()));
  /** The layout as it is now, kept for next time. */
  function keep(l: Layout = layout) {
    layout = l;
    try {
      localStorage.setItem(LAYOUT_KEY, JSON.stringify(l));
    } catch {
      // kept for this window only
    }
  }
  let innerWidth = $state(1024);
  let innerHeight = $state(768);
  const size = $derived({ w: innerWidth, h: innerHeight });
  const dockedPanels = $derived(docked(layout));
  const labelOf = (id: Panel) => PANELS.find(([p]) => p === id)![1];
  // the tab shown is one docked
  $effect(() => {
    if (layout.floating[tab] && dockedPanels.length) tab = dockedPanels[0]!;
  });

  /** `id` floating: at `at`, or beside the right side, each after the
      first a little lower. */
  function floatPanel(id: Panel, at?: { x: number; y: number }) {
    const n = floating(layout).length;
    keep(float(layout, id, at ?? { x: size.w - FLOAT_SIZE.w - 380 + n * 24, y: 64 + n * 24 }, size));
  }
  function dockPanel(id: Panel) {
    keep(dock(layout, id));
    tab = id;
  }
  /** A panel shown: its tab, if it is docked. */
  function showPanel(id: Panel) {
    if (!layout.floating[id]) tab = id;
  }

  let strip: HTMLElement | undefined = $state();
  /** A tab pressed, which dragged far enough floats; a floating panel
      dragged by its title bar, which dropped on the tabs docks. */
  let tearing: { id: Panel; x: number; y: number } | null = null;
  let dragging: { id: Panel; x: number; y: number } | null = null;
  const TEAR = 48;

  function grab(e: PointerEvent, id: Panel) {
    if (e.button !== 0 || (e.target as Element).closest('button')) return;
    e.preventDefault();
    dragging = { id, x: e.clientX, y: e.clientY };
  }
  function onpointermove(e: PointerEvent) {
    if (tearing && Math.hypot(e.clientX - tearing.x, e.clientY - tearing.y) > TEAR) {
      const id = tearing.id;
      tearing = null;
      floatPanel(id, { x: e.clientX - 60, y: e.clientY - 14 });
      dragging = { id, x: e.clientX, y: e.clientY };
      return;
    }
    if (dragging) {
      layout = moved(layout, dragging.id, { x: e.clientX - dragging.x, y: e.clientY - dragging.y }, size);
      dragging = { ...dragging, x: e.clientX, y: e.clientY };
    }
  }
  function onpointerup(e: PointerEvent) {
    tearing = null;
    if (!dragging) return;
    const id = dragging.id;
    dragging = null;
    const r = strip?.getBoundingClientRect();
    if (r && r.width && e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) dockPanel(id);
    else keep();
  }

  /** A floating panel resized by its corner: its size kept. */
  function sized(node: HTMLElement, id: Panel) {
    if (typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(() => {
      const b = layout.floating[id];
      const w = Math.round(node.offsetWidth);
      const h = Math.round(node.offsetHeight);
      if (b && w && h && (w !== b.w || h !== b.h)) keep(resized(layout, id, { w, h }, size));
    });
    watch.observe(node);
    return { destroy: () => watch.disconnect() };
  }

  async function openFolder() {
    const path = await backend.pickFolder();
    if (path) await openLibrary(path);
  }

  /** A library opened, or why not: a recent one that has gone is
      forgotten. */
  async function openLibrary(path: string) {
    try {
      await studio.open(path);
    } catch (e) {
      recents = forget(path);
      opening = `${path} could not be opened: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  let recents = $state<Recent[]>(recent());
  let startup = $state(showAtStartup());
  let opening = $state('');
  let dialog = $state<'library' | 'lexicon' | 'new' | 'export' | 'find' | 'preferences' | null>(null);
  /** Reloading would discard unsaved changes: asked first. */
  let confirming = $state(false);

  // a library open in the app is remembered, with its definitions counted
  let remembered: string | null = null;
  $effect(() => {
    const root = library?.root;
    const count = view.graph?.definitions.length;
    if (!root || count === undefined || !inShell() || remembered === root) return;
    remembered = root;
    recents = remember({ path: root, name: folderName(root), definitions: count, opened: Date.now() });
  });

  /** A key pressed: the command it is bound to run (lib/keymap), or the
      tool's own. */
  function onkeydown(e: KeyboardEvent) {
    // a key a field took for itself is its own; Preferences hears keys
    // to bind
    if (e.defaultPrevented || dialog === 'preferences') return;
    const typing = e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement;
    const key = chord(e);
    const command = key ? shortcuts.find(key) : null;
    if (command?.anywhere) {
      e.preventDefault();
      run(command.id);
      return;
    }
    // in a text field, its own undo and keys apply
    if (typing || !library) return;
    if (e.key === 'Escape') {
      if (!tools.cancel()) studio.select(null);
    } else if (command) {
      e.preventDefault();
      run(command.id);
    } else if (tools.key({ key: e.key, shift: e.shiftKey, ctrl: e.ctrlKey || e.metaKey })) {
      e.preventDefault();
    }
  }

  /** The command `id`, as a key or a button runs it. */
  function run(id: string) {
    switch (id) {
      case 'open':
        void openFolder();
        break;
      case 'new':
        if (inShell()) dialog = 'new';
        break;
      case 'window':
        if (library && inShell()) void backend.newWindow(studio.icon);
        break;
      case 'save':
        if (library) void studio.save();
        break;
      case 'export':
        if (library) dialog = 'export';
        break;
      case 'preferences':
        dialog = 'preferences';
        break;
      // Find, across the library, as Inkscape opens it
      case 'find':
        if (library) dialog = 'find';
        break;
      // Align and Distribute, as Inkscape opens it
      case 'arrange':
        if (library) showPanel('arrange');
        break;
      case 'undo':
        void studio.undo();
        break;
      case 'redo':
        void studio.redo();
        break;
      case 'duplicate':
        tools.duplicateSelected();
        break;
      case 'fit':
        canvasView?.fitIcon();
        break;
      default: {
        const t = TOOLS.find((t) => t && `tool:${t.id}` === id);
        if (t && usable(t)) choose(t.id);
      }
    }
  }

  function choose(tool: string) {
    tools.cancel();
    tools.tool = tool;
    studio.message = '';
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

<svelte:window {onkeydown} {onpointermove} {onpointerup} bind:innerWidth bind:innerHeight />
<svelte:head>{@html `<style>${paletteStyle}</style>`}</svelte:head>

<!-- a panel's contents, docked in the right side or floating -->
{#snippet panel(id: Panel)}
  {#if id === 'source'}
    <SourceTab {studio} />
  {:else if id === 'design'}
    <DesignTab {studio} {tools} {icon} look={canvas === 'dark' ? 'iso-dark' : 'iso-light'} />
  {:else if id === 'arrange'}
    <ArrangeTab {studio} {tools} />
  {:else if id === 'preview'}
    <PreviewTab icon={shown} />
  {:else if id === 'tree'}
    <TreeTab {studio} look={canvas === 'dark' ? 'iso-dark' : 'iso-light'} />
  {/if}
{/snippet}

{#if !library}
  <main class="welcome">
    <div class="card">
      <h1>isolex studio</h1>
      <p>Draw and manage a library of isolex icons: a folder of <code>.ilx</code> files, edited as one thing.</p>
      <div class="actions">
        <button class="primary" onclick={openFolder} title={shortcuts.keys('open')[0]}>Open library folder…</button>
        <button onclick={() => (dialog = 'new')} disabled={!inShell()} title={inShell() ? shortcuts.keys('new')[0] : 'In the studio’s app'}>New library…</button>
      </div>
      {#if sample}<button onclick={() => studio.open('sample')}>Open the sample library</button>{/if}
      {#if opening}<p class="failed" role="alert">{opening}</p>{/if}
      {#if recents.length}
        <h2>Recent</h2>
        <ul class="recent" aria-label="Recent libraries">
          {#each recents as r (r.path)}
            <li>
              <button ondblclick={() => openLibrary(r.path)} onkeydown={(e) => e.key === 'Enter' && openLibrary(r.path)} title="Double-click to open">
                <span class="rname">{r.name}</span>
                <span class="rpath">{r.path}</span>
                <span class="rmeta">{r.definitions} definitions · {when(r.opened)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      <label class="startup"><input type="checkbox" checked={startup} onchange={(e) => setShowAtStartup((startup = e.currentTarget.checked))} /> Show this at startup</label>
    </div>
    {#if dialog === 'new'}<NewLibrary {studio} onclose={() => (dialog = null)} />{/if}
    {#if dialog === 'preferences'}<PreferencesDialog {shortcuts} onclose={() => (dialog = null)} />{/if}
  </main>
{:else}
  <div class="app">
    <header class="top">
      <h1 class="brand">isolex studio</h1>
      <span class="what">
        <span class="kind">{def ? def.kind.toLowerCase() : 'icon'}</span>
        <span class="name">{studio.editing ?? studio.icon ?? '—'}</span>
        {#if studio.icon && def && !def.emits.includes(studio.icon) && !studio.font}
          <span class="kind">as drawn in</span>
          <span class="name">{studio.icon}</span>
        {/if}
      </span>
      <button class="icon-btn" onclick={() => studio.undo()} disabled={!library.canUndo} aria-label="Undo" title={shortcuts.titled('Undo', 'undo')}
        ><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M7 5 3 9l4 4" /><path d="M3 9h9a5 5 0 0 1 0 10h-2" /></svg></button
      >
      <button class="icon-btn" onclick={() => studio.redo()} disabled={!library.canRedo} aria-label="Redo" title={shortcuts.titled('Redo', 'redo')}
        ><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m13 5 4 4-4 4" /><path d="M17 9H8a5 5 0 0 0 0 10h2" /></svg></button
      >
      <HistoryMenu {studio} />
      <button class="text-btn" onclick={() => (dialog = 'library')}>Library</button>
      <button class="text-btn" onclick={() => (dialog = 'lexicon')}>Lexicon</button>
      <button class="text-btn" onclick={() => (dialog = 'export')} title={shortcuts.keys('export')[0]}>Export…</button>
      <button class="text-btn" onclick={() => run('window')} disabled={!inShell()} title={inShell() ? shortcuts.titled('Another window on this library', 'window') : 'In the studio’s app'}>New window</button>
      <button class="text-btn" onclick={() => run('preferences')} title={shortcuts.titled('Preferences and keyboard shortcuts', 'preferences')}>Preferences</button>
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

    <div class="main" class:undocked={!dockedPanels.length}>
      <nav class="rail" aria-label="Tools">
        {#each TOOLS as t, i (i)}
          {#if t}
            {@const key = shortcuts.keys(`tool:${t.id}`)[0] ?? ''}
            {@const named = key ? `${t.label} (${key})` : t.label}
            <button
              class="tool"
              aria-pressed={tools.tool === t.id}
              disabled={!usable(t)}
              aria-label={t.label}
              title={!t.ready ? `${named}: not built yet` : usable(t) ? named : `${named}: draws in a 3-D icon`}
              onclick={() => choose(t.id)}
              ><svg viewBox="0 0 20 20" aria-hidden="true"><path d={t.d} /></svg><kbd aria-hidden="true"
                >{#if key.startsWith('Shift+')}<svg class="shift" viewBox="0 0 8 8"><path d="M4 1 7.5 4.5H5.5V7h-3V4.5H.5z" /></svg>{/if}{key.replace(/^Shift\+/, '')}</kbd
              ></button
            >
          {:else}
            <span class="sep"></span>
          {/if}
        {/each}
      </nav>

      <aside class="side" aria-label="Parts">
        <PartsPanel {studio} />
      </aside>

      <main class="stage {canvas === 'dark' ? 'iso-dark' : 'iso-light'}" aria-label="Canvas area">
        <Canvas
          bind:this={canvasView}
          icon={shown}
          label={canvasName}
          {guides}
          {tools}
          marked={studio.previewing ? undefined : marked}
          hidden={studio.previewing ? undefined : hidden}
          onpointer={(p) => (pointer = p)}
          {onpick}
          onhover={(i) => (hover = i === null ? '' : drawnBy(i))}
        />
        {#if studio.font && !studio.glyph && fontStyles.length}
          <div class="styles" role="group" aria-label="Styles shown">
            {#each fontStyles as s (s)}
              <button aria-pressed={studio.fontStyle.includes(s)} onclick={() => studio.toggleStyle(s)}>{s}</button>
            {/each}
          </div>
        {/if}
        {#if icon && (tools.tool === 'insert' || tools.tool === 'badge')}
          <PlacePicker {studio} {tools} kind={tools.tool} />
        {/if}
        {#if icon}
          <SnapControl {tools} />
        {/if}
        {#if !icon}
          <p class="note">{view.compiled ? (studio.icon ? `${studio.icon} does not compile.` : 'This library emits no icon.') : 'Compiling…'}</p>
        {/if}
        <div class="hint" role="status">{studio.previewing ? `Previewing ${Object.entries(studio.previewing.params).map(([k, v]) => `${k} = ${v}`).join(', ')}: nothing is written until Set as default` : hover || studio.message || HINTS[tools.tool] || 'Scroll to zoom · Ctrl+scroll pans north–south · Shift+scroll east–west · 0 fits'}</div>
      </main>

      {#if dockedPanels.length}
        <aside class="side right" aria-label="Inspector">
          <div class="tabbar" bind:this={strip}>
            <div class="tabs" role="tablist">
              {#each PANELS.filter(([id]) => !layout.floating[id]) as [id, label] (id)}
                <button
                  role="tab"
                  id="tab-{id}"
                  aria-controls="pane"
                  aria-selected={tab === id}
                  title="Drag it out to float it"
                  onclick={() => (tab = id)}
                  onpointerdown={(e) => e.button === 0 && (tearing = { id, x: e.clientX, y: e.clientY })}>{label}</button
                >
              {/each}
            </div>
            <button class="float-btn" aria-label="Float the {labelOf(tab)} panel" title="Float it over the window: or drag its tab out" onclick={() => floatPanel(tab)}
              ><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M8 4H4v12h12v-4M11 3h6v6M17 3l-7 7" /></svg></button
            >
          </div>
          <div class="pane" id="pane" role="tabpanel" aria-labelledby="tab-{tab}">
            {@render panel(tab)}
          </div>
        </aside>
      {/if}
    </div>

    {#each floating(layout) as id (id)}
      {@const b = inside(layout.floating[id]!, size)}
      <div class="float" role="dialog" aria-labelledby="float-{id}" style="left: {b.x}px; top: {b.y}px; width: {b.w}px; height: {b.h}px" use:sized={id}>
        <div class="float-head" role="presentation" onpointerdown={(e) => grab(e, id)}>
          <h2 id="float-{id}">{labelOf(id)}</h2>
          <button aria-label="Dock the {labelOf(id)} panel" title="Dock it on the right again: or drag it onto the tabs" onclick={() => dockPanel(id)}
            ><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M3 4h14v12H3zM12 4v12" /></svg></button
          >
        </div>
        <!-- it scrolls, so it can be scrolled from the keyboard -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="float-body" role="region" aria-label="{labelOf(id)} panel" tabindex="0">{@render panel(id)}</div>
      </div>
    {/each}

    {#if view.outOfSync.length}
      <div class="sync" role="alert">
        <span>The library changed on disk ({view.outOfSync.join(', ')}), and you have unsaved changes.</span>
        {#if confirming}
          <span class="confirm">Your unsaved changes are discarded for what is on disk.</span>
          <button
            onclick={() => {
              confirming = false;
              void backend.reload();
            }}>Discard and reload</button
          >
          <button onclick={() => (confirming = false)}>Keep mine</button>
        {:else}
          <button onclick={() => (confirming = true)}>Reload from disk</button>
          <button onclick={() => studio.save()}>Save over it</button>
        {/if}
      </div>
    {/if}
    {#if dialog === 'library'}<LibraryDialog {studio} onclose={() => (dialog = null)} />{/if}
    {#if dialog === 'lexicon'}<LexiconDialog {studio} onclose={() => (dialog = null)} />{/if}
    {#if dialog === 'export'}<ExportDialog {studio} onclose={() => (dialog = null)} />{/if}
    {#if dialog === 'find'}<FindDialog {studio} onclose={() => (dialog = null)} />{/if}
    {#if dialog === 'preferences'}<PreferencesDialog {shortcuts} onclose={() => (dialog = null)} />{/if}

    <footer class="status">
      <ProblemsMenu {studio} />
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
  .card .actions {
    display: flex;
    gap: 8px;
  }
  .card .actions button {
    flex: 1;
  }
  .card button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .card h2 {
    margin: 8px 0 0;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  .recent {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .recent button {
    width: 100%;
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 2px 12px;
    text-align: left;
  }
  .rname {
    font-weight: 600;
  }
  .rpath {
    grid-column: 1 / -1;
    color: var(--muted);
    font: 11px var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rmeta {
    grid-row: 1;
    grid-column: 2;
    color: var(--muted);
    font-size: 11px;
  }
  .startup {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: 12px;
  }
  .failed {
    margin: 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    font-size: 12px;
  }
  .text-btn {
    padding: 4px 10px;
    border-radius: 7px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }
  .text-btn:hover:not(:disabled) {
    color: var(--fg);
    background: var(--raised);
  }
  .text-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .confirm {
    font-weight: 600;
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
  /* every panel floating: the canvas takes the right side's room */
  .main.undocked {
    grid-template-columns: 52px 220px minmax(0, 1fr);
  }
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 8px 0;
    /* a short window scrolls the tools, never over the status bar */
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: none;
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
    flex: none;
    width: 36px;
    height: 34px;
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
    display: flex;
    align-items: center;
    gap: 1px;
    font: 9px var(--font-mono);
    opacity: 0.75;
  }
  .tool kbd .shift {
    width: 7px;
    height: 7px;
    fill: currentColor;
    stroke: none;
  }
  .side {
    background: var(--panel);
    border-right: 1px solid var(--line);
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .right {
    border-right: 0;
    border-left: 1px solid var(--line);
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
  .styles {
    position: absolute;
    top: 12px;
    right: 12px;
    display: flex;
    gap: 2px;
    padding: 4px;
    border-radius: 8px;
    background: var(--panel);
    border: 1px solid var(--line);
  }
  .styles button {
    padding: 3px 9px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }
  .styles button[aria-pressed='true'] {
    background: var(--raised);
    color: var(--fg);
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
  .tabbar {
    display: flex;
    align-items: flex-end;
    padding: 8px 6px 0 10px;
    border-bottom: 1px solid var(--line);
  }
  .tabs {
    flex: 1;
    display: flex;
    gap: 2px;
  }
  .float-btn,
  .float-head button {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    margin-bottom: 4px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .float-btn:hover,
  .float-head button:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .float-btn svg,
  .float-head svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .float {
    position: fixed;
    z-index: 10;
    /* its size as kept is the size it is drawn at, border and all */
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 12px 36px rgb(0 0 0 / 0.35);
    overflow: hidden;
    resize: both;
  }
  .float-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px 0 12px;
    border-bottom: 1px solid var(--line);
    cursor: grab;
    user-select: none;
    touch-action: none;
  }
  .float-head h2 {
    flex: 1;
    margin: 0 0 4px;
    font-size: 12px;
    font-weight: 600;
  }
  .float-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
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
