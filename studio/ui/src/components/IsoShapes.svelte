<!-- Copied from bam (Cambridge-Hackspace/boring-asset-manager) at b2ca54b:
     web/src/components/IsoShapes.svelte. Changes there are made here too, and the other way round. -->
<svelte:options namespace="svg" />
<!-- Here, not in bam: the tagged, marked and hidden props, which the
     studio's canvas selects by; without them it draws as bam's does. -->

<!--
  An isolex icon's shapes (README §3.7), as isolex's reference renderer
  draws them: the same shapes, in the same order, every number by the same
  rule, tokens painted with the palette's CSS properties. Drawn into an
  SVG: IsoIcon's own, or a scene's. A group of IR version 2 is drawn
  through its matrix, cut to its clip, and with its strokes kept at their
  width when fixed, as isolex 0.4 draws it; its clip paths are named for
  this drawing, so that an icon drawn many times on one page cuts each
  copy to its own.
-->
<script lang="ts">
  import type { IrShape } from '../lib/isolex/ir';
  import { formatNumber as n } from '../lib/isolex/ir';
  import { tokenVar } from '../lib/isolex/palette';

  interface Props {
    shapes: IrShape[];
    /** Tag each shape with its index (`data-shape`), counted as isolex's
        provenance counts them: a group, then its own. */
    tagged?: boolean;
    /** The shapes, by index, to mark as selected. */
    marked?: Set<number>;
    /** The shapes, by index, to leave out. */
    hidden?: Set<number>;
  }

  let { shapes, tagged = false, marked, hidden }: Props = $props();

  const uid = $props.id();
  // each clipped group's clip path, numbered in the order isolex numbers
  // them: as the shapes are drawn
  const clips = $derived.by(() => {
    const ids = new Map<IrShape, string>();
    const walk = (ss: IrShape[]) => {
      for (const s of ss) {
        if (s.kind !== 'group') continue;
        if (s.clip) ids.set(s, `iso-clip-${uid}-${ids.size + 1}`);
        walk(s.shapes ?? []);
      }
    };
    walk(shapes);
    return ids;
  });
  // each shape's index, when they are tagged, marked or hidden
  const index = $derived.by(() => {
    const ids = new Map<IrShape, number>();
    if (!tagged && !marked && !hidden) return ids;
    const walk = (ss: IrShape[]) => {
      for (const s of ss) {
        ids.set(s, ids.size);
        if (s.kind === 'group') walk(s.shapes ?? []);
      }
    };
    walk(shapes);
    return ids;
  });
  const tag = (s: IrShape) => (tagged ? index.get(s) : undefined);
  const mark = (s: IrShape) => (marked?.has(index.get(s) ?? -1) ? 'marked' : undefined);
  const skip = (s: IrShape) => !!hidden?.has(index.get(s) ?? -1);
  const points = (ps: [number, number][]) => ps.map(([x, y]) => `${n(x)},${n(y)}`).join(' ');
  const matrix = (m: number[]) => `matrix(${m.map(n).join(' ')})`;

  const paint = (token: string | undefined) => (!token || token === 'none' ? 'none' : tokenVar(token));
  // What isolex leaves out: an opacity of one, and a stroke's width and
  // dash where there is no stroke.
  const fillOpacity = (s: IrShape) => (s.fill && s.fill !== 'none' && s.fill_opacity !== 1 ? n(s.fill_opacity ?? 1) : undefined);
  const stroked = (s: IrShape) => !!s.stroke && s.stroke !== 'none';
  const strokeOpacity = (s: IrShape) => (stroked(s) && s.fill_opacity !== 1 ? n(s.fill_opacity ?? 1) : undefined);
</script>

