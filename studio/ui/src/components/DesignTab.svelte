<!--
  The Design tab (docs/studio-interface.md, Design). With a shape
  selected: its numbers as the source names them, its paint chosen from
  the lexicon's tokens and stroke widths and never typed, its opacity, a
  rectangle's corner radius, a path open or closed, the shape repeated
  along an axis as one `repeat` (#83), and its order. With nothing
  selected: the icon's frame, and its params, each with a slider that
  previews a value and *Set as default*, which writes it (#70).
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { Tools } from '../lib/tools.svelte';
  import type { Ask, NodeAt } from '../lib/backend';
  import type { IrIcon } from '../lib/isolex/ir';
  import FontDesign from './FontDesign.svelte';
  import TextDesign from './TextDesign.svelte';
  import PlacedDesign from './PlacedDesign.svelte';
  import { DEFAULT_PALETTE } from '../lib/theme';
  import { apply, SLOTS, FLAT, type Point } from '../lib/geometry';
  import { argText, counter, inRepeat, propsOf, stepIn, withArg } from '../lib/kdl';
  import { parse, toD } from '../lib/path';
  import { atPath, find, move, type Move } from '../lib/select';
  import { clean } from '../lib/snap';
  import { plainData } from '../lib/flat';

  interface Props {
    studio: Studio;
    tools: Tools;
    /** The canvas's look, which the swatches are shown in. */
    look: string;
    /** The open icon as compiled, for its footprint as drawn. */
    icon: IrIcon | null;
  }

  let { studio, tools, look, icon }: Props = $props();

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

  /** Where a choice the core needs is offered: by the selected shape. */
  const anchor = $derived.by((): Point => {
    const s = tools.selected;
    if (!s || s.values.length < 2) return [0, 0];
    return apply(s.map, [s.values[0]!, s.values[1]!, FLAT.has(s.kind) ? 0 : (s.values[2] ?? 0)]);
  });

  /** Asks on the selected node, one undo step. */
  async function ask(make: (node: NodeAt) => Ask, label: string) {
    const node = studio.selected;
    if (node) await tools.change(node, label, anchor, [make]);
  }

  // --- numbers, opacity, order ----------------------------------------------

  const slots = $derived(SLOTS[kind] ?? null);
  /** Each number as written, by what the language calls it. */
  const numbers = $derived(slots ? slots.names.map((name, i) => ({ name, k: slots.first + i, written: argText(text, slots.first + i) ?? '' })) : []);
  /** A number typed: a number, or else an expression, as written. */
  function setNumber(k: number, name: string, typed: string) {
    const t = typed.trim();
    if (!t || t === argText(text, k)) return;
    const value = /^-?(\d+\.?\d*|\.\d+)$/.test(t) ? Number(t) : t;
    void ask((node) => ({ kind: 'set-arg', node, index: k, value }), `Set ${name} of ${what}`);
  }

  function order(how: Move) {
    if (!found) return;
    void studio.reorder(move(found.siblings, found.index, how), found.siblings, found.index, found.path);
  }

  // --- the frame and the params, with nothing selected ---------------------

  const frame = $derived(studio.outline?.frame ?? null);
  function setFrame(k: number, typed: string) {
    const f = frame;
    const v = Number(typed);
    if (!f || !(v > 0) || String(v) === f.args[k]) return;
    void tools.change(f.node, `Set the canvas of ${studio.editing}`, [0, 0], [(node) => ({ kind: 'set-arg', node, index: k, value: v })]);
  }

  /** Values tried on the sliders, not yet written. */
  let trial = $state<Record<string, number>>({});
  // the library changed: what was tried is gone with its preview
  $effect(() => {
    void studio.view.library?.revision;
    void studio.editing;
    trial = {};
  });
  function tryValue(name: string, v: number) {
    trial = { ...trial, [name]: v };
    void studio.preview(trial);
  }
  function reset(name: string) {
    const { [name]: _, ...rest } = trial;
    trial = rest;
    void studio.preview(Object.keys(rest).length ? rest : null);
  }
  function setDefault(p: { name: string; node: NodeAt }) {
    const v = trial[p.name];
    if (v === undefined) return;
    void tools.change(p.node, `Set ${p.name} to ${v}`, [0, 0], [(node) => ({ kind: 'set-arg', node, index: 1, value: v })]);
  }
  /** A slider's reach about a default: to twice it, by whole steps for a
      whole number. */
  const range = (v: number) => ({ min: Math.min(0, v * 2), max: Math.max(v * 2, v + 1, 1), step: Number.isInteger(v) ? 1 : 0.01 });

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
    const indent = new TextDecoder().decode(bytes.slice(line, node.start)).match(/^[ \t]*/)?.[0] ?? '';
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
    // the shape stays selected, now the repeat's: its copies and step show
    const path = found?.path;
    await ask((n) => ({ kind: 'type', file: n.file, at: n.start, len: n.len, text: wrapped }), `Repeat ${what} ×${count}`);
    if (path) studio.follow(`${path}/0`);
    await studio.refresh();
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
  /** How the repeated shape steps, when it is written as `"a + i * s"`. */
  const stepping = $derived(repeat ? stepIn(text) : null);
  function setStep(by: number) {
    const s = stepping;
    if (!s || !by || by === s.by) return;
    void ask((node) => ({ kind: 'set-arg', node, index: s.k, value: `${clean(s.from)} + ${s.v} * ${clean(by)}` }), `Step ${what} by ${by}`);
  }
</script>

<div class="design {look}">
  {#if !row && studio.font}
    <FontDesign {studio} {tools} {look} />
  {:else if !row}
    <h2>{studio.editing ?? 'Nothing open'} <small>{studio.outline?.kind ?? ''}</small></h2>
    {#if frame}
      <section aria-labelledby="frame">
        <h3 id="frame">{frame.kind === 'canvas' ? 'Canvas' : 'Footprint'}</h3>
        {#if frame.kind === 'canvas'}
          {#each ['Width', 'Height'] as label, k (label)}
            <label class="row">
              <span class="label">{label}</span>
              <input type="text" value={frame.args[k] ?? ''} onchange={(e) => setFrame(k, e.currentTarget.value)} />
            </label>
          {/each}
        {:else}
          <p class="note">
            <code>footprint {frame.args.join(' ')}</code>{#if icon?.footprint}, drawn as {icon.footprint.join(' × ')}{/if}: what is drawn decides it.
          </p>
        {/if}
        {#if frame.from}<p class="note">From {frame.from}.</p>{/if}
      </section>
    {/if}
    <section aria-labelledby="params">
      <h3 id="params">Params</h3>
      {#each studio.outline?.params ?? [] as p (p.name)}
        {@const tried = trial[p.name]}
        <div class="param" role="group" aria-label="Param {p.name}">
          <div class="head">
            <code class="name">{p.name}</code>
            <span class="default">{p.default}</span>
            {#if p.from}<span class="from">from {p.from}</span>{/if}
          </div>
          {#if p.value !== null}
            {@const r = range(p.value)}
            <div class="slide">
              <input type="range" aria-label="Preview {p.name}" min={r.min} max={r.max} step={r.step} value={tried ?? p.value} oninput={(e) => tryValue(p.name, Number(e.currentTarget.value))} />
              <output class="value">{tried ?? p.value}</output>
            </div>
            {#if tried !== undefined && tried !== p.value}
              <div class="actions">
                <button class="primary" onclick={() => setDefault(p)}>Set as default</button>
                <button onclick={() => reset(p.name)}>Reset</button>
              </div>
            {/if}
          {:else}
            <p class="note">Worked out from its expression; change it in the Source tab.</p>
          {/if}
        </div>
      {:else}
        <p class="note">{studio.editing ?? 'This'} has no params.</p>
      {/each}
    </section>
    <p class="empty">Select a shape to see its numbers and paint it.</p>
  {:else}
    <h2>{kind} <small>{row.note}</small></h2>

    {#if numbers.length}
      <section aria-labelledby="numbers">
        <h3 id="numbers">Numbers</h3>
        {#if kind === 'facerect'}<p class="note">On the {argText(text, 0) ?? 'front'} face.</p>{/if}
        <div class="numbers">
          {#each numbers as num (num.k)}
            <label class="number">
              <span class="label">{num.name}</span>
              <input type="text" value={num.written} spellcheck="false" onchange={(e) => setNumber(num.k, num.name, e.currentTarget.value)} />
            </label>
          {/each}
        </div>
      </section>
    {:else if kind === 'poly' || kind === 'path' || kind === 'poly3d'}
      <p class="note">Its points are shaped with the node tool, or in the Source tab.</p>
    {/if}

    {#if kind === 'text' && studio.selected}
      <TextDesign {studio} {tools} node={studio.selected} {text} />
    {:else if (kind === 'insert' || kind === 'badge') && studio.selected}
      <PlacedDesign {studio} {tools} node={studio.selected} {kind} {text} />
    {:else}
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
          <div class="row paint" role="group" aria-label={label}>
            <span class="label">{label}</span>
            <div class="swatches">
              {#each tokens as t (t)}
                <button class="swatch" style="--c: var(--iso-{t})" aria-label="{label}: {t}" title={t} aria-pressed={(written[key] ?? fallback) === t} onclick={() => setProp(key, t, fallback)}></button>
              {/each}
              <span class="token">{written[key] ?? fallback}</span>
            </div>
          </div>
        {/each}
      {:else if kind === 'shadow'}
        <p class="note">A shadow is painted with the shadow token, as the language paints it.</p>
      {:else if paint}
        {#each [['fill', 'Fill'], ['stroke', 'Stroke']] as [key, label] (key)}
          {@const fallback = paint[key as 'fill' | 'stroke']}
          {@const now = written[key] ?? fallback}
          <div class="row paint" role="group" aria-label={label}>
            <span class="label">{label}</span>
            <div class="swatches">
              <button class="swatch none" aria-label="{label}: none" title="none" aria-pressed={now === 'none'} onclick={() => setProp(key, 'none', fallback)}></button>
              {#each tokens as t (t)}
                <button class="swatch" style="--c: var(--iso-{t})" aria-label="{label}: {t}" title={t} aria-pressed={now === t} onclick={() => setProp(key, t, fallback)}></button>
              {/each}
              <span class="token">{now}</span>
            </div>
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
    {/if}

    {#if kind !== 'shadow' && SLOTS[kind]}
      <section aria-labelledby="opacity">
        <h3 id="opacity">Opacity</h3>
        <label class="row">
          <span class="label">Opacity</span>
          <input type="number" min="0" max="1" step="0.05" value={Number(written.opacity ?? 1)} onchange={(e) => setProp('opacity', clean(Math.min(1, Math.max(0, Number(e.currentTarget.value)))), 1)} />
        </label>
      </section>
    {/if}

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
        {#if stepping}
          <label class="row">
            <span class="label">Step</span>
            <input type="number" step="any" value={stepping.by} onchange={(e) => setStep(Number(e.currentTarget.value))} />
          </label>
        {:else}
          <p class="note">Written as one <code>repeat</code>; its step is in the source.</p>
        {/if}
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

    <section aria-labelledby="order">
      <h3 id="order">Order</h3>
      <div class="buttons">
        <button onclick={() => order('raise')} disabled={!found || found.index >= found.siblings.length - 1}>Bring forward</button>
        <button onclick={() => order('lower')} disabled={!found || found.index <= 0}>Send back</button>
        <button onclick={() => studio.selected && tools.duplicate(studio.selected)}>Duplicate</button>
        <button onclick={() => studio.selected && tools.remove(studio.selected)}>Delete</button>
      </div>
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
  .row.paint {
    grid-template-columns: 56px 1fr;
    align-items: start;
  }
  .row.paint .label {
    padding-top: 3px;
  }
  .token {
    flex-basis: 100%;
    font: 11px var(--font-mono);
    color: var(--muted);
  }
  button:not(.swatch) {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  button:not(.swatch):hover:not(:disabled) {
    border-color: var(--muted);
  }
  button:not(.swatch):disabled {
    opacity: 0.45;
    cursor: default;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-fg);
  }
  .seg button[aria-pressed='false'] {
    background: transparent;
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
  input[type='text'],
  select {
    width: 100%;
    box-sizing: border-box;
  }
  .numbers {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
  }
  .number {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .number input {
    font: 12px var(--font-mono);
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .param {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 6px 0;
    border-top: 1px solid var(--line);
  }
  .param .head {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .param .name {
    font: 12px var(--font-mono);
    color: var(--fg);
  }
  .param .default,
  .param .from {
    color: var(--muted);
    font-size: 12px;
  }
  .param .default {
    font-family: var(--font-mono);
  }
  .slide {
    display: grid;
    grid-template-columns: 1fr 44px;
    gap: 8px;
    align-items: center;
  }
  .value {
    font: 12px var(--font-mono);
    text-align: right;
  }
  .actions {
    display: flex;
    gap: 4px;
  }
</style>
