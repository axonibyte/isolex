<!--
  Importing a font (#84; SPEC §10, Fonts): a TrueType, OpenType or WOFF
  file, the characters to keep, its em, and further styles from other
  files or a variable font's axes, read into the library as an isolex
  font in a file of its own, one undo step. A font whose license forbids
  it is refused, and the dialog says why.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';

  interface Props {
    studio: Studio;
    onclose: () => void;
  }

  let { studio, onclose }: Props = $props();

  let path = $state('');
  let name = $state('');
  let chars = $state<'latin1' | 'ascii' | 'all' | 'these'>('latin1');
  let these = $state('');
  let em = $state('');
  let styles = $state<{ style: string; source: string }[]>([]);
  let refused = $state('');
  let busy = $state(false);

  async function choose(set: (p: string) => void) {
    const picked = await studio.backend.pickFile('Choose a font', ['ttf', 'otf', 'woff']);
    if (picked) set(picked);
  }

  async function run() {
    busy = true;
    refused = '';
    const answer = await studio.backend.importFont({
      path,
      name: name.trim() || null,
      chars: chars === 'these' ? these : chars,
      em: em.trim() && Number(em) > 0 ? Number(em) : null,
      styles: styles.filter((s) => s.style.trim() && s.source.trim()).map((s) => [s.style.trim(), s.source.trim()]),
    });
    busy = false;
    if (answer.kind === 'refused') refused = answer.reason;
    else {
      studio.message = 'Font imported';
      onclose();
    }
  }
</script>

<div class="scrim" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="import-title" tabindex="-1" onkeydown={(e) => e.key === 'Escape' && onclose()}>
    <h2 id="import-title">Import a font</h2>
    <label class="row">
      <span>Font file</span>
      <span class="file">
        <input type="text" bind:value={path} placeholder="A .ttf, .otf or .woff file" aria-label="Font file" />
        <button onclick={() => choose((p) => (path = p))}>Choose…</button>
      </span>
    </label>
    <label class="row">
      <span>Name</span>
      <input type="text" bind:value={name} placeholder="The font’s own, unless given" />
    </label>
    <div class="row" role="group" aria-label="Characters">
      <span>Characters</span>
      <span class="chars">
        <select bind:value={chars} aria-label="Which characters">
          <option value="latin1">Latin-1</option>
          <option value="ascii">ASCII</option>
          <option value="all">All the font has</option>
          <option value="these">These…</option>
        </select>
        {#if chars === 'these'}<input type="text" bind:value={these} aria-label="The characters" placeholder="ABC0123" />{/if}
      </span>
    </div>
    <label class="row">
      <span>Em</span>
      <input type="text" bind:value={em} placeholder="The font’s own units" />
    </label>
    <div class="styles">
      <span class="label">Further styles</span>
      {#each styles as s, i (i)}
        <div class="style">
          <input type="text" bind:value={s.style} aria-label="Style {i + 1}" placeholder="weight.bold" />
          <input type="text" bind:value={s.source} aria-label="Source of style {i + 1}" placeholder="A font file, or wght:700" />
          <button onclick={() => choose((p) => (s.source = p))}>File…</button>
          <button aria-label="Remove style {i + 1}" onclick={() => (styles = styles.filter((_, j) => j !== i))}>×</button>
        </div>
      {/each}
      <button onclick={() => (styles = [...styles, { style: '', source: '' }])}>Add a style</button>
    </div>
    {#if refused}<p class="refused" role="alert">{refused}</p>{/if}
    <div class="actions">
      <button onclick={onclose}>Cancel</button>
      <button class="primary" onclick={run} disabled={!path.trim() || busy}>{busy ? 'Importing…' : 'Import'}</button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.45);
  }
  .dialog {
    width: min(520px, 92vw);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px;
    border-radius: 12px;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 16px 48px rgb(0 0 0 / 0.4);
  }
  h2 {
    margin: 0 0 4px;
    font-size: 15px;
  }
  .row {
    display: grid;
    grid-template-columns: 96px 1fr;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .file,
  .chars {
    display: flex;
    gap: 6px;
  }
  input,
  select {
    flex: 1;
    min-width: 0;
    box-sizing: border-box;
  }
  .styles {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
  }
  .label {
    color: var(--muted);
  }
  .style {
    display: grid;
    grid-template-columns: 1fr 1.4fr auto auto;
    gap: 6px;
  }
  button {
    padding: 5px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    font-size: 12px;
    cursor: pointer;
    justify-self: start;
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-fg);
  }
  .refused {
    margin: 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--fg);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
