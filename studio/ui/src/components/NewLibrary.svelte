<!--
  A new library (docs/studio-interface.md, A library and the disk): its
  name, the folder it goes in, and what it starts from — a lexicon with
  the standard tokens, a palette and one box, a copy of another library,
  or a library exported as a zip (#112). It writes the folder and opens
  it.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';

  interface Props {
    studio: Studio;
    onclose: () => void;
  }

  let { studio, onclose }: Props = $props();

  let name = $state('icons');
  let parent = $state('');
  let from = $state<'empty' | 'copy' | 'zip'>('empty');
  let of = $state('');
  let failed = $state('');

  const sep = $derived(parent.includes('\\') ? '\\' : '/');
  const path = $derived(parent && name.trim() ? `${parent.replace(/[\\/]+$/, '')}${sep}${name.trim()}` : '');

  async function pick(set: (p: string) => void) {
    const p = await studio.backend.pickFolder();
    if (p) set(p);
  }

  async function pickZip() {
    const p = await studio.backend.pickFile('Choose an exported library', ['zip'], 'Zip');
    if (p) of = p;
  }

  async function make() {
    failed = '';
    try {
      await studio.create(path, from === 'empty' ? { from: 'empty' } : { from, of });
      onclose();
    } catch (e) {
      failed = String(e instanceof Error ? e.message : e);
    }
  }

  /** The dialog, focused when it opens. */
  let box: HTMLDivElement | undefined = $state();
  $effect(() => {
    box?.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="scrim" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="new-title" tabindex="-1" bind:this={box}>
    <h2 id="new-title">New library</h2>
    <label class="row"><span>Name</span><input type="text" bind:value={name} /></label>
    <div class="row">
      <span>In</span>
      <span class="pick">
        <input type="text" bind:value={parent} placeholder="A folder" aria-label="Folder it goes in" />
        <button onclick={() => pick((p) => (parent = p))}>Choose…</button>
      </span>
    </div>
    <div class="row" role="radiogroup" aria-label="Start from">
      <span>Start from</span>
      <span class="choices">
        <label><input type="radio" bind:group={from} value="empty" /> A lexicon, a palette and one box</label>
        <label><input type="radio" bind:group={from} value="copy" /> A copy of a library</label>
        <label><input type="radio" bind:group={from} value="zip" /> A library exported as a zip</label>
      </span>
    </div>
    {#if from === 'zip'}
      <div class="row">
        <span>Zip</span>
        <span class="pick">
          <input type="text" bind:value={of} placeholder="The exported zip" aria-label="Exported zip" />
          <button onclick={pickZip}>Choose…</button>
        </span>
      </div>
    {/if}
    {#if from === 'copy'}
      <div class="row">
        <span>Library</span>
        <span class="pick">
          <input type="text" bind:value={of} placeholder="The library to copy" aria-label="Library to copy" />
          <button onclick={() => pick((p) => (of = p))}>Choose…</button>
        </span>
      </div>
    {/if}
    {#if path}<p class="note">It is written in <code>{path}</code>.</p>{/if}
    {#if failed}<p class="failed" role="alert">{failed}</p>{/if}
    <div class="actions">
      <button onclick={onclose}>Cancel</button>
      <button class="primary" onclick={make} disabled={!path || (from !== 'empty' && !of.trim())}>Make it</button>
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
    grid-template-columns: 88px 1fr;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .pick {
    display: flex;
    gap: 6px;
  }
  .choices {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--fg);
  }
  input[type='text'] {
    flex: 1;
    min-width: 0;
  }
  button {
    padding: 5px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    font-size: 12px;
    cursor: pointer;
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
  .note {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .failed {
    margin: 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
