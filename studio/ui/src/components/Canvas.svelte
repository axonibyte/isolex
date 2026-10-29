<!--
  The canvas (docs/studio-interface.md, The canvas): the open icon as
  isolex draws it, over its grid and frame, zoomed and panned as the spec
  says. The tools (lib/tools.svelte.ts) take presses on it: a click
  selects (#68); the tools drag, resize, shape and draw (#69, #83), with
  handles, the shape being drawn, the snap indicator and the size beside
  the pointer drawn over the icon, and the popover for an edit the core
  needs a choice for.
-->
<script lang="ts">
  import type { IrIcon } from '../lib/isolex/ir';
  import { formatNumber as n } from '../lib/isolex/ir';
  import IsoShapes from './IsoShapes.svelte';
  import { fit, onFloor, project, toCanvas, wheel, zoomAt, type Camera } from '../lib/view';
  import type { Press, Tools } from '../lib/tools.svelte';
  import type { Point } from '../lib/geometry';

  interface Props {
    icon: IrIcon | null;
    /** The tools; without them, a click selects and nothing more. */
    tools?: Tools;
    /** The shapes, by index, marked as selected, and those left out. */
    marked?: Set<number>;
    hidden?: Set<number>;
    /** Where the pointer is: tile x and y on the floor, or flat x and y. */
    onpointer?: (at: string) => void;
    /** A click on the shape at this index, or on none. */
    onpick?: (shape: number | null) => void;
    /** The shape under the pointer, or none. */
    onhover?: (shape: number | null) => void;
  }

  let { icon, tools, marked, hidden, onpointer, onpick, onhover }: Props = $props();

  let host: HTMLDivElement;
  // unmeasured until the window lays it out
  let size = $state({ w: 0, h: 0 });
  let camera = $state<Camera>({ k: 1, x: 400, y: 300 });
  let fitted: string | null = null;
  let pan = $state<{ x: number; y: number; cx: number; cy: number } | null>(null);
  let space = $state(false);
  /** A press the tools have, and the shape it was on. */
  let pressed: { shape: number | null } | null = null;
  let hovered: number | null = null;

  const tool = $derived(tools?.tool ?? 'select');
  const drawing = $derived(!['select', 'node', 'hand'].includes(tool));

  /** The index of the shape an event is on, and the handle. */
  function shapeOf(e: Event): number | null {
    const i = (e.target as Element | null)?.closest?.('[data-shape]')?.getAttribute('data-shape');
    return i == null ? null : Number(i);
  }
  const handleOf = (e: Event): string | null => (e.target as Element | null)?.closest?.('[data-handle]')?.getAttribute('data-handle') ?? null;

  const flat = $derived(!!icon?.canvas);

  /** The icon fitted to the window, when it first shows. */
  $effect(() => {
    if (icon && icon.name !== fitted && size.w > 0 && size.h > 0) {
      fitted = icon.name;
      camera = fit(icon.viewbox, size.w, size.h);
    }
  });

  export function fitIcon() {
    if (icon) camera = fit(icon.viewbox, size.w, size.h);
  }

  function local(e: { clientX: number; clientY: number }): [number, number] {
    const r = host.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  }

  /** On screen, in the canvas: a point of the viewBox. */
  const screen = (p: Point): Point => [camera.x + p[0] * camera.k, camera.y + p[1] * camera.k];

  function press(e: PointerEvent, shape: number | null, handle: string | null): Press {
    return { at: toCanvas(camera, local(e)), shape, handle, alt: e.altKey, shift: e.shiftKey, scale: camera.k };
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    const a = wheel(e);
    camera = a.kind === 'zoom' ? zoomAt(camera, a.factor, local(e)) : { ...camera, x: camera.x + a.dx, y: camera.y + a.dy };
  }

  function onpointerdown(e: PointerEvent) {
    if (e.button === 1 || (e.button === 0 && (space || tool === 'hand'))) {
      e.preventDefault();
      (e.target as Element).setPointerCapture?.(e.pointerId);
      pan = { x: e.clientX, y: e.clientY, cx: camera.x, cy: camera.y };
      return;
    }
    if (e.button !== 0 || !icon) return;
    const shape = shapeOf(e);
    pressed = { shape };
    host.setPointerCapture?.(e.pointerId);
    tools?.down(press(e, shape, handleOf(e)));
  }

  function onpointerup(e: PointerEvent) {
    pan = null;
    const p = pressed;
    pressed = null;
    if (!p) return;
    const click = tools ? tools.up(press(e, p.shape, null)) : true;
    if (click && tool === 'select') onpick?.(p.shape);
  }

  function onpointermove(e: PointerEvent) {
    if (pan) {
      camera = { ...camera, x: pan.cx + e.clientX - pan.x, y: pan.cy + e.clientY - pan.y };
      return;
    }
    const at = toCanvas(camera, local(e));
    tools?.move(press(e, pressed?.shape ?? shapeOf(e), null), !!pressed);
    if (!pressed) {
      const over = shapeOf(e);
      if (over !== hovered) {
        hovered = over;
        onhover?.(over);
      }
    }
    if (flat) onpointer?.(`x ${n(at[0])} · y ${n(at[1])}`);
    else {
      const [x, y] = onFloor(at);
      onpointer?.(`x ${n(x)} · y ${n(y)} · z 0.00`);
    }
  }

  function ondblclick(e: MouseEvent) {
    if (!icon || !tools) return;
    tools.double({ at: toCanvas(camera, local(e)), shape: shapeOf(e), handle: handleOf(e), alt: e.altKey, shift: e.shiftKey, scale: camera.k });
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === ' ' && !(e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement)) {
      space = true;
    }
  }
  function onkeyup(e: KeyboardEvent) {
    if (e.key === ' ') space = false;
  }

  /** The grid: the floor's at whole and quarter tiles, or a flat icon's at
      every unit, every fourth stronger. */
  const grid = $derived.by(() => {
    const lines: { a: [number, number]; b: [number, number]; strong: boolean }[] = [];
    if (!icon) return lines;
    if (flat) {
      const [, , w, h] = icon.viewbox;
      for (let x = 0; x <= w; x++) lines.push({ a: [x, 0], b: [x, h], strong: x % 4 === 0 });
      for (let y = 0; y <= h; y++) lines.push({ a: [0, y], b: [w, y], strong: y % 4 === 0 });
      return lines;
    }
    for (let i = -8; i <= 12; i += 0.25) {
      const strong = Number.isInteger(i);
      lines.push({ a: project(i, -8, 0), b: project(i, 12, 0), strong });
      lines.push({ a: project(-8, i, 0), b: project(12, i, 0), strong });
    }
    return lines;
  });

  const handles = $derived(tools?.handles ?? []);
  const line = (ps: Point[]) => ps.map((p) => p.join(',')).join(' ');
