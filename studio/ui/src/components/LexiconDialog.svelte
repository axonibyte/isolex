<!--
  Lexicon Properties (#72; SPEC §2 and §9): the library's `lexicon` and
  `palette` blocks, in tabs as Inkscape's Document Properties has them —
  the tokens icons paint with, their swatches (each token's color on a
  light ground and a dark one), the stroke widths, the reserved tokens,
  and the limits. Each change edits the node it is about, where it is
  written, as one undo step; a block not yet written is added.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import type { Ask, LexiconRows, Setting } from '../lib/backend';
  import { DEFAULT_PALETTE } from '../lib/theme';
  import { kdlString } from '../lib/font';

  interface Props {
    studio: Studio;
    onclose: () => void;
  }

  let { studio, onclose }: Props = $props();

  const TABS = ['Tokens', 'Swatches', 'Stroke widths', 'Reserved', 'Limits'] as const;
  let tab = $state<(typeof TABS)[number]>('Tokens');

  let rows = $state<LexiconRows | null>(null);
  $effect(() => {
    void studio.view.library?.revision;
    void studio.backend.lexicon().then((r) => (rows = r));
  });

  const setting = (name: string): Setting | null => rows?.lexicon?.rows.find((r) => r.name === name) ?? null;
  const tokens = $derived(setting('tokens')?.args ?? []);
  const widths = $derived(setting('stroke-widths')?.args ?? []);
  const reserved = $derived(setting('reserved')?.args ?? []);
  const swatches = $derived(rows?.palette?.rows ?? []);
  /** The tokens to color: those declared, else those the palette colors. */
  const colored = $derived(tokens.length ? tokens : swatches.map((s) => s.name));

  let message = $state('');

  async function ask(a: Ask, label: string) {
    const answer = await studio.ask(a, label);
    message = answer.kind === 'refused' ? answer.reason : '';
    return answer.kind === 'applied';
  }

  /** A setting of the lexicon block written as `line`, or taken off for
      none: its node rewritten where it is, added to the block, or the
      block added at the top of its file. */
  async function setSetting(name: string, line: string | null, label: string) {
    const r = rows;
    if (!r) return;
    const at = setting(name);
    if (at) {
      if (line === null) await ask({ kind: 'delete', node: at.node }, label);
      else await ask({ kind: 'type', file: at.node.file, at: at.node.start, len: at.node.len, text: line }, label);
    } else if (line !== null) {
      if (r.lexicon) await ask({ kind: 'insert', parent: r.lexicon.node, text: line, before: null }, label);
      else await ask({ kind: 'type', file: r.file, at: 0, len: 0, text: `lexicon {\n  ${line}\n}\n\n` }, label);
    }
  }

  const quoted = (list: string[]) => list.map(kdlString).join(' ');

  // --- tokens --------------------------------------------------------------

  let newToken = $state('');
  async function addToken() {
    const t = newToken.trim();
    if (!t || tokens.includes(t)) return;
    await setSetting('tokens', `tokens ${quoted([...tokens, t])}`, `Add token ${t}`);
    newToken = '';
  }
  function removeToken(t: string) {
    const rest = tokens.filter((x) => x !== t);
    void setSetting('tokens', rest.length ? `tokens ${quoted(rest)}` : null, `Remove token ${t}`);
  }

  // --- swatches -------------------------------------------------------------

  /** A token's color on a ground, set where its swatch is written, or a
      swatch added (with the other color the default one), or the palette
      block added after the lexicon block. */
  async function setColor(token: string, ground: 'light' | 'dark', color: string) {
    const r = rows;
    if (!r || !/^#[0-9a-fA-F]{6}$/.test(color)) return;
    const label = `Color ${token} on ${ground}`;
    const swatch = swatches.find((s) => s.name === token);
    if (swatch) {
      await ask({ kind: 'set-prop', node: swatch.node, key: ground, value: color }, label);
      return;
    }
    const fallback = DEFAULT_PALETTE[token] ?? { light: '#808080', dark: '#808080' };
    const colors = { ...fallback, [ground]: color };
    const line = `${token} light=${kdlString(colors.light)} dark=${kdlString(colors.dark)}`;
    if (r.palette) {
      await ask({ kind: 'insert', parent: r.palette.node, text: line, before: null }, label);
    } else if (r.lexicon) {
      // after the lexicon block, in its file
      const end = r.lexicon.node.start + r.lexicon.node.len;
      await ask({ kind: 'type', file: r.lexicon.node.file, at: end, len: 0, text: `\n\npalette {\n  ${line}\n}` }, label);
    } else {
      await ask({ kind: 'type', file: r.file, at: 0, len: 0, text: `palette {\n  ${line}\n}\n\n` }, label);
    }
  }
  function removeSwatch(s: Setting) {
    void ask({ kind: 'delete', node: s.node }, `Remove the swatch of ${s.name}`);
  }

  // --- stroke widths, reserved ------------------------------------------------

  let newWidth = $state('');
  function addWidth() {
    const w = Number(newWidth);
    if (!(w > 0) || widths.map(Number).includes(w)) return;
    const all = [...widths.map(Number), w].sort((a, b) => a - b);
    void setSetting('stroke-widths', `stroke-widths ${all.join(' ')}`, `Add stroke width ${w}`);
    newWidth = '';
  }
  function removeWidth(w: string) {
    const rest = widths.filter((x) => x !== w);
    void setSetting('stroke-widths', rest.length ? `stroke-widths ${rest.join(' ')}` : null, `Remove stroke width ${w}`);
  }
  function toggleReserved(t: string) {
    const next = reserved.includes(t) ? reserved.filter((x) => x !== t) : [...reserved, t];
    void setSetting('reserved', next.length ? `reserved ${quoted(next)}` : null, `${reserved.includes(t) ? 'Free' : 'Reserve'} ${t}`);
  }

  // --- limits -------------------------------------------------------------------

  const LIMITS: [string, number, string][] = [
    ['shape-limit', 100000, 'shapes and polygon points one icon emits'],
    ['repeat-limit', 1000000, 'repeat iterations in one icon'],
    ['insert-depth', 32, 'how deeply fragment insertions nest'],
    ['icon-limit', 100000, 'icons the lexicon emits, after axis products'],
    ['expression-depth', 256, 'how deeply one expression nests'],
    ['transform-chain', 64, 'transform steps one shape is drawn through'],
    ['text-length', 1000, 'characters in one text'],
    ['text-limit', 10000, 'characters of text in one icon'],
    ['glyph-limit', 65536, 'glyphs one font draws in its styles'],
  ];
  function setLimit(name: string, typed: string, none: boolean) {
    const n = Math.floor(Number(typed));
    if (none) void setSetting(name, `${name} #null`, `Switch off ${name}`);
    else if (!typed.trim()) void setSetting(name, null, `Default ${name}`);
    else if (n > 0) void setSetting(name, `${name} ${n}`, `Set ${name} to ${n}`);
  }

  /** The dialog, focused when it opens. */
  let box: HTMLDivElement | undefined = $state();
  $effect(() => {
    box?.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="scrim" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="lexicon-title" tabindex="-1" bind:this={box}>
    <div class="head">
      <h2 id="lexicon-title">Lexicon Properties</h2>
      <span class="where">{rows?.lexicon ? `in ${rows.lexicon.node.file}` : `a new block goes in ${rows?.file ?? '…'}`}</span>
      <button class="close" onclick={onclose} aria-label="Close">×</button>
    </div>
    <div class="tabs" role="tablist">
      {#each TABS as t (t)}
        <button role="tab" id="lex-tab-{t}" aria-controls="lex-pane" aria-selected={tab === t} onclick={() => (tab = t)}>{t}</button>
      {/each}
    </div>
    <div class="pane" id="lex-pane" role="tabpanel" aria-labelledby="lex-tab-{tab}">
      {#if tab === 'Tokens'}
        <p class="note">What icons may paint with. With none declared, any token is let through.</p>
        <ul class="chips" aria-label="Tokens">
          {#each tokens as t (t)}
            <li><code>{t}</code><button aria-label="Remove token {t}" onclick={() => removeToken(t)}>×</button></li>
          {/each}
        </ul>
        <div class="add">
          <input type="text" bind:value={newToken} placeholder="a token, as body-top" aria-label="New token" onkeydown={(e) => e.key === 'Enter' && addToken()} />
          <button onclick={addToken} disabled={!newToken.trim()}>Add token</button>
        </div>
      {:else if tab === 'Swatches'}
        <p class="note">Each token’s color on a light canvas and on a dark one: the library’s own palette.</p>
        <table>
          <thead><tr><th scope="col">Token</th><th scope="col">Light</th><th scope="col">Dark</th><th scope="col"><span class="sr-only">Remove</span></th></tr></thead>
          <tbody>
            {#each colored as t (t)}
              {@const s = swatches.find((x) => x.name === t)}
              <tr>
                <th scope="row"><code>{t}</code></th>
                {#each ['light', 'dark'] as const as ground (ground)}
                  <td>
                    <input type="color" aria-label="{t} on {ground}" value={s?.props[ground] ?? DEFAULT_PALETTE[t]?.[ground] ?? '#808080'} onchange={(e) => setColor(t, ground, e.currentTarget.value)} />
                    <code class="hex">{s?.props[ground] ?? '—'}</code>
                  </td>
                {/each}
                <td>{#if s}<button aria-label="Remove the swatch of {t}" onclick={() => removeSwatch(s)}>×</button>{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else if tab === 'Stroke widths'}
        <p class="note">The widths strokes may be drawn at, chosen in Design, never typed there.</p>
        <ul class="chips" aria-label="Stroke widths">
          {#each widths as w (w)}
            <li><code>{w}</code><button aria-label="Remove stroke width {w}" onclick={() => removeWidth(w)}>×</button></li>
          {/each}
        </ul>
        <div class="add">
          <input type="number" min="0.1" step="0.1" bind:value={newWidth} placeholder="1.4" aria-label="New stroke width" />
          <button onclick={addWidth} disabled={!(Number(newWidth) > 0)}>Add width</button>
        </div>
      {:else if tab === 'Reserved'}
        <p class="note">Tokens an icon paints with only under a <code>grant</code>, such as a state’s alarm color.</p>
        <div class="checks" role="group" aria-label="Reserved tokens">
          {#each tokens as t (t)}
            <label><input type="checkbox" checked={reserved.includes(t)} onchange={() => toggleReserved(t)} /> <code>{t}</code></label>
          {:else}
            <p class="note">Declare tokens first.</p>
          {/each}
        </div>
      {:else}
        <p class="note">Each keeps a mistake from exhausting the machine. Empty is the default; a definition can set its own.</p>
        <table>
          <tbody>
            {#each LIMITS as [name, fallback, what] (name)}
              {@const s = setting(name)}
              {@const none = s?.args[0] === '#null'}
              <tr>
                <th scope="row"><code>{name}</code><small>{what}</small></th>
                <td><input type="number" min="1" step="1" aria-label={name} placeholder={String(fallback)} value={none ? '' : (s?.args[0] ?? '')} disabled={none} onchange={(e) => setLimit(name, e.currentTarget.value, false)} /></td>
                <td><label class="off"><input type="checkbox" checked={none} onchange={(e) => (e.currentTarget.checked ? setLimit(name, '', true) : setLimit(name, '', false))} /> no limit</label></td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
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
    width: min(620px, 94vw);
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
  .where {
    flex: 1;
    color: var(--muted);
    font-size: 12px;
  }
  .close {
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 18px;
    cursor: pointer;
  }
  .tabs {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    padding: 6px 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .tabs button[aria-selected='true'] {
    color: var(--fg);
    border-bottom-color: var(--accent);
  }
  .pane {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 200px;
  }
  .note {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .chips li {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px 2px 8px;
    border-radius: 999px;
    background: var(--raised);
    border: 1px solid var(--line);
  }
  .chips button,
  td button {
    border: 0;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .add {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 6px;
  }
  .add button {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--fg);
    cursor: pointer;
  }
  .add button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  input {
    min-width: 0;
    box-sizing: border-box;
  }
  table {
    border-collapse: collapse;
    font-size: 12px;
  }
  th,
  td {
    padding: 4px 6px;
    text-align: left;
    border-bottom: 1px solid var(--line);
  }
  th small {
    display: block;
    color: var(--muted);
    font-weight: 400;
  }
  input[type='color'] {
    width: 32px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--line);
    background: transparent;
    vertical-align: middle;
  }
  .hex {
    margin-left: 4px;
    color: var(--muted);
  }
  .checks {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    font-size: 12px;
  }
  .off {
    white-space: nowrap;
    color: var(--muted);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .message {
    margin: 0;
    min-height: 1.3em;
    color: var(--muted);
    font-size: 12px;
  }
</style>
