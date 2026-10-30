<!--
  What the Insert and Badge tools place (#116, docs/studio-interface.md,
  Insert and Badge), as Inkscape's Symbols dialog lists what it places:
  the library's fragments, or its icons, found by name. One chosen is
  placed by a click on the canvas; any can be dragged onto it. One that
  cannot go in what is open says why.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { Tools } from '../lib/tools.svelte';
  import { refusal } from '../lib/place';
  import IsoIcon from './IsoIcon.svelte';
  import { thumbHeight } from './TreeTab.svelte';

  interface Props {
    studio: Studio;
    tools: Tools;
    kind: 'insert' | 'badge';
  }

  let { studio, tools, kind }: Props = $props();

  let filter = $state('');
  const all = $derived(
    (studio.view.graph?.definitions ?? [])
      .filter((d) => d.kind === (kind === 'insert' ? 'Fragment' : 'Icon'))
      .map((d) => d.name)
      .sort((a, b) => a.localeCompare(b)),
  );
  const shown = $derived(all.filter((n) => n.toLowerCase().includes(filter.trim().toLowerCase())));
  const why = (name: string) => (studio.view.graph && studio.editing ? refusal(studio.view.graph, studio.editing, name, kind) : null);
  const chosen = $derived(tools.placing?.kind === kind ? tools.placing.name : null);

  function choose(name: string) {
    tools.placing = { kind, name };
    studio.message = kind === 'insert' ? `Click where ${name} goes` : `Click near the corner ${name} goes in`;
  }
</script>

<div class="picker" role="dialog" aria-label={kind === 'insert' ? 'Insert a fragment' : 'Badge an icon'}>
  <input type="search" placeholder="Find…" aria-label={kind === 'insert' ? 'Find a fragment' : 'Find an icon'} bind:value={filter} />
  {#if !all.length}
    <p class="none">{kind === 'insert' ? 'This library has no fragments.' : 'This library has no icons.'}</p>
  {/if}
  <ul aria-label={kind === 'insert' ? 'Fragments' : 'Icons'}>
    {#each shown as name (name)}
      {@const no = why(name)}
      <li>
        <button
          aria-pressed={chosen === name}
          disabled={!!no}
          title={no ?? (kind === 'insert' ? `Click, then click the canvas; or drag ${name} onto it` : `Click, then click near a corner; or drag ${name} onto it`)}
          draggable={!no}
          ondragstart={(e) => e.dataTransfer?.setData('application/x-isolex-place', JSON.stringify({ kind, name }))}
          onclick={() => choose(name)}
        >
          {#if kind === 'badge' && studio.view.icons[name]}
            <span class="thumb"><IsoIcon icon={studio.view.icons[name]!} height={Math.min(18, thumbHeight(studio.view.icons[name]!) / 2)} /></span>
          {/if}
          <span class="name">{name}</span>
        </button>
      </li>
    {/each}
  </ul>
</div>

<style>
  .picker {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 4;
    width: 210px;
    max-height: 60%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--panel);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.25);
  }
  input {
    padding: 4px 6px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--raised);
    color: var(--fg);
    font-size: 12px;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: auto;
  }
  button {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 3px 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--fg);
    text-align: left;
    cursor: grab;
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--raised);
  }
  button[aria-pressed='true'] {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .thumb {
    display: grid;
    place-items: center;
    width: 26px;
    height: 20px;
    border-radius: 4px;
    background: var(--canvas);
  }
  .name {
    font: 12px var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .none {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
</style>
