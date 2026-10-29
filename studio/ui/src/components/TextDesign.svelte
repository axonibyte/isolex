<!--
  The Design tab for a text (#84, docs/studio-interface.md, Flat: "font,
  style, size and alignment in Design"): its words, the library font they
  are set in and its styles, their size, where they are aligned, and the
  token every paint of their glyphs is drawn in instead. The words redraw
  as the font's glyphs are edited.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { Tools } from '../lib/tools.svelte';
  import type { Ask, NodeAt } from '../lib/backend';
  import { argText, propsOf } from '../lib/kdl';
  import { DEFAULT_PALETTE } from '../lib/theme';

  interface Props {
    studio: Studio;
    tools: Tools;
    node: NodeAt;
    /** The text node as written. */
    text: string;
  }

  let { studio, tools, node, text }: Props = $props();

  const written = $derived(propsOf(text));
  const words = $derived(argText(text, 2) ?? '');
  const fonts = $derived(studio.view.tree?.fonts ?? []);
  const font = $derived(written.font ?? '');
  /** The styles of the font it is set in. */
  const styles = $derived((fonts.find((f) => f.name === font)?.children ?? []).map((r) => r.variant ?? '').filter(Boolean));
  const on = $derived(new Set((written.style ?? '').split(/\s+/).filter(Boolean)));
  const tokens = $derived(studio.view.lexicon.tokens.length ? studio.view.lexicon.tokens : Object.keys(studio.view.palette ?? DEFAULT_PALETTE));

  function ask(make: (n: NodeAt) => Ask, label: string) {
    void tools.change(node, label, [0, 0], [make]);
  }
  /** A property set, or taken off when it is what isolex does unasked. */
  function prop(key: string, value: string | number, unasked: string | number | null = null) {
    ask((n) => (value === unasked || value === '' ? { kind: 'remove-prop', node: n, key } : { kind: 'set-prop', node: n, key, value }), `Set the ${key} of text in ${studio.editing}`);
  }
  /** The words typed: read as they are typed, the edit made after. */
  function setWords(value: string) {
    ask((n) => ({ kind: 'set-arg', node: n, index: 2, value }), `Set the words of text in ${studio.editing}`);
  }
  function toggle(style: string) {
    const next = new Set(on);
    if (!next.delete(style)) next.add(style);
    prop('style', [...next].join(' '), '');
  }
</script>

<section aria-labelledby="words">
  <h3 id="words">Text</h3>
  <label class="row">
    <span class="label">Words</span>
    <input type="text" value={words} onchange={(e) => setWords(e.currentTarget.value)} />
  </label>
  <label class="row">
    <span class="label">Font</span>
    <select value={font} onchange={(e) => prop('font', e.currentTarget.value)}>
      {#each fonts as f (f.name)}<option value={f.name}>{f.name}</option>{/each}
    </select>
  </label>
  <label class="row">
    <span class="label">Size</span>
    <input type="number" min="0.5" step="0.5" value={Number(written.size ?? 8)} onchange={(e) => Number(e.currentTarget.value) > 0 && prop('size', Number(e.currentTarget.value))} />
  </label>
  {#if styles.length}
    <div class="row" role="group" aria-label="Styles">
      <span class="label">Style</span>
      <div class="seg">
        {#each styles as s (s)}<button aria-pressed={on.has(s)} onclick={() => toggle(s)}>{s}</button>{/each}
      </div>
    </div>
  {/if}
  <div class="row" role="group" aria-label="Align">
    <span class="label">Align</span>
    <div class="seg">
      {#each ['start', 'middle', 'end'] as a (a)}
        <button aria-pressed={(written.anchor ?? 'start') === a} onclick={() => prop('anchor', a, 'start')}>{a}</button>
      {/each}
    </div>
  </div>
  <div class="row paint" role="group" aria-label="Fill">
    <span class="label">Fill</span>
    <div class="swatches">
      <button class="swatch drawn" aria-label="Fill: as drawn" title="as the font draws it" aria-pressed={!written.fill} onclick={() => prop('fill', '')}></button>
      {#each tokens as t (t)}
        <button class="swatch" style="--c: var(--iso-{t})" aria-label="Fill: {t}" title={t} aria-pressed={written.fill === t} onclick={() => prop('fill', t)}></button>
      {/each}
      <span class="token">{written.fill ?? 'as drawn'}</span>
    </div>
  </div>
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
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
    grid-template-columns: 56px 1fr;
    align-items: center;
    gap: 8px;
  }
  .row.paint {
    align-items: start;
  }
  .label {
    color: var(--muted);
    font-size: 12px;
  }
  input,
  select {
    width: 100%;
    box-sizing: border-box;
  }
  .seg {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
  }
  .seg button {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }
  .seg button[aria-pressed='true'] {
    background: var(--raised);
    color: var(--fg);
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
  .swatch.drawn {
    background: linear-gradient(135deg, var(--iso-ink, #333) 50%, var(--iso-accent, #d64545) 50%);
  }
  .swatch[aria-pressed='true'] {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .token {
    flex-basis: 100%;
    font: 11px var(--font-mono);
    color: var(--muted);
  }
</style>
