<!--
  The Design tab (docs/studio-interface.md, Design), as far as drawing
  needs it (#83): the selected shape's paint, chosen from the lexicon's
  tokens and stroke widths and never typed; a rectangle's corner radius; a
  path open or closed; and the shape repeated along an axis as one
  `repeat`. Its numbers, order and params come with #70.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { Tools } from '../lib/tools.svelte';
  import type { Ask, NodeAt } from '../lib/backend';
  import { DEFAULT_PALETTE } from '../lib/theme';
  import { SLOTS, FLAT } from '../lib/geometry';
  import { argText, counter, inRepeat, propsOf, withArg } from '../lib/kdl';
  import { parse, toD } from '../lib/path';
  import { atPath, find } from '../lib/select';
  import { clean } from '../lib/snap';
  import { plainData } from '../lib/flat';

  interface Props {
    studio: Studio;
    tools: Tools;
    /** The canvas's look, which the swatches are shown in. */
    look: string;
  }

  let { studio, tools, look }: Props = $props();

  const found = $derived(find(studio.outline, studio.selected));
  const row = $derived(found?.row ?? null);
  const kind = $derived(row?.kind ?? '');
  /** The node's text as the source has it now. */
  let text = $state('');
  $effect(() => {
    const node = studio.selected;
    void studio.view.library?.revision;
    if (node) void tools.text(node).then((t) => (text = t));
    else text = '';
  });
  const written = $derived(propsOf(text));

  const tokens = $derived(studio.view.lexicon.tokens.length ? studio.view.lexicon.tokens : Object.keys(studio.view.palette ?? DEFAULT_PALETTE));
  /** Widths to choose from: the lexicon's, or the language's own. */
  const widths = $derived(studio.view.lexicon.widths.length ? studio.view.lexicon.widths : [1.2, 1.4]);

  /** What each kind is painted with when its source says nothing. */
  const DEFAULTS: Record<string, { fill: string; stroke: string; width: number | null }> = {
    tile: { fill: 'body-top', stroke: 'outline', width: 1.2 },
    facerect: { fill: 'ink', stroke: 'none', width: null },
    poly3d: { fill: 'body-top', stroke: 'outline', width: 1.2 },
    seg: { fill: 'none', stroke: 'ink', width: 1.4 },
    line: { fill: 'none', stroke: 'ink', width: 1.4 },
    circle: { fill: 'none', stroke: 'outline', width: 1.2 },
    ellipse: { fill: 'none', stroke: 'outline', width: 1.2 },
    rect: { fill: 'none', stroke: 'none', width: 1.4 },
    arc: { fill: 'none', stroke: 'ink', width: 1.4 },
    path: { fill: 'none', stroke: 'ink', width: 1.4 },
    poly: { fill: 'ink', stroke: 'none', width: null },
  };
  const paint = $derived(DEFAULTS[kind] ?? null);

  /** Asks on the selected node, one undo step. */
  async function ask(make: (node: NodeAt) => Ask, label: string) {
    const node = studio.selected;
    if (!node) return;
    await studio.backend.beginGesture();
    const answer = await studio.ask(make(node), label);
    await studio.backend.endGesture();
    if (answer.kind === 'applied') await studio.refresh();
  }

  const what = $derived(`${kind} in ${studio.editing}`);
  function setProp(key: string, value: string | number, fallback: string | number | null) {
    void ask((node) => (value === fallback ? { kind: 'remove-prop', node, key } : { kind: 'set-prop', node, key, value }), `Paint ${what}`);
  }

  // --- a rectangle's corner radius, a path open or closed ---------------

  let corner = $state(0);
  $effect(() => {
    corner = Number(written.rx ?? 0);
  });
  const path = $derived(kind === 'path' ? parse(tools.selected?.d ?? '') : null);
  function setClosed(closed: boolean) {
    if (!path || !plainData(text)) {
      studio.message = 'This path’s data is worked out from expressions: change it in the Source tab';
      return;
    }
    void ask((node) => ({ kind: 'set-prop', node, key: 'd', value: toD({ ...path, closed }) }), `${closed ? 'Close' : 'Open'} ${what}`);
  }

  // --- repeat --------------------------------------------------------------

  /** The axes a kind repeats along, and the numbers (by index) each
      moves: both ends of a line; a face rect along its wall, which `along`
      says. */
  const AXES: Record<string, Record<string, number[]>> = {
    box: { x: [0], y: [1], z: [2] },
    tile: { x: [0], y: [1], z: [2] },
    cylinder: { x: [0], y: [1], z: [2] },
    shadow: { x: [0], y: [1] },
    facerect: { along: [], z: [2] },
    line: { x: [0, 2], y: [1, 3] },
    circle: { x: [0], y: [1] },
    ellipse: { x: [0], y: [1] },
    rect: { x: [0], y: [1] },
    arc: { x: [0], y: [1] },
  };
  const axes = $derived(AXES[kind] ?? null);
  /** A face rect's a on the front, b on the right. */
  const along = $derived(row?.note === 'right' ? 1 : 0);

  /** The repeat the node is the only shape of, if it is. */
  const repeat = $derived.by(() => {
    if (!found) return null;
    const cut = found.path.lastIndexOf('/');
    if (cut <= found.path.indexOf('/')) return null;
    const up = atPath(studio.outline, found.path.slice(0, cut))?.row;
    return up?.kind === 'repeat' && up.children.length === 1 ? up : null;
  });

  let count = $state(4);
  let axis = $state('x');
  let step = $state(1);
  $effect(() => {
    if (axes && !(axis in axes)) axis = Object.keys(axes)[0]!;
  });

  async function makeRepeat() {
    const node = studio.selected;
    const s = tools.selected;
    if (!node || !axes || !s || count < 2 || !step) return;
    const first = SLOTS[kind]!.first;
    const moved = axis === 'along' ? [along] : axes[axis]!;
    const src = await studio.backend.source(node.file);
    if (!src) return;
    // the node keeps its place: its line's indentation
    const bytes = new TextEncoder().encode(src.text);
    const line = bytes.lastIndexOf(10, node.start - 1) + 1;
    const indent = new TextDecoder().decode(bytes.slice(line, node.start)).match(/^[ 	]*/)?.[0] ?? '';
    const wrapped = wrap(
      text,
      moved.map((i) => i + first),
      moved.map((i) => s.values[i]!),
      indent,
    );
    if (!wrapped) {
      studio.message = 'This shape’s numbers are written as expressions: repeat it in the Source tab';
      return;
    }
    await ask((n) => ({ kind: 'type', file: n.file, at: n.start, len: n.len, text: wrapped }), `Repeat ${what} ×${count}`);
    studio.select(null);
  }

  /** The node in a repeat, each argument of `ks` stepping from its value
      in `from`; none if one is not written as a plain number. */
  function wrap(t: string, ks: number[], from: number[], indent: string): string | null {
    for (const k of ks) {
      const a = argText(t, k);
      if (a === null || a.trim() === '' || Number.isNaN(Number(a))) return null;
    }
    const v = counter(t);
    let node: string | null = t;
    for (let j = ks.length - 1; j >= 1 && node !== null; j--) node = withArg(node, ks[j]!, `"${clean(from[j]!)} + ${v} * ${clean(step)}"`);
    return node === null ? null : inRepeat(node, indent, ks[0]!, from[0]!, count, step, v);
  }

  function setCount(n: number) {
    const r = repeat;
    if (!r || n < 1) return;
    void ask(() => ({ kind: 'set-arg', node: r.node, index: 0, value: n }), `Repeat ${what} ×${n}`);
  }
  const copies = $derived(repeat ? Number(repeat.note.replace('×', '')) || 0 : 0);
</script>

<div class="design {look}">
  {#if !row}
    <p class="empty">Select a shape to paint it, or to repeat it.</p>
  {:else}
    <h2>{kind} <small>{row.note}</small></h2>

    <section aria-labelledby="paint">
      <h3 id="paint">Paint</h3>
      {#if kind === 'box'}
        <p class="note">A box’s faces are painted body-top, body-left and body-right, as the language paints them.</p>
        <div class="seg" role="group" aria-label="Faces">
          {#each ['body', 'virt', 'none'] as f (f)}
            <button aria-pressed={(written.fills ?? 'body') === f} onclick={() => setProp('fills', f, 'body')}>{f}</button>
          {/each}
        </div>
      {:else if kind === 'cylinder'}
        {#each [['top', 'Top', 'body-top'], ['side', 'Side', 'body-right']] as [key, label, fallback] (key)}
          <div class="row" role="group" aria-label={label}>
            <span class="label">{label}</span>
            <div class="swatches">
              {#each tokens as t (t)}
                <button class="swatch" style="--c: var(--iso-{t})" aria-label="{label}: {t}" title={t} aria-pressed={(written[key] ?? fallback) === t} onclick={() => setProp(key, t, fallback)}></button>
              {/each}
            </div>
            <span class="token">{written[key] ?? fallback}</span>
          </div>
        {/each}
      {:else if kind === 'shadow'}
        <p class="note">A shadow is painted with the shadow token, as the language paints it.</p>
      {:else if paint}
        {#each [['fill', 'Fill'], ['stroke', 'Stroke']] as [key, label] (key)}
          {@const fallback = paint[key as 'fill' | 'stroke']}
          {@const now = written[key] ?? fallback}
          <div class="row" role="group" aria-label={label}>
            <span class="label">{label}</span>
            <div class="swatches">
              <button class="swatch none" aria-label="{label}: none" title="none" aria-pressed={now === 'none'} onclick={() => setProp(key, 'none', fallback)}></button>
              {#each tokens as t (t)}
                <button class="swatch" style="--c: var(--iso-{t})" aria-label="{label}: {t}" title={t} aria-pressed={now === t} onclick={() => setProp(key, t, fallback)}></button>
              {/each}
            </div>
            <span class="token">{now}</span>
          </div>
        {/each}
        <label class="row">
          <span class="label">Width</span>
          <select value={String(Number(written.width ?? paint.width ?? widths[0]))} onchange={(e) => setProp('width', Number(e.currentTarget.value), paint.width)}>
            {#each widths as w (w)}<option value={String(w)}>{w}</option>{/each}
          </select>
        </label>
      {:else}
        <p class="note">This is painted where it is written.</p>
      {/if}
    </section>

    {#if kind === 'rect'}
      <section aria-labelledby="corners">
        <h3 id="corners">Corners</h3>
        <label class="row">
          <span class="label">Radius</span>
          <input type="number" min="0" step="0.5" bind:value={corner} onchange={() => setProp('rx', corner, 0)} />
        </label>
      </section>
    {/if}

    {#if kind === 'path' && path}
      <section aria-labelledby="ends">
        <h3 id="ends">Ends</h3>
        <div class="seg" role="group" aria-label="Path">
          <button aria-pressed={!path.closed} onclick={() => setClosed(false)}>Open</button>
          <button aria-pressed={path.closed} onclick={() => setClosed(true)}>Closed</button>
        </div>
      </section>
    {/if}

    <section aria-labelledby="repeat">
      <h3 id="repeat">Repeat</h3>
      {#if repeat}
        <label class="row">
          <span class="label">Copies</span>
          <input type="number" min="1" step="1" value={copies} onchange={(e) => setCount(Number(e.currentTarget.value))} />
        </label>
        <p class="note">Written as one <code>repeat</code>; its step is in the source.</p>
      {:else if axes}
        <label class="row">
          <span class="label">Copies</span>
          <input type="number" min="2" step="1" bind:value={count} />
        </label>
        <label class="row">
          <span class="label">Along</span>
          <select bind:value={axis}>
            {#each Object.keys(axes) as a (a)}<option value={a}>{a}</option>{/each}
          </select>
        </label>
        <label class="row">
          <span class="label">Step</span>
          <input type="number" step={FLAT.has(kind) ? 1 : 0.25} bind:value={step} />
        </label>
        <button class="primary" onclick={makeRepeat} disabled={count < 2 || !step}>Repeat</button>
      {:else}
        <p class="note">A {kind} is repeated in the Source tab.</p>
      {/if}
    </section>
  {/if}
</div>

<style>
  .design {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: transparent;
  }
  h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  h2 small {
    color: var(--muted);
    font-weight: 400;
    margin-left: 6px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: grid;
    grid-template-columns: 56px 1fr auto;
    align-items: center;
    gap: 8px;
  }
  .label {
    color: var(--muted);
    font-size: 12px;
  }
  .token {
    font: 11px var(--font-mono);
    color: var(--muted);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .swatch {
    width: 20px;
    height: 20px;
    padding: 0;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--c);
    cursor: pointer;
  }
  .swatch.none {
    background: linear-gradient(135deg, transparent 45%, var(--muted) 45% 55%, transparent 55%), var(--canvas);
  }
  .swatch[aria-pressed='true'] {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .note,
  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .seg {
    display: inline-flex;
    gap: 2px;
  }
  .seg button[aria-pressed='true'] {
    background: var(--raised);
    color: var(--fg);
  }
  input[type='number'],
  select {
    width: 100%;
  }
</style>
