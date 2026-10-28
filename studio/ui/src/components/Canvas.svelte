<!--
  The canvas (docs/studio-interface.md, The canvas): the open icon as
  isolex draws it, over its grid and frame, zoomed and panned as the spec
  says. A click selects the shape under the pointer (#68); drawing on it
  comes with the tools (#69, #83).
-->
<script lang="ts">
  import type { IrIcon } from '../lib/isolex/ir';
  import { formatNumber as n } from '../lib/isolex/ir';
  import IsoShapes from './IsoShapes.svelte';
  import { fit, onFloor, project, toCanvas, wheel, zoomAt, type Camera } from '../lib/view';

  interface Props {
    icon: IrIcon | null;
    /** The tool: with Pan, a drag pans; with Select, a click selects. */
    tool?: string;
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

  let { icon, tool = 'select', marked, hidden, onpointer, onpick, onhover }: Props = $props();

  let host: HTMLDivElement;
  // unmeasured until the window lays it out
  let size = $state({ w: 0, h: 0 });
  let camera = $state<Camera>({ k: 1, x: 400, y: 300 });
  let fitted: string | null = null;
  let pan = $state<{ x: number; y: number; cx: number; cy: number } | null>(null);
  let space = $state(false);
  /** Where a click began, to tell it from a drag. */
  let press: { x: number; y: number } | null = null;
  let hovered: number | null = null;

  /** The index of the shape an event is on. */
  function shapeOf(e: Event): number | null {
    const el = (e.target as Element | null)?.closest?.('[data-shape]');
    const i = el?.getAttribute('data-shape');
    return i == null ? null : Number(i);
  }

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
    } else if (e.button === 0 && tool === 'select') {
      press = { x: e.clientX, y: e.clientY };
    }
  }

  function onpointerup(e: PointerEvent) {
    pan = null;
    if (press && Math.hypot(e.clientX - press.x, e.clientY - press.y) < 4) onpick?.(shapeOf(e));
    press = null;
  }

  function onpointermove(e: PointerEvent) {
    if (pan) {
      camera = { ...camera, x: pan.cx + e.clientX - pan.x, y: pan.cy + e.clientY - pan.y };
      return;
    }
    const over = shapeOf(e);
    if (over !== hovered) {
      hovered = over;
      onhover?.(over);
    }
    const p = toCanvas(camera, local(e));
    if (flat) onpointer?.(`x ${n(p[0])} · y ${n(p[1])}`);
    else {
      const [x, y] = onFloor(p);
      onpointer?.(`x ${n(x)} · y ${n(y)} · z 0.00`);
    }
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
</script>

<svelte:window {onkeydown} {onkeyup} />

<div
  class="canvas"
  class:panning={space || pan || tool === 'hand'}
  bind:this={host}
  bind:clientWidth={size.w}
  bind:clientHeight={size.h}
  role="application"
  aria-label={icon ? `Canvas: ${icon.name}` : 'Canvas'}
  {onwheel}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={() => {
    pan = null;
    press = null;
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
      </g>
    </svg>
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
</style>
