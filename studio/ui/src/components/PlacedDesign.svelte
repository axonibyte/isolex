<!--
  The Design tab with an insert or a badge selected (#116,
  docs/studio-interface.md, Design): what it places, opened from here;
  for an insert, the fragment's params, each as the insert sets it or the
  fragment's default; for a badge, its corner and scale. Each change is
  one undo step.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { Tools } from '../lib/tools.svelte';
  import type { Ask, NodeAt, Outline } from '../lib/backend';
  import { argText, propsOf } from '../lib/kdl';

  interface Props {
    studio: Studio;
    tools: Tools;
    node: NodeAt;
    kind: 'insert' | 'badge';
    /** The node's text, as the source has it now. */
    text: string;
  }

  let { studio, tools, node, kind, text }: Props = $props();

  const target = $derived(argText(text, 0) ?? '');
  const written = $derived(propsOf(text));

  /** The fragment's params, as it declares them. */
  let params = $state<Outline['params']>([]);
  $effect(() => {
    const name = target;
    void studio.view.library?.revision;
    if (kind !== 'insert' || !name) {
      params = [];
      return;
    }
    void studio.backend.outline(name).then((o) => (params = o?.params ?? []));
  });

  function ask(make: (n: NodeAt) => Ask, label: string) {
    void tools.change(node, label, [0, 0], [make]);
  }

  /** A value typed for a property: cleared, a number, or an expression as
      written; cleared, the property goes and its default applies. */
  function setValue(key: string, typed: string) {
    const t = typed.trim();
    if (t === (written[key] ?? '')) return;
    const what = `${kind} ${target} in ${studio.editing}`;
    if (!t) return ask((n) => ({ kind: 'remove-prop', node: n, key }), `Reset ${key} of ${what}`);
    const value = /^-?(\d+\.?\d*|\.\d+)$/.test(t) ? Number(t) : t;
    ask((n) => ({ kind: 'set-prop', node: n, key, value }), `Set ${key} of ${what}`);
  }

  function open() {
    if (kind === 'badge') studio.openIcon(target);
    else studio.edit(target);
  }

  const CORNERS = [
    ['nw', 'Top left'],
    ['ne', 'Top right'],
    ['sw', 'Bottom left'],
    ['se', 'Bottom right'],
  ] as const;
</script>

<section aria-labelledby="placed">
  <h3 id="placed">{kind === 'insert' ? 'Fragment' : 'Badge'}</h3>
  <div class="head">
    <code class="name">{target}</code>
    <button onclick={open}>Open {target}</button>
  </div>
  {#if kind === 'insert'}
    {#each params as p (p.name)}
      <label class="row">
        <span class="label">{p.name}</span>
        <input type="text" spellcheck="false" value={written[p.name] ?? ''} placeholder={p.default} onchange={(e) => setValue(p.name, e.currentTarget.value)} />
      </label>
    {:else}
      <p class="note">{target} has no params.</p>
    {/each}
    <p class="note">A param left empty is the fragment’s default.</p>
  {:else}
    <label class="row">
      <span class="label">Corner</span>
      <select value={written.corner ?? 'ne'} onchange={(e) => (e.currentTarget.value === 'ne' ? ask((n) => ({ kind: 'remove-prop', node: n, key: 'corner' }), `Move badge ${target} in ${studio.editing}`) : setValue('corner', e.currentTarget.value))}>
        {#each CORNERS as [c, label] (c)}<option value={c}>{label}</option>{/each}
      </select>
    </label>
    <label class="row">
      <span class="label">Scale</span>
      <input type="text" spellcheck="false" value={written.scale ?? ''} placeholder="0.5" onchange={(e) => setValue('scale', e.currentTarget.value)} />
    </label>
  {/if}
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
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .name {
    font: 12px var(--font-mono);
  }
  .label {
    color: var(--muted);
    font-size: 12px;
  }
  .note {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  input,
  select {
    width: 100%;
    box-sizing: border-box;
    font-size: 12px;
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
</style>
