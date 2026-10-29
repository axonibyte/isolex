<!--
  The Design tab for a font (#84, docs/studio-interface.md, Fonts): with
  the font open, its metrics (em, ascent, descent, slant, stroke or
  outline) and its export; with a glyph open, its advance and the kerning
  pairs it is in, each shown as the pair is set, its right-hand glyph
  dragged to set the space between them.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { Tools } from '../lib/tools.svelte';
  import type { Drawing, NodeAt } from '../lib/backend';
  import { advanceOf, metricsOf, pairSources } from '../lib/font';
  import { clean, snapValue } from '../lib/snap';
  import IsoIcon from './IsoIcon.svelte';
  import { inShell } from '../lib/backend';

  interface Props {
    studio: Studio;
    tools: Tools;
    look: string;
  }

  let { studio, tools, look }: Props = $props();

  const o = $derived(studio.outline);
  const font = $derived(o?.font ?? null);
  const m = $derived(metricsOf(o));
  const glyph = $derived(o?.parts.find((p) => p.name === studio.glyph) ?? null);

  function set(node: NodeAt, index: number, typed: string, label: string, number = true) {
    const t = typed.trim();
    if (!t) return;
    const value = number && Number.isFinite(Number(t)) ? Number(t) : t;
    void tools.change(node, label, [0, 0], [(n) => ({ kind: 'set-arg', node: n, index, value })]);
  }

  // --- the font's metrics, and a metric not yet written ------------------

  const METRICS = ['em', 'ascent', 'descent', 'slant'] as const;
  const written = (name: string) => font?.metrics.find((r) => r.name === name) ?? null;
  function setMetric(name: string, typed: string) {
    const r = written(name);
    if (r) set(r.node, 0, typed, `Set the ${name} of ${o!.definition}`);
    else if (o && typed.trim()) void studio.ask({ kind: 'insert', parent: o.node, text: `${name} ${typed.trim()}`, before: o.parts[0]?.node ?? null }, `Set the ${name} of ${o.definition}`);
  }

  // --- a glyph's advance ---------------------------------------------------

  let glyphText = $state('');
  $effect(() => {
    const g = glyph;
    void studio.view.library?.revision;
    if (g) void tools.text(g.node).then((t) => (glyphText = t));
  });
  const advance = $derived(glyphText.match(/\badvance=("?)([^"\s{]+)\1/)?.[2] ?? '');
  function setAdvance(typed: string) {
    const g = glyph;
    const v = Number(typed);
    if (!g || !Number.isFinite(v) || v < 0) return;
    void tools.change(g.node, `Set the advance of ${g.name}`, [0, 0], [(n) => ({ kind: 'set-prop', node: n, key: 'advance', value: v })]);
  }

  // --- kerning ---------------------------------------------------------------

  const pairs = $derived(glyph && font ? font.kerns.filter((k) => k.left === glyph.name || k.right === glyph.name) : []);
  const others = $derived(o?.parts.filter((p) => p.op === 'glyph' && p.name !== glyph?.name) ?? []);
  let other = $state('');
  let side = $state<'after' | 'before'>('after');

  /** Each pair's glyphs as the font draws them, by the pair's key, and
      the left one's advance, which the right one is set after. */
  let shown = $state<Record<string, { left: Drawing; right: Drawing; k: number; advance: number }>>({});
  $effect(() => {
    void studio.view.library?.revision;
    const fontName = o?.definition;
    const list = pairs;
    const outline = o;
    if (!fontName || !outline) return;
    void Promise.all(
      list.map(async (p) => {
        const s = pairSources(fontName, p.left, p.right, m);
        const part = outline.parts.find((q) => q.op === 'glyph' && q.name === p.left);
        const [left, right, text] = await Promise.all([studio.backend.sketch(s.left), studio.backend.sketch(s.right), part ? tools.text(part.node) : Promise.resolve('')]);
        return [`${p.left}|${p.right}`, left && right ? { left, right, k: s.k, advance: advanceOf(text, m) } : null] as const;
      }),
    ).then((all) => {
      const next: Record<string, { left: Drawing; right: Drawing; k: number; advance: number }> = {};
      for (const [key, v] of all) if (v) next[key] = v;
      shown = next;
    });
  });

  /** The pair being dragged: where from, and the space it is at. */
  let dragging = $state<{ key: string; x: number; from: number; by: number } | null>(null);
  function down(e: PointerEvent, key: string, by: number) {
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
    dragging = { key, x: e.clientX, from: by, by };
  }
  /** The drag in screen pixels, `k` of them to a unit of the em grid. */
  function move(e: PointerEvent, k: number) {
    const d = dragging;
    if (!d) return;
    d.by = clean(snapValue(d.from + (e.clientX - d.x) / k, m.em / 100));
  }
  function up(p: { node: NodeAt; left: string; right: string }) {
    const d = dragging;
    dragging = null;
    if (d && d.by !== d.from) void tools.change(p.node, `Kern ${p.left}${p.right}`, [0, 0], [(n) => ({ kind: 'set-arg', node: n, index: 2, value: d.by })]);
  }

  async function addPair() {
    const g = glyph;
    if (!g || !o || !other) return;
    const [left, right] = side === 'after' ? [g.name, other] : [other, g.name];
    const q = (s: string) => JSON.stringify(s);
    await studio.ask({ kind: 'insert', parent: o.node, text: `kern ${q(left)} ${q(right)} 0`, before: null }, `Kern ${left}${right}`);
    other = '';
  }

  // --- export -----------------------------------------------------------------

  const styles = $derived((studio.view.tree?.fonts.find((f) => f.name === o?.definition)?.children ?? []).map((r) => r.variant ?? '').filter(Boolean));
  let chosen = $state<string[]>([]);
  let ufo = $state(false);
  let exported = $state('');
  async function exportFont() {
    const name = o?.definition;
    if (!name) return;
    const suffix = chosen.length ? `-${chosen.join('-')}` : '';
    const out = await studio.backend.pickSave(`Export ${name}`, `${name}${suffix}${ufo ? '.ufo' : '.ttf'}`, ufo ? [] : ['ttf']);
    if (!out) return;
    try {
      exported = `Written: ${await studio.backend.exportFont(name, chosen, out, ufo)}`;
    } catch (e) {
      exported = String(e instanceof Error ? e.message : e);
    }
  }
  const inApp = inShell();
</script>

<div class="font {look}">
  {#if !glyph}
    <h2>{o?.definition} <small>font</small></h2>
    <section aria-labelledby="metrics">
      <h3 id="metrics">Metrics</h3>
      {#each METRICS as name (name)}
        <label class="row">
          <span class="label">{name}</span>
          <input type="text" value={written(name)?.value ?? ''} placeholder={name === 'slant' ? '0' : ''} onchange={(e) => setMetric(name, e.currentTarget.value)} />
        </label>
      {/each}
      <div class="row" role="group" aria-label="Kind">
        <span class="label">kind</span>
        <div class="seg">
          {#each ['stroke', 'outline'] as k (k)}
            <button aria-pressed={m.kind === k} onclick={() => written('kind') && set(written('kind')!.node, 0, k, `Make ${o!.definition} ${k}`, false)}>{k}</button>
          {/each}
        </div>
      </div>
      <p class="note">Click a glyph to draw it; its advance and kerning are here once it is open.</p>
    </section>
    <section aria-labelledby="export">
      <h3 id="export">Export</h3>
      {#if styles.length}
        <div class="checks" role="group" aria-label="Styles">
          {#each styles as s (s)}
            <label><input type="checkbox" value={s} bind:group={chosen} /> {s}</label>
          {/each}
        </div>
      {/if}
      <div class="seg" role="group" aria-label="Format">
        <button aria-pressed={!ufo} onclick={() => (ufo = false)}>OpenType (.ttf)</button>
        <button aria-pressed={ufo} onclick={() => (ufo = true)}>UFO</button>
      </div>
      <button class="primary" onclick={exportFont} disabled={!inApp} title={inApp ? '' : 'Fonts are exported in the studio’s app'}>Export…</button>
      {#if exported}<p class="note" role="status">{exported}</p>{/if}
    </section>
  {:else}
    <h2>{glyph.name === ' ' ? 'space' : glyph.name} <small>glyph of {o?.definition}</small></h2>
    <section aria-labelledby="advance">
      <h3 id="advance">Advance</h3>
      <label class="row">
        <span class="label">advance</span>
        <input type="text" value={advance} onchange={(e) => setAdvance(e.currentTarget.value)} />
      </label>
      <p class="note">Or drag the advance line on the canvas.</p>
    </section>
    <section aria-labelledby="kerning">
      <h3 id="kerning">Kerning</h3>
      {#each pairs as p (p.left + '|' + p.right)}
        {@const key = `${p.left}|${p.right}`}
        {@const s = shown[key]}
        {@const by = dragging?.key === key ? dragging.by : (p.value ?? 0)}
        <div class="pair" role="group" aria-label="Kern {p.left} {p.right}">
          <div class="pair-head">
            <code>{p.left}{p.right}</code>
            <input type="text" aria-label="Space between {p.left} and {p.right}" value={dragging?.key === key ? String(by) : p.by} onchange={(e) => set(p.node, 2, e.currentTarget.value, `Kern ${p.left}${p.right}`)} />
          </div>
          {#if s && p.value !== null}
            <div
              class="preview"
              role="slider"
              tabindex="0"
              aria-label="Drag {p.right} to kern it"
              aria-valuenow={by}
              onpointerdown={(e) => down(e, key, p.value ?? 0)}
              onpointermove={(e) => move(e, s.k)}
              onpointerup={() => up(p)}
              onkeydown={(e) => {
                const step = m.em / 100;
                if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
                  e.preventDefault();
                  set(p.node, 2, String(clean((p.value ?? 0) + (e.key === 'ArrowRight' ? step : -step))), `Kern ${p.left}${p.right}`);
                }
              }}
            >
              <span class="glyph"><IsoIcon icon={s.left.ir} /></span>
              <span class="glyph right" style="left: {(s.advance + by) * s.k + 4}px"><IsoIcon icon={s.right.ir} /></span>
            </div>
          {/if}
        </div>
      {:else}
        <p class="note">{glyph.name} is kerned with no glyph.</p>
      {/each}
      {#if others.length}
        <div class="add">
          <select aria-label="Side" bind:value={side}>
            <option value="after">{glyph.name} then</option>
            <option value="before">before {glyph.name}</option>
          </select>
          <select aria-label="Other glyph" bind:value={other}>
            <option value="">a glyph…</option>
            {#each others as g (g.name)}<option value={g.name}>{g.name === ' ' ? 'space' : g.name}</option>{/each}
          </select>
          <button onclick={addPair} disabled={!other}>Kern</button>
        </div>
      {/if}
    </section>
  {/if}
</div>

<style>
  .font {
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
    grid-template-columns: 64px 1fr;
    align-items: center;
    gap: 8px;
  }
  .label {
    color: var(--muted);
    font: 12px var(--font-mono);
  }
  input[type='text'],
  select {
    width: 100%;
    box-sizing: border-box;
    font: 12px var(--font-mono);
  }
  .note {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .seg {
    display: inline-flex;
    gap: 2px;
  }
  button {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .seg button[aria-pressed='false'] {
    background: transparent;
    color: var(--muted);
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-fg);
    align-self: flex-start;
  }
  .checks {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 12px;
  }
  .pair {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 6px 0;
    border-top: 1px solid var(--line);
  }
  .pair-head {
    display: grid;
    grid-template-columns: 1fr 72px;
    gap: 8px;
    align-items: center;
  }
  .preview {
    position: relative;
    height: 96px;
    padding: 4px;
    border-radius: 6px;
    background: var(--canvas);
    cursor: ew-resize;
    overflow: hidden;
    touch-action: none;
  }
  .glyph {
    position: absolute;
    left: 4px;
    top: 4px;
  }
  .glyph.right {
    outline: 1px dashed var(--accent);
  }
  .add {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 4px;
  }
</style>
