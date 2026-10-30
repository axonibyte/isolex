<!--
  Export (#112), as Inkscape's Export dialog has it (Ctrl+Shift+E): what
  to export (the open icon, chosen icons or every icon), as what (SVG,
  PNG, the IR, or the library as a zip), on which grounds, and for PNG at
  which sizes. In the app the files go where the person picks: a folder
  for several, a file for one; outside it they are downloaded.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { inShell, type Ground } from '../lib/backend';
  import type { Studio } from '../lib/state.svelte';
  import { PNG_SIZES, fileName, inFolder, pngSize, rasterize, viewBoxOf, type Format } from '../lib/exporting';
  import { folderName } from '../lib/recent';

  interface Props {
    studio: Studio;
    onclose: () => void;
  }

  let { studio, onclose }: Props = $props();

  const all = $derived(studio.view.compiled?.icons ?? []);
  const open = $derived(studio.icon && all.includes(studio.icon) ? studio.icon : null);
  const library = $derived(folderName(studio.view.library?.root ?? '') || 'library');

  // the open icon, if there is one, as the dialog opens
  let scope = $state<'open' | 'chosen' | 'all'>(untrack(() => (open ? 'open' : 'all')));
  let chosen = $state<Set<string>>(new Set());
  let format = $state<Format>('svg');
  let grounds = $state<Set<Ground>>(new Set(['light']));
  let sizes = $state<Set<number>>(new Set([32]));
  let busy = $state(false);
  let done = $state('');
  let failed = $state('');

  /** The icons asked for, or none for every one. */
  const icons = $derived<string[] | null>(scope === 'all' ? null : scope === 'open' ? (open ? [open] : []) : all.filter((n) => chosen.has(n)));
  const count = $derived(icons?.length ?? all.length);
  const drawn = $derived(format === 'svg' || format === 'png');
  /** How many files it makes. */
  const files = $derived(format === 'ir' || format === 'zip' ? 1 : count * grounds.size * (format === 'png' ? sizes.size : 1));
  const ready = $derived(!busy && files > 0);

  function toggle<T>(set: Set<T>, v: T): Set<T> {
    const next = new Set(set);
    if (next.has(v)) next.delete(v);
    else next.add(v);
    return next;
  }

  /** Every file the export makes, named. */
  async function make(): Promise<{ name: string; data: string | Uint8Array }[]> {
    const backend = studio.backend;
    if (format === 'zip') return [{ name: `${library}.zip`, data: await backend.exportZip() }];
    if (format === 'ir') return [{ name: `${icons?.length === 1 ? icons[0] : library}.json`, data: await backend.exportIr(icons) }];
    const out: { name: string; data: string | Uint8Array }[] = [];
    for (const ground of (['light', 'dark'] as const).filter((g) => grounds.has(g))) {
      for (const { name, svg } of await backend.exportSvgs(icons, ground)) {
        if (format === 'svg') {
          out.push({ name: fileName(name, ground), data: svg });
          continue;
        }
        const box = viewBoxOf(svg);
        if (!box) throw new Error(`${name}'s SVG has no viewBox`);
        for (const px of PNG_SIZES.filter((s) => sizes.has(s))) {
          const { width, height } = pngSize(box, px);
          out.push({ name: fileName(name, ground, px), data: await rasterize(svg, width, height) });
        }
      }
    }
    return out;
  }

  async function run() {
    busy = true;
    done = '';
    failed = '';
    try {
      const made = await make();
      let where: (name: string) => string = (name) => name;
      let said = made.length === 1 ? made[0].name : `${made.length} files`;
      if (inShell()) {
        if (made.length === 1) {
          const ext = made[0].name.split('.').pop() ?? '';
          const kind = { svg: 'SVG', png: 'PNG', json: 'JSON', zip: 'Zip' }[ext] ?? ext;
          const path = await studio.backend.pickSave(`Export ${made[0].name}`, made[0].name, [ext], kind);
          if (!path) return;
          where = () => path;
          said = path;
        } else {
          const folder = await studio.backend.pickFolder(`Export ${made.length} files into a folder`);
          if (!folder) return;
          where = (name) => inFolder(folder, name);
          said = `${made.length} files in ${folder}`;
        }
      }
      for (const f of made) await studio.backend.writeExport(where(f.name), f.data);
      done = `Exported ${said}`;
    } catch (e) {
      failed = String(e instanceof Error ? e.message : e).replace(/^studio core: /, '');
    } finally {
      busy = false;
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
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="export-title" tabindex="-1" bind:this={box}>
    <h2 id="export-title">Export</h2>

    <label class="row">
      <span>As</span>
      <select bind:value={format}>
        <option value="svg">SVG</option>
        <option value="png">PNG</option>
        <option value="ir">IR (JSON)</option>
        <option value="zip">The library, as a zip</option>
      </select>
    </label>

    {#if format === 'zip'}
      <p class="note">Every file of the library as it is open, saved or not. <em>New library…</em> can start from it.</p>
    {:else}
      <div class="row" role="radiogroup" aria-label="Icons to export">
        <span>Icons</span>
        <span class="choices">
          <label><input type="radio" bind:group={scope} value="open" disabled={!open} /> {open ? `The open icon, ${open}` : 'The open icon'}</label>
          <label><input type="radio" bind:group={scope} value="chosen" /> Chosen icons</label>
          <label><input type="radio" bind:group={scope} value="all" /> Every icon ({all.length})</label>
        </span>
      </div>
      {#if scope === 'chosen'}
        <fieldset class="list">
          <legend>Chosen icons ({chosen.size} of {all.length})</legend>
          <span class="quick">
            <button onclick={() => (chosen = new Set(all))}>All</button>
            <button onclick={() => (chosen = new Set())}>None</button>
          </span>
          <ul aria-label="Icons">
            {#each all as name (name)}
              <li><label><input type="checkbox" checked={chosen.has(name)} onchange={() => (chosen = toggle(chosen, name))} /> {name}</label></li>
            {/each}
          </ul>
        </fieldset>
      {/if}
    {/if}

    {#if drawn}
      <div class="row" role="group" aria-label="Grounds">
        <span>Ground</span>
        <span class="inline">
          {#each [['light', 'Light'], ['dark', 'Dark']] as const as [g, label] (g)}
            <label><input type="checkbox" checked={grounds.has(g)} onchange={() => (grounds = toggle(grounds, g))} /> {label}</label>
          {/each}
        </span>
      </div>
    {/if}
    {#if format === 'png'}
      <div class="row" role="group" aria-label="Sizes">
        <span>Sizes</span>
        <span class="inline">
          {#each PNG_SIZES as px (px)}
            <label><input type="checkbox" checked={sizes.has(px)} onchange={() => (sizes = toggle(sizes, px))} /> {px}</label>
          {/each}
        </span>
      </div>
      <p class="note">Each PNG's longer side is the size, as Preview shows it.</p>
    {/if}

    <p class="note">
      {#if files === 0}
        Nothing to export: choose {count === 0 ? 'an icon' : drawn && grounds.size === 0 ? 'a ground' : 'a size'}.
      {:else if inShell()}
        {files === 1 ? 'One file, saved where you choose.' : `${files} files, into a folder you choose.`}
      {:else}
        {files === 1 ? 'One file, downloaded.' : `${files} files, downloaded.`}
      {/if}
    </p>
    {#if done}<p class="note done" role="status">{done}</p>{/if}
    {#if failed}<p class="failed" role="alert">{failed}</p>{/if}
    <div class="actions">
      <button onclick={onclose}>Close</button>
      <button class="primary" onclick={run} disabled={!ready}>{busy ? 'Exporting…' : 'Export'}</button>
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
    max-height: 90vh;
    overflow: auto;
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
  .choices {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--fg);
  }
  .inline {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    color: var(--fg);
  }
  .list {
    margin: 0;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: 8px;
    font-size: 12px;
  }
  .list legend {
    color: var(--muted);
    padding: 0 4px;
  }
  .quick {
    display: flex;
    gap: 6px;
    margin-bottom: 6px;
  }
  .list ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 180px;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 2px 10px;
  }
  select {
    font-size: 12px;
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
  .done {
    color: var(--fg);
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