</script>

<svelte:window {onkeydown} {onkeyup} />

<div
  class="canvas"
  class:panning={space || pan || tool === 'hand'}
  class:drawing
  bind:this={host}
  bind:clientWidth={size.w}
  bind:clientHeight={size.h}
  role="application"
  aria-label={icon ? `Canvas: ${icon.name}` : 'Canvas'}
  {onwheel}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  {ondblclick}
  onpointercancel={() => {
    pan = null;
    pressed = null;
    tools?.cancel();
  }}
  onpointerleave={() => {
    if (hovered !== null) onhover?.((hovered = null));
  }}
>
  {#if icon}
    <svg width="100%" height="100%" aria-hidden="true">
      <g transform="translate({camera.x} {camera.y}) scale({camera.k})">
        <g class="grid">
          {#each grid as l, i (i)}
            <line x1={l.a[0]} y1={l.a[1]} x2={l.b[0]} y2={l.b[1]} class:strong={l.strong} vector-effect="non-scaling-stroke" />
          {/each}
        </g>
        <rect
          class="frame"
          x={icon.viewbox[0]}
          y={icon.viewbox[1]}
          width={icon.viewbox[2]}
          height={icon.viewbox[3]}
          vector-effect="non-scaling-stroke"
        />
        <IsoShapes shapes={icon.shapes} tagged {marked} {hidden} />
        {#if tools}
          {#if tools.highlight}
            <polygon class="face" points={line(tools.highlight)} vector-effect="non-scaling-stroke" />
          {/if}
          {#each tools.preview as ps, i (i)}
            <polyline class="preview" points={line(ps)} vector-effect="non-scaling-stroke" />
          {/each}
          {#each tools.dots as d, i (i)}
            <circle class="dot" class:first={d.first} cx={d.at[0]} cy={d.at[1]} r={3.5 / camera.k} vector-effect="non-scaling-stroke" />
          {/each}
          {#each handles as h (h.id)}
            {#if h.from}
              <line class="arm" x1={h.from[0]} y1={h.from[1]} x2={h.at[0]} y2={h.at[1]} vector-effect="non-scaling-stroke" />
            {/if}
          {/each}
          {#if tools.indicator}
            <circle class="snapped" cx={tools.indicator.at[0]} cy={tools.indicator.at[1]} r={5 / camera.k} vector-effect="non-scaling-stroke" />
          {/if}
          {#each handles as h (h.id)}
            {@const r = 4.5 / camera.k}
            {#if h.kind === 'height'}
              <g class="handle height" data-handle={h.id}>
                <line x1={h.at[0]} y1={h.at[1]} x2={h.at[0]} y2={h.at[1] - 18 / camera.k} vector-effect="non-scaling-stroke" />
                <polygon
                  points={line([
                    [h.at[0] - r, h.at[1] - 14 / camera.k],
                    [h.at[0] + r, h.at[1] - 14 / camera.k],
                    [h.at[0], h.at[1] - 22 / camera.k],
                  ])}
                  vector-effect="non-scaling-stroke"
                />
              </g>
            {:else if h.kind === 'radius' || h.kind === 'control'}
              <circle class="handle" class:control={h.kind === 'control'} data-handle={h.id} cx={h.at[0]} cy={h.at[1]} r={h.kind === 'control' ? r * 0.8 : r} vector-effect="non-scaling-stroke"><title>{h.label}</title></circle>
            {:else if h.kind === 'point'}
              <rect class="handle point" class:first={h.first} class:touched={tools.touched === Number(h.id.slice(3))} data-handle={h.id} x={h.at[0] - r} y={h.at[1] - r} width={2 * r} height={2 * r} vector-effect="non-scaling-stroke"><title>{h.label}</title></rect>
            {:else}
              <rect class="handle" data-handle={h.id} x={h.at[0] - r} y={h.at[1] - r} width={2 * r} height={2 * r} vector-effect="non-scaling-stroke" />
            {/if}
          {/each}
        {/if}
      </g>
    </svg>
    {#if tools?.indicator}
      {@const at = screen(tools.indicator.at)}
      <span class="tag snap" style="left: {at[0] + 10}px; top: {at[1] - 24}px">{tools.indicator.what}</span>
    {/if}
    {#if tools?.feedback}
      {@const at = screen(tools.feedback.at)}
      <span class="tag" role="status" style="left: {at[0] + 14}px; top: {at[1] + 12}px">{tools.feedback.text}</span>
    {/if}
    {#if tools?.choice}
      {@const at = screen(tools.choice.at)}
      <div class="choice" role="dialog" tabindex="-1" aria-label="This edit needs a choice" style="left: {at[0] + 12}px; top: {at[1] + 12}px" onpointerdown={(e) => e.stopPropagation()}>
        {#each tools.choice.options as o, k (k)}
          <button onclick={() => tools.choose(k)}><span>{o.label}</span><small>{o.detail}</small></button>
        {/each}
        <button class="cancel" onclick={() => tools.cancel()}>Cancel</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .canvas {
    position: absolute;
    inset: 0;
    background: var(--canvas);
    overflow: hidden;
    touch-action: none;
  }
  .canvas.panning {
    cursor: grab;
  }
  .canvas.drawing {
    cursor: crosshair;
  }
  .canvas :global(.marked) {
    filter: drop-shadow(0 0 1px var(--accent)) drop-shadow(0 0 1px var(--accent));
  }
  svg {
    display: block;
  }
  .grid line {
    stroke: var(--grid);
    stroke-width: 1;
  }
  .grid line.strong {
    stroke: var(--grid-strong);
  }
  .frame {
    fill: none;
    stroke: var(--frame);
    stroke-dasharray: 3 3;
  }
  .preview {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.5;
    stroke-dasharray: 4 3;
  }
  .face {
    fill: var(--accent);
    fill-opacity: 0.18;
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .dot {
    fill: var(--canvas);
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .dot.first,
  .handle.point.first {
    fill: var(--accent);
  }
  .handle.point.touched {
    stroke-width: 2.5;
  }
  .arm {
    stroke: var(--accent);
    stroke-width: 1;
  }
  .handle.control {
    fill: var(--accent);
  }
  .snapped {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
  }
  .handle,
  .handle polygon {
    fill: var(--canvas);
    stroke: var(--accent);
    stroke-width: 1.5;
    cursor: pointer;
  }
  .handle.height line {
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .handle.height polygon {
    fill: var(--accent);
    cursor: ns-resize;
  }
  .tag {
    position: absolute;
    pointer-events: none;
    padding: 2px 7px;
    border-radius: 6px;
    background: var(--panel);
    color: var(--fg);
    border: 1px solid var(--line);
    font: 11px var(--font-mono);
    white-space: nowrap;
  }
  .tag.snap {
    color: var(--accent);
  }
  .choice {
    position: absolute;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 220px;
    padding: 6px;
    border-radius: 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  }
  .choice button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 6px 9px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .choice button:hover,
  .choice button:focus-visible {
    background: var(--raised);
  }
  .choice small {
    color: var(--muted);
  }
  .choice .cancel {
    color: var(--muted);
  }
</style>
