<!--
  Find (#117, docs/studio-interface.md, Find), as Inkscape's Find dialog
  (Ctrl+F): definitions, parts, tokens and text across the library, by a
  part of their name or words, listed by kind as they are written. A
  definition opens; anything else opens its definition and its place in
  the Source tab. Enter opens the first.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import { search, type Hit } from '../lib/find';

  interface Props {
    studio: Studio;
    onclose: () => void;
  }

  let { studio, onclose }: Props = $props();

  let query = $state('');
  let files = $state<{ file: string; text: string }[]>([]);
  // the library's files as they are now
  $effect(() => {
    const names = studio.view.library?.files ?? [];
    void studio.view.library?.revision;
    void Promise.all(names.map(async (file) => ({ file, text: (await studio.backend.source(file))?.text ?? '' }))).then((all) => (files = all));
  });
  const tokens = $derived(studio.view.lexicon.tokens.length ? studio.view.lexicon.tokens : Object.keys(studio.view.palette ?? {}));
  const hits = $derived(search(query, files, studio.view.graph, tokens));
  const KINDS: [Hit['kind'], string][] = [
    ['definition', 'Definitions'],
    ['part', 'Parts'],
    ['token', 'Tokens'],
    ['text', 'Text'],
  ];

  /** A definition opened as the Library dialog opens one: its first icon
      drawn, or itself edited. */
  function openDefinition(name: string) {
    const d = studio.view.graph?.definitions.find((x) => x.name === name);
    if (d?.emits.length) studio.openIcon(d.emits[0]!);
    else studio.edit(name);
  }

  function open(h: Hit) {
    if (h.definition) openDefinition(h.definition);
    if (h.kind !== 'definition') studio.place = { file: h.file, line: h.line, col: h.col, message: h.label, severity: null };
    onclose();
  }

  let input: HTMLInputElement | undefined = $state();
  $effect(() => {
    input?.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="scrim" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="find-title">
    <h2 id="find-title">Find</h2>
    <input
      type="search"
      placeholder="A name, a token or words…"
      aria-label="Find in the library"
      bind:value={query}
      bind:this={input}
      onkeydown={(e) => {
        if (e.key === 'Enter' && hits[0]) open(hits[0]);
      }}
    />
    {#if query.trim() && !hits.length}
      <p class="note">Nothing in the library matches “{query.trim()}”.</p>
    {/if}
    <div class="results">
      {#each KINDS as [kind, title] (kind)}
        {@const mine = hits.filter((h) => h.kind === kind)}
        {#if mine.length}
          <section aria-labelledby="find-{kind}">
            <h3 id="find-{kind}">{title} ({mine.length})</h3>
            <ul aria-labelledby="find-{kind}">
              {#each mine as h, i (i)}
                <li>
                  <button onclick={() => open(h)}>
                    <code>{h.label}</code>
                    <span class="where">{h.kind === 'definition' ? '' : `${h.definition ?? h.file} · `}{h.file}:{h.line}</span>
                  </button>
                </li>
              {/each}
            </ul>
          </section>
        {/if}
      {/each}
    </div>
    <div class="actions">
      <button class="close" onclick={onclose}>Close</button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: start center;
    padding-top: 12vh;
    background: rgb(0 0 0 / 0.35);
  }
  .dialog {
    width: min(560px, 92vw);
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    border-radius: 12px;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 16px 48px rgb(0 0 0 / 0.4);
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  input {
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--raised);
    color: var(--fg);
    font-size: 13px;
  }
  .results {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h3 {
    margin: 0 0 4px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li button {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    padding: 4px 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--fg);
    text-align: left;
    cursor: pointer;
  }
  li button:hover,
  li button:focus-visible {
    background: var(--raised);
  }
  code {
    font: 12px var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .where {
    flex: none;
    color: var(--muted);
    font-size: 11px;
  }
  .note {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
  .close {
    padding: 5px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    font-size: 12px;
    cursor: pointer;
  }
</style>
