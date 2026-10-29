<!--
  The Preview tab (docs/studio-interface.md, Preview): the open icon as
  isolex draws it at 16, 24, 32, 64 and 128 px, each on a light and a
  dark ground, with the palette's colors for each.
-->
<script lang="ts">
  import type { IrIcon } from '../lib/isolex/ir';
  import IsoIcon from './IsoIcon.svelte';

  interface Props {
    icon: IrIcon | null;
  }

  let { icon }: Props = $props();

  const SIZES = [16, 24, 32, 64, 128];
</script>

{#if icon}
  <div class="preview">
    {#each [['iso-light', 'Light'], ['iso-dark', 'Dark']] as [look, name] (look)}
      <section class="ground {look}" aria-label="{icon.name} on a {name.toLowerCase()} ground">
        <h3>{name}</h3>
        <ul>
          {#each SIZES as px (px)}
            <li>
              <span class="frame" style="width: {px}px; height: {px}px"><IsoIcon {icon} height={fit(icon, px)} label="{icon.name} at {px} px" /></span>
              <span class="px">{px}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
{:else}
  <p class="empty">The open icon at real sizes appears here.</p>
{/if}

<script module lang="ts">
  /** The height that fits an icon in a `px` square, as an icon is sized
      in a list: by its longer side. */
  export function fit(icon: IrIcon, px: number): number {
    const [, , w, h] = icon.viewbox;
    return w > h ? (px * h) / w : px;
  }
</script>

<style>
  .preview {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
  }
  .ground {
    border-radius: 10px;
    padding: 10px 12px 12px;
    border: 1px solid var(--line);
  }
  .ground.iso-light {
    background: #ffffff;
    color: #3e4c59;
  }
  .ground.iso-dark {
    background: #15181d;
    color: #c4ccd5;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  ul {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 14px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .frame {
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .px {
    font: 11px var(--font-mono);
  }
  .empty {
    margin: 12px;
    color: var(--muted);
  }
</style>
