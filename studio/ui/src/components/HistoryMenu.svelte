<!--
  Undo History (docs/studio-interface.md: one undo history covers the
  library): beside undo and redo, the steps done, oldest first, and those
  undone after them; a click goes back or forward to that step, as that
  many undos or redos.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';

  interface Props {
    studio: Studio;
  }

  let { studio }: Props = $props();
  let open = $state(false);
  let panel: HTMLDivElement | undefined = $state();

  const past = $derived(studio.view.library?.history ?? []);
  const future = $derived(studio.view.library?.future ?? []);

  async function go(steps: number) {
    open = false;
    if (steps) await studio.travel(steps);
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

<div class="history">
  <button class="icon-btn" aria-label="History" title="Undo history" aria-expanded={open} aria-haspopup="dialog" onclick={() => (open = !open)}
    ><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M3.5 10a6.5 6.5 0 1 0 2-4.7" /><path d="M3 3v3.5h3.5M10 6.5V10l2.5 2" /></svg></button
  >
  {#if open}
    <div class="panel" role="dialog" aria-label="Undo history" tabindex="-1" bind:this={panel} {onkeydown}>
      <ol>
        <li><button aria-current={past.length === 0 ? 'step' : undefined} onclick={() => go(-past.length)}>Opened</button></li>
        {#each past as label, i (i)}
          <li><button aria-current={i === past.length - 1 ? 'step' : undefined} onclick={() => go(i + 1 - past.length)}>{label}</button></li>
        {/each}
        {#each future as label, j (j)}
          <li><button class="undone" onclick={() => go(j + 1)}>{label}</button></li>
        {/each}
      </ol>
      {#if !past.length && !future.length}<p class="note">Nothing done yet.</p>{/if}
    </div>
  {/if}
</div>

<style>
  .history {
    position: relative;
  }
  .panel {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 10;
    min-width: 260px;
    max-height: 60vh;
    overflow: auto;
    padding: 6px;
    border-radius: 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li button {
    display: block;
    width: 100%;
    padding: 5px 9px;
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
  li button[aria-current='step'] {
    color: var(--accent);
    font-weight: 600;
  }
  li button.undone {
    color: var(--muted);
    font-style: italic;
  }
  .note {
    margin: 4px 9px;
    color: var(--muted);
    font-size: 12px;
  }
</style>
