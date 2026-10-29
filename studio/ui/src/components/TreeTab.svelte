<!--
  The Tree tab (docs/studio-interface.md, Tree): every icon of the library
  as a live thumbnail, nested by `extends`, each icon's axis variants under
  it, and fonts in a section of their own (#71). An icon an edit redrew
  gets a dot that fades; one it broke a warning mark, its last drawing
  kept and its errors on hover; one it did not change is not drawn again.
  A click opens that icon, or that variant, for editing; the one being
  edited is marked; hovering says how a row relates to what is open.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { TreeRow } from '../lib/backend';
  import { relation } from '../lib/tree';
  import IsoIcon from './IsoIcon.svelte';

  interface Props {
    studio: Studio;
    /** The canvas's look, which the thumbnails are drawn in. */
    look: string;
  }

  let { studio, look }: Props = $props();

  const view = $derived(studio.view);
  let hovered = $state<TreeRow | null>(null);
  const said = $derived(hovered && view.graph ? relation(view.graph, hovered, studio.editing, studio.icon) : '');

  /** The row open: its definition edited, its icon (if it has one) drawn. */
  const open = (r: TreeRow) => r.definition === studio.editing && (!r.icon || r.icon === studio.icon);
  const kindNote = (r: TreeRow) => (r.kind === 'variant' ? `variant ${r.variant ?? ''}`.trim() : r.kind === 'icon' ? '' : r.kind);
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
        onpointerenter={() => (hovered = r)}
        onpointerleave={() => (hovered = null)}
        onfocus={() => (hovered = r)}
        onblur={() => (hovered = null)}
      >
        <span class="thumb">
          {#if r.icon && view.icons[r.icon]}
            <IsoIcon icon={view.icons[r.icon]!} height={32} />
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
  <ul class="top" aria-label="Icons">
    {@render rows(view.tree?.icons ?? [], 0)}
  </ul>
  {#if view.tree?.fonts.length}
    <h3>Fonts</h3>
    <ul class="top" aria-label="Fonts">
      {@render rows(view.tree.fonts, 0)}
    </ul>
  {/if}
  <p class="said" role="status">{said ? `${hovered!.name}: ${said}` : ''}</p>
</div>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    padding: 6px 0 0;
    background: transparent;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  h3 {
    margin: 12px 12px 4px;
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
