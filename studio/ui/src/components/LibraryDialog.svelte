<!--
  The Library (#72): the library's folder as its files, each with the
  definitions written in it, as VS Code's Explorer shows a folder. A click
  opens a definition; F2 or a double-click renames it (every reference
  rewritten) or a file (moved); a definition dragged onto a file, or sent
  there from its row, moves there; new files and definitions are made
  here. Each change is one undo step; a name taken is refused, offering
  one that is free.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { LibraryOp } from '../lib/backend';

  interface Props {
    studio: Studio;
    onclose: () => void;
  }

  let { studio, onclose }: Props = $props();

  const files = $derived(studio.view.library?.files ?? []);
  const defs = $derived(studio.view.graph?.definitions ?? []);
  const byFile = $derived(files.map((f) => ({ file: f, defs: defs.filter((d) => d.file === f) })));
  const KINDS = ['icon', 'base', 'fragment', 'axis', 'font'];

  let message = $state('');
  /** What is being renamed: a definition or a file, by name or path. */
  let renaming = $state<{ kind: 'definition' | 'file'; name: string } | null>(null);
  let draft = $state('');
  let dragging = $state<string | null>(null);
  let over = $state<string | null>(null);

  let newFile = $state('');
  let newDef = $state({ kind: 'icon', name: '', file: '' });
  // a new definition goes in the first file, unless another is chosen
  $effect(() => {
    if (!files.includes(newDef.file) && files[0]) newDef.file = files[0];
  });

  async function run(op: LibraryOp, done = '') {
    const answer = await studio.backend.libraryOp(op);
    message = answer.kind === 'refused' ? answer.reason : done;
    return answer.kind === 'applied';
  }

  function startRename(kind: 'definition' | 'file', name: string) {
    renaming = { kind, name };
    draft = name;
  }

  async function finishRename() {
    const r = renaming;
    renaming = null;
    const to = draft.trim();
    if (!r || !to || to === r.name) return;
    if (r.kind === 'definition') {
      if (await run({ op: 'rename-definition', from: r.name, to }, `Renamed ${r.name} to ${to}, and every reference to it`) && studio.editing === r.name) studio.edit(to);
    } else await run({ op: 'move-file', from: r.name, to }, `Moved ${r.name} to ${to}`);
  }

  async function moveTo(name: string, file: string) {
    if (!file || defs.find((d) => d.name === name)?.file === file) return;
    await run({ op: 'move-definition', name, to: file }, `Moved ${name} to ${file}`);
  }

  function open(name: string) {
    const d = defs.find((x) => x.name === name);
    if (d?.emits.length) studio.openIcon(d.emits[0]!);
    else studio.edit(name);
    onclose();
  }

  async function makeFile() {
    const path = newFile.trim().endsWith('.ilx') ? newFile.trim() : `${newFile.trim()}.ilx`;
    if (!newFile.trim()) return;
    if (await run({ op: 'new-file', path }, `Made ${path}`)) newFile = '';
  }

  async function makeDefinition() {
    const name = newDef.name.trim();
    const file = newDef.file || files[0] || 'icons.ilx';
    if (!name) return;
    if (await run({ op: 'new-definition', file, kind: newDef.kind, name }, `Made ${newDef.kind} ${name} in ${file}`)) {
      newDef.name = '';
      studio.edit(name);
    }
  }

  function onrenamekey(e: KeyboardEvent) {
    if (e.key === 'Enter') void finishRename();
    else if (e.key === 'Escape') {
      e.stopPropagation();
      renaming = null;
    }
  }

  /** The dialog, focused when it opens. */
  let box: HTMLDivElement | undefined = $state();
  $effect(() => {
    box?.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !renaming && onclose()} />

<div class="scrim" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="library-title" tabindex="-1" bind:this={box}>
    <div class="head">
      <h2 id="library-title">Library</h2>
      <span class="root">{studio.view.library?.root}</span>
      <button class="close" onclick={onclose} aria-label="Close">×</button>
    </div>
    <ul class="files" aria-label="Files">
      {#each byFile as f (f.file)}
        <li
          class="file"
          class:over={over === f.file}
          ondragover={(e) => {
            if (dragging) {
              e.preventDefault();
              over = f.file;
            }
          }}
          ondragleave={() => over === f.file && (over = null)}
          ondrop={(e) => {
            e.preventDefault();
            over = null;
            if (dragging) void moveTo(dragging, f.file);
            dragging = null;
          }}
        >
          <div class="row">
            {#if renaming?.kind === 'file' && renaming.name === f.file}
              <!-- svelte-ignore a11y_autofocus -->
              <input aria-label="New path for {f.file}" bind:value={draft} autofocus onkeydown={onrenamekey} onblur={finishRename} />
            {:else}
              <button class="name file-name" ondblclick={() => startRename('file', f.file)} onkeydown={(e) => e.key === 'F2' && startRename('file', f.file)} title="F2 or a double-click moves it">{f.file}</button>
              <button class="act" onclick={() => startRename('file', f.file)} aria-label="Move or rename {f.file}">Rename</button>
            {/if}
          </div>
          <ul aria-label="Definitions in {f.file}">
            {#each f.defs as d (d.name)}
              <li class="def" draggable="true" ondragstart={() => (dragging = d.name)} ondragend={() => (dragging = null)}>
                {#if renaming?.kind === 'definition' && renaming.name === d.name}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input aria-label="New name for {d.name}" bind:value={draft} autofocus onkeydown={onrenamekey} onblur={finishRename} />
                {:else}
                  <span class="kind">{d.kind.toLowerCase()}</span>
                  <button class="name" aria-current={studio.editing === d.name ? 'true' : undefined} onclick={() => open(d.name)} ondblclick={() => startRename('definition', d.name)} onkeydown={(e) => e.key === 'F2' && startRename('definition', d.name)} title="Click opens it; F2 or a double-click renames it">{d.name}</button>
                  <button class="act" onclick={() => startRename('definition', d.name)} aria-label="Rename {d.name}">Rename</button>
                  {#if files.length > 1}
                    <select class="act" aria-label="Move {d.name} to" value="" onchange={(e) => moveTo(d.name, e.currentTarget.value)}>
                      <option value="">Move to…</option>
                      {#each files.filter((x) => x !== f.file) as other (other)}<option value={other}>{other}</option>{/each}
                    </select>
                  {/if}
                {/if}
              </li>
            {:else}
              <li class="empty">No definitions</li>
            {/each}
          </ul>
        </li>
      {/each}
    </ul>
    <div class="make">
      <div class="line" role="group" aria-label="New file">
        <input type="text" bind:value={newFile} placeholder="racks.ilx, or a path under the folder" aria-label="New file path" onkeydown={(e) => e.key === 'Enter' && makeFile()} />
        <button onclick={makeFile} disabled={!newFile.trim()}>New file</button>
      </div>
      <div class="line def-line" role="group" aria-label="New definition">
        <select bind:value={newDef.kind} aria-label="Kind">
          {#each KINDS as k (k)}<option value={k}>{k}</option>{/each}
        </select>
        <input type="text" bind:value={newDef.name} placeholder="Its name" aria-label="New definition name" onkeydown={(e) => e.key === 'Enter' && makeDefinition()} />
        <select bind:value={newDef.file} aria-label="In file">
          {#each files as f (f)}<option value={f}>{f}</option>{/each}
        </select>
        <button onclick={makeDefinition} disabled={!newDef.name.trim()}>New definition</button>
      </div>
    </div>
    <p class="message" role="status">{message}</p>
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
    width: min(640px, 94vw);
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 18px;
    border-radius: 12px;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 16px 48px rgb(0 0 0 / 0.4);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  .root {
    flex: 1;
    color: var(--muted);
    font: 11px var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .close {
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 18px;
    cursor: pointer;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .files {
    overflow: auto;
    min-height: 120px;
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 4px 0;
  }
  .file.over {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .row,
  .def {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 10px;
  }
  .def {
    padding-left: 26px;
  }
  .kind {
    width: 56px;
    color: var(--muted);
    font-size: 11px;
  }
  .name {
    flex: 1;
    padding: 3px 4px;
    border: 0;
    background: transparent;
    color: var(--fg);
    text-align: left;
    font: 12px var(--font-mono);
    cursor: pointer;
  }
  .file-name {
    font-weight: 600;
  }
  .name[aria-current='true'] {
    color: var(--accent);
  }
  button.act,
  select.act {
    padding: 2px 8px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--muted);
    font-size: 11px;
  }
  button.act:hover,
  select.act:hover {
    border-color: var(--line);
    color: var(--fg);
  }
  .empty {
    padding: 2px 26px;
    color: var(--muted);
    font-size: 11px;
  }
  .make {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .line {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 6px;
  }
  .def-line {
    grid-template-columns: 96px 1fr 1fr auto;
  }
  input,
  select {
    min-width: 0;
    box-sizing: border-box;
    font: 12px var(--font-mono);
  }
  button:not(.name):not(.close):not(.act) {
    padding: 4px 10px;
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
  .message {
    margin: 0;
    min-height: 1.3em;
    color: var(--muted);
    font-size: 12px;
  }
</style>