{#snippet draw(shapes: IrShape[], fixed: boolean)}
  {#each shapes as s, k (k)}
    {#if skip(s)}
      <!-- left out -->
    {:else if s.kind === 'group' && s.matrix}
      {@const id = clips.get(s)}
      {#if id}
        <clipPath {id}><polygon points={points(s.clip ?? [])} /></clipPath>
        <g data-shape={tag(s)} clip-path="url(#{id})"
          ><g transform={matrix(s.matrix)}>{@render draw(s.shapes ?? [], s.strokes === 'fixed')}</g></g
        >
      {:else}
        <g data-shape={tag(s)} transform={matrix(s.matrix)}>{@render draw(s.shapes ?? [], s.strokes === 'fixed')}</g>
      {/if}
    {:else if s.kind === 'group'}
      <g data-shape={tag(s)} transform="translate({n(s.tx ?? 0)} {n(s.ty ?? 0)}) scale({n(s.scale ?? 1)})">{@render draw(s.shapes ?? [], fixed)}</g>
    {:else if s.kind === 'polygon'}
      <polygon
        data-shape={tag(s)}
        class={mark(s)}
        points={points(s.points ?? [])}
        fill={paint(s.fill)}
        fill-opacity={fillOpacity(s)}
        stroke={paint(s.stroke)}
        stroke-width={stroked(s) ? n(s.stroke_width ?? 0) : undefined}
        stroke-opacity={strokeOpacity(s)}
        stroke-dasharray={stroked(s) ? s.dash : undefined}
        vector-effect={fixed && stroked(s) ? 'non-scaling-stroke' : undefined}
      />
    {:else if s.kind === 'path'}
      <path
        data-shape={tag(s)}
        class={mark(s)}
        d={s.d}
        fill={paint(s.fill)}
        fill-opacity={fillOpacity(s)}
        stroke={paint(s.stroke)}
        stroke-width={stroked(s) ? n(s.stroke_width ?? 0) : undefined}
        stroke-opacity={strokeOpacity(s)}
        stroke-dasharray={stroked(s) ? s.dash : undefined}
        vector-effect={fixed && stroked(s) ? 'non-scaling-stroke' : undefined}
      />
    {:else if s.kind === 'ellipse'}
      <ellipse
        data-shape={tag(s)}
        class={mark(s)}
        cx={n(s.cx ?? 0)}
        cy={n(s.cy ?? 0)}
        rx={n(s.rx ?? 0)}
        ry={n(s.ry ?? 0)}
        fill={paint(s.fill)}
        fill-opacity={fillOpacity(s)}
        stroke={paint(s.stroke)}
        stroke-width={stroked(s) ? n(s.stroke_width ?? 0) : undefined}
        stroke-opacity={strokeOpacity(s)}
        stroke-dasharray={stroked(s) ? s.dash : undefined}
        vector-effect={fixed && stroked(s) ? 'non-scaling-stroke' : undefined}
      />
    {:else if s.kind === 'circle'}
      <circle
        data-shape={tag(s)}
        class={mark(s)}
        cx={n(s.cx ?? 0)}
        cy={n(s.cy ?? 0)}
        r={n(s.r ?? 0)}
        fill={paint(s.fill)}
        fill-opacity={fillOpacity(s)}
        stroke={paint(s.stroke)}
        stroke-width={stroked(s) ? n(s.stroke_width ?? 0) : undefined}
        stroke-opacity={strokeOpacity(s)}
        stroke-dasharray={stroked(s) ? s.dash : undefined}
        vector-effect={fixed && stroked(s) ? 'non-scaling-stroke' : undefined}
      />
    {:else if s.kind === 'rect'}
      <rect
        data-shape={tag(s)}
        class={mark(s)}
        x={n(s.x ?? 0)}
        y={n(s.y ?? 0)}
        width={n(s.w ?? 0)}
        height={n(s.h ?? 0)}
        rx={(s.rx ?? 0) > 0 ? n(s.rx ?? 0) : undefined}
        fill={paint(s.fill)}
        fill-opacity={fillOpacity(s)}
        stroke={paint(s.stroke)}
        stroke-width={stroked(s) ? n(s.stroke_width ?? 0) : undefined}
        stroke-opacity={strokeOpacity(s)}
        stroke-dasharray={stroked(s) ? s.dash : undefined}
        vector-effect={fixed && stroked(s) ? 'non-scaling-stroke' : undefined}
      />
    {/if}
  {/each}
{/snippet}

{@render draw(shapes, false)}
