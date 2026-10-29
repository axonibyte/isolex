<!--
  Problems (#70): the compiler's errors and warnings for the whole
  library, from the status bar, by file; a click opens its place in the
  Source tab.
-->
<script lang="ts">
  import type { Diag } from '../lib/backend';
  import type { Studio } from '../lib/state.svelte';

  interface Props {
    studio: Studio;
  }

  let { studio }: Props = $props();
  let open = $state(false);
  let panel: HTMLDivElement | undefined = $state();
  /** Where the panel opens: above the button, on the window. */
  let place = $state({ left: 8, bottom: 40 });
  function toggle(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    place = { left: Math.max(8, r.left - 8), bottom: Math.max(8, window.innerHeight - r.top + 6) };
    open = !open;
  }

  const compiled = $derived(studio.view.compiled);
  const errors = $derived(compiled?.errors.length ?? 0);
  const warnings = $derived(compiled?.warnings.length ?? 0);
  /** Each file's problems, errors first, in the order of their lines. */
  const byFile = $derived.by(() => {
    const all: { d: Diag; severity: 'error' | 'warning' }[] = [
      ...(compiled?.errors ?? []).map((d) => ({ d, severity: 'error' as const })),
      ...(compiled?.warnings ?? []).map((d) => ({ d, severity: 'warning' as const })),
    ];
    const files = new Map<string, typeof all>();
    for (const p of all) files.set(p.d.file, [...(files.get(p.d.file) ?? []), p]);
    for (const list of files.values()) list.sort((a, b) => (a.severity === b.severity ? a.d.line - b.d.line || a.d.col - b.d.col : a.severity === 'error' ? -1 : 1));
    return [...files.entries()];
  });

  function reveal(d: Diag, severity: 'error' | 'warning') {
    open = false;
    studio.reveal(d, severity);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) {
      e.stopPropagation();
      open = false;
    }
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (open && panel && !panel.parentElement!.contains(e.target as Node)) open = false;
  }}
/>

<span class="problems">
  {#if !compiled}<span class="dot"></span>Compiling…
  {:else}
    <button class="count" aria-expanded={open} aria-haspopup="dialog" onclick={toggle} disabled={!errors && !warnings}>
      {#if errors}<span class="dot bad"></span>{errors} error{errors === 1 ? '' : 's'}
      {:else}<span class="dot ok"></span>Compiled revision {compiled.revision}{/if}
      {#if warnings}<span class="warn"> · {warnings} warning{warnings === 1 ? '' : 's'}</span>{/if}
    </button>
  {/if}
  {#if open}
    <div class="panel" role="dialog" aria-label="Problems" tabindex="-1" style="left: {place.left}px; bottom: {place.bottom}px" bind:this={panel} {onkeydown}>
      {#each byFile as [file, list] (file)}
        <section aria-label={file || 'the library'}>
          <h3>{file || 'the library'}</h3>
          <ul>
            {#each list as p, i (i)}
              <li>
                <button onclick={() => reveal(p.d, p.severity)}>
                  <span class="sev {p.severity}" aria-label={p.severity}>{p.severity === 'error' ? '✕' : '▲'}</span>
                  <span class="at">{p.d.line}:{p.d.col}</span>
                  <span class="msg">{p.d.message}</span>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    </div>
  {/if}
</span>

<style>
  .problems {
    position: relative;
  }
  .count {
    border: 0;
    background: transparent;
    padding: 0;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .count:disabled {
    cursor: default;
  }
  /* over the window, above the status bar, which clips what it holds */
  .panel {
    position: fixed;
    white-space: normal;
    z-index: 10;
    width: min(520px, 90vw);
    max-height: 50vh;
    overflow: auto;
    padding: 8px;
    border-radius: 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  }
  h3 {
    margin: 4px 6px;
    font: 11px var(--font-mono);
    color: var(--muted);
  }
  ul {
    margin: 0 0 6px;
    padding: 0;
    list-style: none;
  }
  li button {
    display: grid;
    grid-template-columns: 14px 48px 1fr;
    gap: 6px;
    width: 100%;
    padding: 5px 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  li button:hover,
  li button:focus-visible {
    background: var(--raised);
  }
  .sev.error {
    color: var(--danger);
  }
  .sev.warning {
    color: var(--warn);
  }
  .at {
    font: 11px var(--font-mono);
    color: var(--muted);
  }
  .warn {
    color: var(--warn);
  }
  .dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    margin-right: 6px;
    background: var(--muted);
  }
  .dot.ok {
    background: var(--ok);
  }
  .dot.bad {
    background: var(--danger);
  }
</style>
