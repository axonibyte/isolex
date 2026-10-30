<!--
  Preferences (#119, docs/studio-interface.md, Preferences), with
  Inkscape's Keyboard Shortcuts page: every command and its keys, by
  group, filtered by name or key. A key is changed by choosing it and
  pressing the new one: Escape cancels, Backspace leaves the command
  without one. A key another command has is refused, saying which, as is
  one kept for what it does everywhere. Each change is kept per person.
-->
<script lang="ts">
  import { chord, type Command } from '../lib/keymap';
  import type { Shortcuts } from '../lib/shortcuts.svelte';

  interface Props {
    shortcuts: Shortcuts;
    onclose: () => void;
  }

  let { shortcuts, onclose }: Props = $props();

  let query = $state('');
  /** The command whose key is being changed. */
  let changing = $state<string | null>(null);
  /** Why the last change was refused, or what it did. */
  let said = $state<{ text: string; refused: boolean } | null>(null);

  const GROUPS: Command['group'][] = ['File', 'Edit', 'View', 'Tools'];
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return shortcuts.commands.filter((c) => !q || c.label.toLowerCase().includes(q) || shortcuts.keys(c.id).some((k) => k.toLowerCase().includes(q)));
  });

  function pressed(e: KeyboardEvent, c: Command) {
    // the press is the new key: nothing else hears it
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      changing = null;
      return;
    }
    const clear = e.key === 'Backspace' && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey;
    const key = clear ? null : chord(e);
    // a modifier alone: the key is still to come
    if (!clear && key === null) return;
    const why = shortcuts.set(c.id, key);
    said = why ? { text: why, refused: true } : { text: key ? `${c.label} is ${key} now` : `${c.label} has no key now`, refused: false };
    if (!why) changing = null;
  }

  function resetOne(c: Command) {
    const why = shortcuts.reset(c.id);
    said = why ? { text: why, refused: true } : { text: `${c.label} is ${shortcuts.keys(c.id).join(', ')} again`, refused: false };
  }

  let search: HTMLInputElement | undefined = $state();
  $effect(() => {
    search?.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !changing && onclose()} />

<div class="scrim" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="prefs-title">
    <h2 id="prefs-title">Preferences</h2>
    <h3 id="keys-title">Keyboard shortcuts</h3>
    <div class="bar">
      <input type="search" placeholder="A command or a key…" aria-label="Find a command or a key" bind:value={query} bind:this={search} />
      <button onclick={() => ((said = { text: 'Every command has its own keys again', refused: false }), shortcuts.resetAll())}>Reset all</button>
    </div>
    <p class="said" class:refused={said?.refused} role={said?.refused ? 'alert' : 'status'}>{said?.text ?? 'Choose a key, then press the new one · Escape cancels · Backspace leaves it without one'}</p>
    <div class="list">
      {#each GROUPS as group (group)}
        {@const mine = shown.filter((c) => c.group === group)}
        {#if mine.length}
          <table aria-labelledby="keys-{group}">
            <caption id="keys-{group}">{group}</caption>
            <thead>
              <tr><th scope="col">Command</th><th scope="col">Shortcut</th><th scope="col"><span class="sr">Reset</span></th></tr>
            </thead>
            <tbody>
              {#each mine as c (c.id)}
                {@const keys = shortcuts.keys(c.id)}
                <tr>
                  <th scope="row" class:changed={shortcuts.changed(c.id)}>{c.label}</th>
                  <td>
                    <button
                      class="key"
                      aria-pressed={changing === c.id}
                      aria-label="Shortcut for {c.label}: {changing === c.id ? 'Press a key…' : keys.join(' or ') || 'none'}"
                      onclick={() => {
                        changing = changing === c.id ? null : c.id;
                        said = null;
                      }}
                      onkeydown={(e) => changing === c.id && pressed(e, c)}
                      onblur={() => changing === c.id && (changing = null)}
                    >
                      {#if changing === c.id}Press a key…{:else if keys.length}{#each keys as k, i (k)}{#if i}<span class="or">{' or '}</span>{/if}<kbd>{k}</kbd>{/each}{:else}<span class="none">none</span>{/if}
                    </button>
                  </td>
                  <td>
                    {#if shortcuts.changed(c.id)}<button class="reset" onclick={() => resetOne(c)} aria-label="Reset {c.label}">Reset</button>{/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      {/each}
      {#if !shown.length}<p class="note">No command or key matches “{query.trim()}”.</p>{/if}
    </div>
    <div class="actions">
      <button onclick={onclose}>Close</button>
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
    padding-top: 8vh;
    background: rgb(0 0 0 / 0.35);
  }
  .dialog {
    width: min(560px, 92vw);
    max-height: 80vh;
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
  h3 {
    margin: 0;
    font-size: 13px;
  }
  .bar {
    display: flex;
    gap: 8px;
  }
  input {
    flex: 1;
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--raised);
    color: var(--fg);
    font-size: 13px;
  }
  button {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    cursor: pointer;
  }
  .said {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .said.refused {
    padding: 6px 8px;
    border-radius: 6px;
    color: var(--fg);
    background: color-mix(in srgb, var(--danger) 16%, transparent);
  }
  .list {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  caption {
    text-align: left;
    padding-bottom: 4px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  thead th {
    text-align: left;
    font-weight: 400;
    color: var(--muted);
    padding: 2px 6px;
  }
  tbody th {
    text-align: left;
    font-weight: 400;
    padding: 2px 6px;
  }
  tbody th.changed {
    font-weight: 600;
  }
  td {
    padding: 2px 6px;
  }
  td:last-child {
    width: 64px;
    text-align: right;
  }
  .key {
    min-width: 120px;
    text-align: left;
    background: transparent;
  }
  .key[aria-pressed='true'] {
    border-color: var(--accent);
  }
  kbd {
    font: 11px var(--font-mono);
    padding: 0 4px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--raised);
  }
  .none,
  .or,
  .note {
    color: var(--muted);
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
