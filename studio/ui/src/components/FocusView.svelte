<!--
  The Tree's focus view (#113, docs/studio-interface.md, Tree), as
  Unreal's Reference Viewer: the definition being edited in the middle,
  what it depends on to its left, what depends on it to its right, a
  column a step, to a depth. Live as the library view is: thumbnails
  redrawn, a dot for what an edit redrew, a mark for what it broke. A
  click opens a definition as the Library dialog does; hovering one lights
  its lines and says how it relates to the middle.
-->
<script lang="ts">
  import type { Studio } from '../lib/state.svelte';
  import { focus, key, said, verb, type FocusNode } from '../lib/focus';
  import { thumbHeight } from './TreeTab.svelte';
  import IsoIcon from './IsoIcon.svelte';

  interface Props {
    studio: Studio;
    depth: number;
    /** Says how a hovered node relates to the middle; empty when none is. */
    onhint: (said: string) => void;
  }

  let { studio, depth, onhint }: Props = $props();

  const W = 156;
  const H = 46;
  const GAP_X = 44;
  const GAP_Y = 12;
  const PAD = 12;

  const view = $derived(studio.view);
  const center = $derived(studio.editing);
  const layout = $derived(view.graph && center ? focus(view.graph, center, depth) : null);
  const defs = $derived(new Map((view.graph?.definitions ?? []).map((d) => [d.name, d])));

  /** The columns, left to right, each its nodes in order. */
  const columns = $derived.by(() => {
    const out: { column: number; nodes: FocusNode[] }[] = [];
    for (let column = -depth; column <= depth; column++) {
      const nodes = (layout?.nodes ?? []).filter((n) => n.column === column).sort((a, b) => a.row - b.row);
      if (nodes.length) out.push({ column, nodes });
    }
    return out;
  });
  const tallest = $derived(Math.max(1, ...columns.map((c) => c.nodes.length)));
  const first = $derived(columns[0]?.column ?? 0);
  const last = $derived(columns.at(-1)?.column ?? 0);
  const width = $derived(PAD * 2 + (last - first + 1) * W + (last - first) * GAP_X);
  const height = $derived(PAD * 2 + tallest * H + (tallest - 1) * GAP_Y);

  /** Where each node's card is: its column's nodes centered on the middle. */
  const places = $derived.by(() => {
    const out = new Map<string, { x: number; y: number }>();
    for (const c of columns) {
      const top = PAD + ((tallest - c.nodes.length) * (H + GAP_Y)) / 2;
      for (const n of c.nodes) out.set(key(n), { x: PAD + (c.column - first) * (W + GAP_X), y: top + n.row * (H + GAP_Y) });
    }
    return out;
  });

  const lines = $derived(
    (layout?.links ?? []).flatMap((l) => {
      const a = places.get(l.left);
      const b = places.get(l.right);
      if (!a || !b) return [];
      const [x1, y1, x2, y2] = [a.x + W, a.y + H / 2, b.x, b.y + H / 2];
      const bend = GAP_X / 2;
      return [{ ...l, d: `M${x1},${y1} C${x1 + bend},${y1} ${x2 - bend},${y2} ${x2},${y2}` }];
    }),
  );

  let hovered = $state<string | null>(null);

  /** The icon a node's card draws: the one open, for the middle, or the
      first its definition emits. */
  function drawn(n: FocusNode): string | null {
    const emits = defs.get(n.name)?.emits ?? [];
    return n.column === 0 && studio.icon && emits.includes(studio.icon) ? studio.icon : (emits[0] ?? null);
  }

  function open(n: FocusNode) {
    const emits = defs.get(n.name)?.emits ?? [];
    if (emits.length && !(studio.icon && emits.includes(studio.icon))) studio.openIcon(emits[0]!);
    else studio.edit(n.name);
  }

  function hover(n: FocusNode | null) {
    hovered = n ? key(n) : null;
    onhint(n ? `${n.name}: ${said(n)}` : '');
  }

  const kindOf = (n: FocusNode) => (n.kind ? n.kind.toLowerCase() : 'not defined');
  const heading = (column: number) => (column === 0 ? 'Being edited' : column < 0 ? `Depends on, step ${-column}` : `Depended on by, step ${column}`);

  // the middle kept in view as it changes
  let scroller: HTMLDivElement | undefined = $state();
  $effect(() => {
    void center;
    void depth;
    const middle = scroller?.querySelector<HTMLElement>('[aria-current="true"]');
    middle?.scrollIntoView?.({ block: 'nearest', inline: 'center' });
  });
</script>

