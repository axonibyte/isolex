<!--
  The Parts panel (docs/studio-interface.md, Parts): the edited
  definition's parts, top to bottom in source order, each with its shapes
  topmost first; the part new shapes go into; renaming, adding, hiding and
  locking parts; reordering shapes by drag or PageUp, PageDown, Home and
  End. Below a line, the parts it draws from the definitions it extends,
  each opening the definition it comes from.
-->
<script lang="ts">
  import type { NodeAt, Part, Row } from '../lib/backend';
  import type { Studio } from '../lib/state.svelte';
  import { dropAbove, move, same, type Move } from '../lib/select';

  interface Props {
    studio: Studio;
  }

  let { studio }: Props = $props();

  const outline = $derived(studio.outline);
  let renaming = $state<string | null>(null);
  let draft = $state('');
  /** The row being dragged: its siblings and index. */
  let dragging: { siblings: Row[]; index: number; path: string } | null = null;

  const KEYS: Record<string, Move> = { PageUp: 'raise', PageDown: 'lower', Home: 'top', End: 'bottom' };

  const isSelected = (n: NodeAt) => !!studio.selected && same(studio.selected, n);

  function pickPart(p: Part) {
    if (p.op !== 'remove') studio.target = p.name;
    studio.select(p.node);
  }

  function startRename(p: Part) {
    renaming = p.name;
    draft = p.name;
  }

  async function rename(p: Part) {
    const name = draft.trim();
    renaming = null;
    if (!name || name === p.name) return;
    const answer = await studio.ask({ kind: 'set-arg', node: p.node, index: 0, value: name }, `Rename part ${p.name} to ${name}`);
    if (answer.kind !== 'applied') return;
    const renamed = (set: Set<string>) => new Set([...set].map((n) => (n === p.name ? name : n)));
    studio.hidden = renamed(studio.hidden);
    studio.locked = renamed(studio.locked);
    if (studio.target === p.name) studio.target = name;
  }

  async function addPart() {
    const o = outline;
    if (!o) return;
    const taken = new Set(o.parts.map((p) => p.name).concat(o.inherited.map((p) => p.name)));
    let k = 1;
    while (taken.has(`part-${k}`)) k++;
    const name = `part-${k}`;
    const answer = await studio.ask({ kind: 'insert', parent: o.node, text: `part "${name}" {\n}`, before: null }, `Add part ${name} to ${o.definition}`);
    if (answer.kind === 'applied') studio.target = name;
  }

  function toggle(set: 'hidden' | 'locked', name: string) {
    const next = new Set(studio[set]);
    if (!next.delete(name)) next.add(name);
    studio[set] = next;
  }

  const reorder = (ask: ReturnType<typeof move>, siblings: Row[], index: number, path: string) => studio.reorder(ask, siblings, index, path);

  function onrowkey(e: KeyboardEvent, siblings: Row[], index: number, path: string) {
    const how = KEYS[e.key];
    if (!how) return;
    e.preventDefault();
    void reorder(move(siblings, index, how), siblings, index, path);
  }

  function ondrop(e: DragEvent, siblings: Row[], index: number) {
    e.preventDefault();
    const d = dragging;
    dragging = null;
    if (!d || d.siblings !== siblings) return;
    void reorder(dropAbove(siblings, d.index, index), siblings, d.index, d.path);
  }
</script>

