<!--
  Snapping on the canvas (docs/studio-interface.md, The canvas), as
  Inkscape 1.2's snap popover: the step, and an Advanced list (#118) that
  switches each kind of target on or off, remembered per person.
-->
<script lang="ts">
  import type { Tools } from '../lib/tools.svelte';
  import { STEPS, TARGETS, stepLabel } from '../lib/snap';

  interface Props {
    tools: Tools;
  }

  let { tools }: Props = $props();
  let open = $state(false);
</script>

<div class="snap" role="group" aria-label="Snapping">
  <div class="row">
    <label>
      Snap
      <select bind:value={tools.step}>
        {#each STEPS as s (s)}<option value={s}>{stepLabel(s)}</option>{/each}
      </select>
    </label>
    <button class="more" aria-expanded={open} aria-controls="snap-targets" onclick={() => (open = !open)}>Advanced</button>
  </div>
  {#if open}
    <fieldset id="snap-targets">
      <legend>Snap to</legend>
      {#each TARGETS as [kind, label] (kind)}
        <label class="target">
          <input type="checkbox" checked={tools.snapOn[kind]} onchange={(e) => tools.setSnap(kind, e.currentTarget.checked)} />
          {label}
        </label>
      {/each}
    </fieldset>
  {/if}
</div>

<style>
  .snap {
    position: absolute;
    left: 12px;
    top: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 6px 4px 10px;
    border-radius: 8px;
    background: var(--panel);
    border: 1px solid var(--line);
    font-size: 12px;
    color: var(--muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  select,
  .more {
    background: var(--raised);
    color: var(--fg);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 2px 6px;
    font-size: 12px;
  }
  .more {
    cursor: pointer;
  }
  .more[aria-expanded='true'] {
    border-color: var(--accent);
  }
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin: 0;
    padding: 4px 4px 6px;
    border: 0;
    border-top: 1px solid var(--line);
  }
  legend {
    padding: 0;
    color: var(--muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .target {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--fg);
    cursor: pointer;
  }
</style>
