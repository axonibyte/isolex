<!--
  The Tree tab (docs/studio-interface.md, Tree): every icon of the library
  as a live thumbnail, nested by `extends`, each icon's axis variants under
  it, and fonts in a section of their own (#71), each with a sample line
  set in it and its styles under it, and a font imported from there (#84). An icon an edit redrew
  gets a dot that fades; one it broke a warning mark, its last drawing
  kept and its errors on hover; one it did not change is not drawn again.
  A click opens that icon, or that variant, for editing; the one being
  edited is marked; hovering says how a row relates to what is open.
  Its focus view (#113) shows instead what is open, what it depends on
  and what depends on it, to a depth.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { TreeRow } from '../lib/backend';
  import { relation } from '../lib/tree';
  import IsoIcon from './IsoIcon.svelte';
  import ImportFont from './ImportFont.svelte';
  import { sampleSource } from '../lib/font';
  import type { IrIcon } from '../lib/isolex/ir';
  import { inShell } from '../lib/backend';
  import { DEPTHS, clampDepth, focusDepth, setFocusDepth, setTreeView, treeView, type TreeView } from '../lib/focus';
  import FocusView from './FocusView.svelte';

  interface Props {
    studio: Studio;
    /** The canvas's look, which the thumbnails are drawn in. */
    look: string;
  }

  let { studio, look }: Props = $props();

  const view = $derived(studio.view);
  let hovered = $state<TreeRow | null>(null);
  const said = $derived(hovered && view.graph ? relation(view.graph, hovered, studio.editing, studio.icon) : '');
  /** What the focus view's hovered node says. */
  let focusSaid = $state('');

  let mode = $state<TreeView>(treeView());
  let depth = $state(focusDepth());
  $effect(() => setTreeView(mode));
  $effect(() => setFocusDepth(depth));

  /** The row open: its definition edited, its icon (if it has one) drawn. */
  const open = (r: TreeRow) => r.definition === studio.editing && (!r.icon || r.icon === studio.icon);
  /** Each font's and style's sample line, as the font sets it now. */
  let samples = $state<Record<string, IrIcon>>({});
  $effect(() => {
    void view.library?.revision;
    const fonts = view.tree?.fonts ?? [];
    void (async () => {
      const next: Record<string, IrIcon> = {};
      for (const f of fonts) {
        const o = await studio.backend.outline(f.name);
        if (!o) continue;
        for (const r of [f, ...f.children]) {
          const d = await studio.backend.sketch(sampleSource(f.name, o, r.variant ? [r.variant] : []));
          if (d) next[r.name] = d.ir;
        }
      }
      samples = next;
    })();
  });

  let importing = $state(false);

  const kindNote = (r: TreeRow) => (r.kind === 'variant' ? `variant ${r.variant ?? ''}`.trim() : r.kind === 'icon' ? '' : r.kind);
</script>

<script module lang="ts">
  /** A thumbnail's height that fits its icon in the 40 by 32 box, by its
      longer side: a wide icon is no wider than the box. */
  export function thumbHeight(icon: IrIcon): number {
    const [, , w, h] = icon.viewbox;
    return Math.min(32, (40 * h) / (w || 1));
  }
</script>

{#snippet rows(list: TreeRow[], depth: number)}
  {#each list as r (r.kind + r.name)}
    {@const broken = r.icon ? view.broken[r.icon] : undefined}
    {@const times = r.icon ? (view.redrawn[r.icon] ?? 0) : 0}
    <li>
      <button
        class="row"
        style="--depth: {depth}"
        data-name={r.name}
        aria-current={open(r) ? 'true' : undefined}
        title={broken ? broken.map((d) => d.message).join('\n') : undefined}
        onclick={() => studio.openRow(r)}
        draggable={r.kind === 'icon'}
        ondragstart={(e) => e.dataTransfer?.setData('application/x-isolex-place', JSON.stringify({ kind: 'badge', name: r.name }))}
        onpointerenter={() => (hovered = r)}
        onpointerleave={() => (hovered = null)}
        onfocus={() => (hovered = r)}
        onblur={() => (hovered = null)}
      >
        <span class="thumb" class:sample={!!samples[r.name]}>
          {#if samples[r.name]}
            <IsoIcon icon={samples[r.name]!} height={14} />
          {:else if r.icon && view.icons[r.icon]}
            <IsoIcon icon={view.icons[r.icon]!} height={thumbHeight(view.icons[r.icon]!)} />
          {:else}
            <span class="glyph" aria-hidden="true">{r.kind === 'font' ? 'Aa' : r.kind === 'base' ? '◇' : '·'}</span>
          {/if}
        </span>
        <span class="name">{r.name}</span>
        {#if kindNote(r)}<span class="kind">{kindNote(r)}</span>{/if}
        {#if broken}
          <span class="broken" aria-hidden="true">▲</span>
        {:else if times > 0}
          {#key times}<span class="redrawn" aria-hidden="true"></span>{/key}
        {/if}
      </button>
      {#if r.children.length}
        <ul>{@render rows(r.children, depth + 1)}</ul>
      {/if}
    </li>
  {/each}
{/snippet}

<div class="tree {look}">
  <div class="head">
    <div class="views" role="radiogroup" aria-label="Tree view">
      <label><input type="radio" bind:group={mode} value="library" /> Library</label>
      <label><input type="radio" bind:group={mode} value="focus" /> Focus</label>
    </div>
    {#if mode === 'focus'}
      <label class="depth">
        Depth
        <input
          type="number"
          min={DEPTHS[0]}
          max={DEPTHS.at(-1)}
          step="1"
          value={depth}
          onchange={(e) => {
            depth = clampDepth(e.currentTarget.valueAsNumber, depth);
            e.currentTarget.value = String(depth);
          }}
        />
      </label>
    {/if}
  </div>
  {#if mode === 'focus'}
    <FocusView {studio} {depth} onhint={(s) => (focusSaid = s)} />
    <p class="said" role="status">{focusSaid}</p>
  {:else}
    <ul class="top" aria-label="Icons">
      {@render rows(view.tree?.icons ?? [], 0)}
    </ul>
    <div class="fonts-head">
      <h3>Fonts</h3>
      <button onclick={() => (importing = true)} disabled={!inShell()} title={inShell() ? 'A TrueType, OpenType or WOFF font, as an isolex font' : 'Fonts are imported in the studio’s app'}>Import…</button>
    </div>
    {#if view.tree?.fonts.length}
      <ul class="top" aria-label="Fonts">
        {@render rows(view.tree.fonts, 0)}
      </ul>
    {:else}
      <p class="none">No fonts yet: import one to set text.</p>
    {/if}
    {#if importing}<ImportFont {studio} onclose={() => (importing = false)} />{/if}
    <p class="said" role="status">{said ? `${hovered!.name}: ${said}` : ''}</p>
  {/if}
</div>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    padding: 6px 0 0;
    background: transparent;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0 12px 6px;
    font-size: 12px;
  }
  .views {
    display: flex;
    gap: 10px;
  }
  .depth {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  .depth input {
    width: 3.5em;
    padding: 1px 4px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--raised);
    color: var(--fg);
    font-size: 12px;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .fonts-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 12px 12px 4px 0;
  }
  .fonts-head button {
    padding: 2px 8px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    font-size: 11px;
    cursor: pointer;
  }
  .fonts-head button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .none {
    margin: 0 12px;
    color: var(--muted);
    font-size: 12px;
  }
  .thumb.sample {
    width: 96px;
    justify-content: start;
    padding: 0 4px;
    overflow: hidden;
  }
  h3 {
    margin: 0 12px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 3px 10px 3px calc(10px + var(--depth) * 16px);
    border: 0;
    background: transparent;
    color: var(--fg);
    text-align: left;
    cursor: pointer;
  }
  .row:hover,
  .row:focus-visible {
    background: var(--raised);
  }
  .row[aria-current='true'] {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    box-shadow: inset 2px 0 var(--accent);
  }
  .thumb {
    display: grid;
    place-items: center;
    width: 44px;
    height: 34px;
    flex: none;
    border-radius: 6px;
    background: var(--canvas);
  }
  .glyph {
    color: var(--muted);
    font-size: 13px;
  }
  .name {
    font: 12px var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
  }
  .broken {
    margin-left: auto;
    color: var(--warn);
  }
  .redrawn {
    margin-left: auto;
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 50%;
    background: var(--accent);
    animation: fade 2.4s ease-out forwards;
  }
  @keyframes fade {
    0%,
    30% {
      opacity: 1;
      transform: scale(1.4);
    }
    100% {
      opacity: 0;
      transform: scale(1);
    }
  }
  .said {
    position: sticky;
    bottom: 0;
    margin: auto 0 0;
    min-height: 1.4em;
    padding: 6px 12px;
    background: var(--panel);
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 12px;
  }
</style>
