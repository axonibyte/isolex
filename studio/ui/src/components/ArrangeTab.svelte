<!--
  The Arrange tab (#114, docs/studio-interface.md, Arrange): Align and
  Distribute, as Inkscape's dialog (Ctrl+Shift+A). The shapes selected
  (Shift+click for several) lined up by an edge or their middles along
  x, y and z, or a flat shape's x and y; relative to the first or last
  chosen, the selection's bounds, or the icon's footprint (a flat icon's
  canvas); or spread evenly, by their middles or their gaps. Each is one
  undo step.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { Tools } from '../lib/tools.svelte';
  import { FLAT, type Solid } from '../lib/geometry';
  import { same } from '../lib/select';
  import { align, distribute, extentOf, frameOf, reference, sameFrame, type Edge, type Relative, type Spread } from '../lib/arrange';

  interface Props {
    studio: Studio;
    tools: Tools;
  }

  let { studio, tools }: Props = $props();

  let relative = $state<Relative>('selection');

  /** The shapes Align and Distribute move, in the order chosen: each node
      selected that draws one shape, written where it is drawn, in the
      plane of the selection; and how many are left out. */
  const picked = $derived.by(() => {
    const all = tools.solids;
    const ones: Solid[] = [];
    for (const n of studio.chosen) {
      const mine = all.filter((s) => s.select && same(s.select, n) && same(s.node, n));
      if (mine.length === 1 && extentOf(mine[0]!)) ones.push(mine[0]!);
    }
    const last = ones.at(-1);
    const shapes = last ? ones.filter((s) => FLAT.has(s.kind) === FLAT.has(last.kind) && sameFrame(s.map, last.map)) : [];
    return { shapes, left: studio.chosen.length - shapes.length };
  });
  const shapes = $derived(picked.shapes);
  const flat = $derived(!!shapes[0] && FLAT.has(shapes[0].kind));
  const icon = $derived(studio.drawing?.ir ?? null);
  const frame = $derived(icon && shapes[0] ? frameOf(icon, shapes[0]) : null);
  const frameName = $derived(icon?.canvas ? 'The canvas' : 'The footprint');

  const AXES = $derived(
    flat
      ? [
          { axis: 0, name: 'x', edges: ['left', 'center', 'right'] },
          { axis: 1, name: 'y', edges: ['top', 'middle', 'bottom'] },
        ]
      : [
          { axis: 0, name: 'x', edges: ['min', 'middle', 'max'] },
          { axis: 1, name: 'y', edges: ['min', 'middle', 'max'] },
          { axis: 2, name: 'z', edges: ['bottom', 'middle', 'top'] },
        ],
  );
  const EDGES: Edge[] = ['min', 'center', 'max'];
  const SPREADS: [Spread, string][] = [
    ['centers', 'middles'],
    ['gaps', 'gaps'],
  ];

  const canAlign = $derived(relative === 'frame' ? !!frame && shapes.length >= 1 : shapes.length >= 2);
  const canSpread = $derived(shapes.length >= 3);

  const plural = (n: number) => (n === 1 ? 'one shape' : `${n} shapes`);

  async function run(verb: string, by: number[][]) {
    const moves = shapes.map((solid, i) => ({ solid, by: by[i]! }));
    const moved = moves.filter((m) => m.by.some((d) => d)).length;
    if (!moved) {
      studio.message = `${verb}: nothing to move`;
      return;
    }
    await tools.arrange(moves, `${verb} ${plural(moved)} in ${studio.editing}`);
    studio.message = `${verb} ${plural(moved)}`;
  }

  /** The move along `axis` for each shape, as a vector of its own axes. */
  const along = (axis: number, d: number[]) => d.map((v) => (flat ? [0, 0] : [0, 0, 0]).map((_, i) => (i === axis ? v : 0)));

  function alignTo(axis: number, edge: Edge) {
    const extents = shapes.map((s) => extentOf(s)!);
    const ref = reference(extents, relative, frame);
    if (ref) void run('Aligned', along(axis, align(extents, axis, edge, ref)));
  }

  function spread(axis: number, how: Spread) {
    void run('Distributed', along(axis, distribute(shapes.map((s) => extentOf(s)!), axis, how)));
  }
</script>

<div class="arrange">
  <section aria-labelledby="align-title">
    <h3 id="align-title">Align and Distribute</h3>
    {#if !shapes.length}
      <p class="empty">Select shapes to line up or spread out: Shift+click to select several.</p>
    {:else}
      <p class="note">
        {plural(shapes.length)}{flat ? ', flat' : ''}{picked.left ? `; ${picked.left} left out (a part, a group, a repeat, text, or in another plane)` : ''}.
      </p>
    {/if}
    <label class="row">
      <span class="label">Relative to</span>
      <select bind:value={relative}>
        <option value="first">The first chosen</option>
        <option value="last">The last chosen</option>
        <option value="selection">The selection</option>
        <option value="frame" disabled={!frame}>{frameName}</option>
      </select>
    </label>
    <table>
      <thead>
        <tr><th scope="col"><span class="sr">Axis</span></th><th scope="col">Align</th><th scope="col">Distribute</th></tr>
      </thead>
      <tbody>
        {#each AXES as a (a.axis)}
          <tr>
            <th scope="row">{a.name}</th>
            <td>
              <span class="buttons">
                {#each EDGES as edge, k (edge)}
                  <button aria-label="Align {a.name}: {a.edges[k]}" title="Line up each shape’s {a.edges[k]} along {a.name}" disabled={!canAlign} onclick={() => alignTo(a.axis, edge)}>{a.edges[k]}</button>
                {/each}
              </span>
            </td>
            <td>
              <span class="buttons">
                {#each SPREADS as [how, word] (how)}
                  <button aria-label="Distribute {a.name}: {word}" title="Space their {word} evenly along {a.name}, the outermost two staying" disabled={!canSpread} onclick={() => spread(a.axis, how)}>{word}</button>
                {/each}
              </span>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
</div>

<style>
  .arrange {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: transparent;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  .row {
    display: grid;
    grid-template-columns: 80px 1fr;
    align-items: center;
    gap: 8px;
  }
  .label {
    color: var(--muted);
    font-size: 12px;
  }
  select {
    justify-self: start;
    font-size: 12px;
  }
  table {
    border-collapse: collapse;
    font-size: 12px;
  }
  th {
    color: var(--muted);
    font-weight: 400;
    text-align: left;
    padding: 4px 8px 4px 0;
  }
  th[scope='row'] {
    font: 12px var(--font-mono);
    color: var(--fg);
  }
  td {
    padding: 4px 12px 4px 0;
  }
  .buttons {
    display: inline-flex;
    gap: 4px;
  }
  button {
    padding: 3px 8px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    border-color: var(--muted);
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .note,
  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