{#snippet rows(list: Row[], path: string, depth: number)}
  <!-- topmost (last written) first -->
  {#each [...list.keys()].reverse() as i (i)}
    {@const r = list[i]!}
    {@const here = `${path}/${i}`}
    <li>
      <button
        class="row"
        style="padding-left: {12 + depth * 14}px"
        aria-current={isSelected(r.node)}
        draggable="true"
        title="PageUp, PageDown, Home and End move it in painter order; drag to reorder"
        onclick={() => studio.select(r.node)}
        onkeydown={(e) => onrowkey(e, list, i, here)}
        ondragstart={(e) => {
          dragging = { siblings: list, index: i, path: here };
          e.dataTransfer?.setData('text/plain', here);
        }}
        ondragover={(e) => {
          if (dragging?.siblings === list) e.preventDefault();
        }}
        ondrop={(e) => ondrop(e, list, i)}
        ><span class="kind">{r.kind}</span><span class="note">{r.note}</span></button
      >
      {#if r.children.length}
        <ul>{@render rows(r.children, here, depth + 1)}</ul>
      {/if}
    </li>
  {/each}
{/snippet}

<div class="parts">
  <div class="top">
    <h2>Parts</h2>
    {#if outline}
      <button class="add" onclick={addPart} aria-label="Add a part" title="Add a part">+</button>
    {/if}
  </div>
  {#if !outline}
    <p class="empty">{studio.editing ? 'Reading the definition…' : 'Open an icon to see its parts.'}</p>
  {:else}
    <ul class="list">
      {#if !outline.parts.length}
        <li class="empty">{outline.definition} has no parts of its own.</li>
      {/if}
      {#each outline.parts as p, k (k)}
        {@const hidden = studio.hidden.has(p.name)}
        {@const locked = studio.locked.has(p.name)}
        <li class="part" class:dim={hidden}>
          <div class="head" class:target={studio.target === p.name}>
            {#if renaming === p.name}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                aria-label="New name for part {p.name}"
                bind:value={draft}
                autofocus
                onkeydown={(e) => {
                  if (e.key === 'Enter') void rename(p);
                  else if (e.key === 'Escape') renaming = null;
                }}
                onblur={() => void rename(p)}
              />
            {:else}
              <button
                class="name"
                aria-current={isSelected(p.node)}
                title="Click to draw here; double-click to rename"
                onclick={() => pickPart(p)}
                ondblclick={() => startRename(p)}
                onkeydown={(e) => {
                  if (e.key === 'F2') startRename(p);
                }}
                >{#if p.op !== 'part'}<span class="op">{p.op}</span>{/if}{p.name}{#if p.variant}<span class="note">
                    in {p.variant}</span
                  >{/if}</button
              >
            {/if}
            {#if studio.target === p.name}<span class="here">drawing here</span>{/if}
            <button class="flag" aria-pressed={hidden} aria-label="Hide part {p.name}" title="Hide in the editor" onclick={() => toggle('hidden', p.name)}
              ><svg viewBox="0 0 20 20" aria-hidden="true"
                >{#if hidden}<path d="M3 3l14 14M8 5.5A8 8 0 0 1 18 10a9 9 0 0 1-2.5 3M12 14.5A8 8 0 0 1 2 10a9 9 0 0 1 3-3.6" />{:else}<path
                    d="M2 10s3-5.5 8-5.5S18 10 18 10s-3 5.5-8 5.5S2 10 2 10z"
                  /><circle cx="10" cy="10" r="2.5" />{/if}</svg
              ></button
            >
            <button class="flag" aria-pressed={locked} aria-label="Lock part {p.name}" title="Lock in the editor" onclick={() => toggle('locked', p.name)}
              ><svg viewBox="0 0 20 20" aria-hidden="true"
                ><rect x="4.5" y="9" width="11" height="8" rx="1.5" /><path d={locked ? 'M7 9V6.5a3 3 0 0 1 6 0V9' : 'M7 9V6.5a3 3 0 0 1 5.8-1'} /></svg
              ></button
            >
          </div>
          {#if p.shapes.length}
            <ul>{@render rows(p.shapes, `${k}`, 1)}</ul>
          {/if}
        </li>
      {/each}
    </ul>
    {#if outline.inherited.length}
      <h2 class="inherited">Inherited</h2>
      <ul class="list">
        {#each outline.inherited as p (p.name)}
          <li>
            <button class="row from" title="Open {p.from}" onclick={() => studio.edit(p.from)}
              ><span class="kind">{p.name}</span><span class="note">from {p.from}</span></button
            >
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .parts {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-right: 8px;
  }
  h2 {
    margin: 0;
    padding: 10px 12px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  h2.inherited {
    border-top: 1px solid var(--line);
    margin-top: 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .empty {
    margin: 4px 12px;
    color: var(--muted);
  }
  .add,
  .flag {
    display: inline-grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .add {
    font-size: 16px;
  }
  .add:hover,
  .flag:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .flag svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
  }
  .flag[aria-pressed='false'] {
    opacity: 0.5;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px 6px 2px 4px;
    border-left: 2px solid transparent;
  }
  .head.target {
    border-left-color: var(--accent);
  }
  .name {
    flex: 1;
    min-width: 0;
    text-align: left;
    padding: 4px 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    font-weight: 600;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .head input {
    flex: 1;
    min-width: 0;
    padding: 3px 6px;
    border: 1px solid var(--accent);
    border-radius: 6px;
    background: var(--bg);
  }
  .op {
    margin-right: 6px;
    font-weight: 400;
    color: var(--warn);
  }
  .here {
    font-size: 11px;
    color: var(--accent);
    white-space: nowrap;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: baseline;
    width: 100%;
    padding: 3px 8px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .row:hover,
  .name:hover {
    background: var(--raised);
  }
  .row[aria-current='true'],
  .name[aria-current='true'] {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .kind {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .note {
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dim .row,
  .dim .name {
    opacity: 0.5;
  }
  .from {
    padding-left: 12px;
  }
</style>