{#if !center}
  <p class="none">Open an icon to see what it depends on, and what depends on it.</p>
{:else if layout}
  <div class="scroller" bind:this={scroller}>
    <div class="board" style="width: {width}px; height: {height}px">
      <svg class="lines" {width} {height} aria-hidden="true">
        {#each lines as l (`${l.left} ${l.right} ${l.kind} ${l.variant ?? ''}`)}
          <path class="line {l.kind}" class:lit={hovered === l.left || hovered === l.right} d={l.d}><title>{verb(l.kind)}{l.variant ? `, in variant ${l.variant}` : ''}</title></path>
        {/each}
      </svg>
      {#each columns as c (c.column)}
        <ul aria-label={heading(c.column)}>
          {#each c.nodes as n (key(n))}
            {@const at = places.get(key(n))!}
            {@const icon = drawn(n)}
            {@const broken = icon ? view.broken[icon] : undefined}
            {@const times = icon ? (view.redrawn[icon] ?? 0) : 0}
            <li style="left: {at.x}px; top: {at.y}px; width: {W}px; height: {H}px">
              <button
                class="card"
                class:missing={!n.kind}
                class:lit={hovered === key(n)}
                data-key={key(n)}
                aria-current={n.column === 0 ? 'true' : undefined}
                disabled={!n.kind}
                title={broken ? broken.map((d) => d.message).join('\n') : n.name}
                onclick={() => open(n)}
                draggable={n.kind === 'Fragment' || n.kind === 'Icon'}
                ondragstart={(e) => e.dataTransfer?.setData('application/x-isolex-place', JSON.stringify({ kind: n.kind === 'Fragment' ? 'insert' : 'badge', name: n.name }))}
                onpointerenter={() => hover(n)}
                onpointerleave={() => hover(null)}
                onfocus={() => hover(n)}
                onblur={() => hover(null)}
              >
                <span class="thumb">
                  {#if icon && view.icons[icon]}
                    <IsoIcon icon={view.icons[icon]!} height={thumbHeight(view.icons[icon]!)} />
                  {:else}
                    <span class="glyph" aria-hidden="true">{n.kind === 'Font' ? 'Aa' : n.kind === 'Base' ? '◇' : n.kind === 'Axis' ? '⇄' : n.kind ? '·' : '?'}</span>
                  {/if}
                </span>
                <span class="text">
                  <span class="name">{n.name}</span>
                  <span class="kind">{kindOf(n)}</span>
                </span>
                {#if broken}
                  <span class="broken" aria-hidden="true">▲</span>
                {:else if times > 0}
                  {#key times}<span class="redrawn" aria-hidden="true"></span>{/key}
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/each}
    </div>
  </div>
  <ul class="legend" aria-label="Lines">
    {#each ['extends', 'insert', 'badge', 'axis', 'font'] as const as k (k)}
      <li><svg width="22" height="8" aria-hidden="true"><path class="line {k}" d="M1,4 H21" /></svg>{verb(k)}</li>
    {/each}
  </ul>
{/if}

<style>
  .none {
    margin: 12px;
    color: var(--muted);
    font-size: 12px;
  }
  .scroller {
    flex: 1;
    overflow: auto;
  }
  .board {
    position: relative;
  }
  .lines {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    position: absolute;
  }
  .card {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 100%;
    padding: 4px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--panel);
    color: var(--fg);
    cursor: pointer;
  }
  .card:hover,
  .card:focus-visible,
  .card.lit {
    border-color: var(--accent);
  }
  .card[aria-current='true'] {
    background: color-mix(in srgb, var(--accent) 18%, var(--panel));
    border-color: var(--accent);
  }
  .card.missing {
    border-style: dashed;
    cursor: default;
  }
  .thumb {
    display: grid;
    place-items: center;
    width: 44px;
    height: 34px;
    flex: none;
    border-radius: 6px;
    background: var(--canvas);
  }
  .glyph {
    color: var(--muted);
    font-size: 13px;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    text-align: left;
  }
  .name {
    font: 11px var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    color: var(--muted);
    font-size: 10px;
    white-space: nowrap;
  }
  .broken {
    position: absolute;
    top: 3px;
    right: 6px;
    color: var(--warn);
  }
  .redrawn {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: fade 2.4s ease-out forwards;
  }
  @keyframes fade {
    0%,
    30% {
      opacity: 1;
      transform: scale(1.4);
    }
    100% {
      opacity: 0;
      transform: scale(1);
    }
  }
  .line {
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.5;
  }
  .line.insert {
    stroke-dasharray: 5 3;
  }
  .line.badge {
    stroke-dasharray: 1.5 3;
  }
  .line.axis {
    stroke-dasharray: 8 3 1.5 3;
  }
  .line.font {
    stroke-dasharray: 10 4;
  }
  .line.lit {
    stroke: var(--accent);
    stroke-width: 2;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    padding: 6px 12px;
    color: var(--muted);
    font-size: 11px;
  }
  .legend li {
    position: static;
    display: flex;
    align-items: center;
    gap: 4px;
  }
</style>
