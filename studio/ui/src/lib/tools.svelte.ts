// The tools (#69, #83, docs/studio-interface.md, Tools): what a press, a
// drag and a release on the canvas do with each, and what the canvas
// draws over the icon meanwhile (handles, the shape being drawn, the snap
// indicator, the size beside the pointer). Each gesture's edits are asks
// the core plans, one undo step a gesture; one it cannot make alone comes
// back as a choice, which the popover offers.

import type { Answer, Ask, Map3, NodeAt } from './backend';
import type { Studio } from './state.svelte';
import { shifted } from './arrange';
import { around, composed, head, idle, inTiles, middle, pivotHandles, pivotStep, runs, stepped, wrapped, type Step } from './transform';
import { PROJECTION, apply, counted, flatTargets, footprint, onFloor, onWall, solids as solidsOf, targets, top, topFace, zScale, FLAT, type Point, type Point3, type Solid } from './geometry';
import {
  boxText,
  changes,
  cylinderText,
  faceOf,
  faceRectText,
  height,
  lift,
  moveTo,
  nudge,
  poly3dText,
  radius,
  rect,
  resizeCorner,
  resizeFace,
  segText,
  shadowText,
  size,
  tileText,
  type Change,
} from './gesture';
import { angleOf, angled, arcPoints, arcText, atAngle, ellipsePoints, ellipseText, flatStep, lineText, moveFlat, plainData, polyText, rectText, resizeRect, resizeRound, snapAngle, squared, withPoints } from './flat';
import { insertPoint, moveHandle, movePoint, nearestSide, parseAll, polyline, removePoint, shiftData, toD, toDAll, toggle, type Path } from './path';
import { clean, PULL, snapPoint, snapValue, type Snapped } from './snap';
import { atPath, find, same, within } from './select';

export { PROJECTION };

/** The flat plane: output units are the viewBox's. */
const PLANE: Map3 = [1, 0, 0, 1, 0, 0, 0, 0];

/** A press, a move or a release on the canvas. */
export interface Press {
  /** Where, on the viewBox. */
  at: Point;
  /** The shape under the pointer, by index, and the handle, by name. */
  shape: number | null;
  handle: string | null;
  alt: boolean;
  shift: boolean;
  /** The viewBox's pixels per unit: the camera's zoom. */
  scale: number;
}

/** A handle on the selected shape. */
export interface Handle {
  id: string;
  at: Point;
  kind: 'corner' | 'height' | 'radius' | 'point' | 'control' | 'advance' | 'rotate' | 'scale';
  label: string;
  /** A curve's handle is drawn from its point. */
  from?: Point;
  /** The point a path or a polygon starts at. */
  first?: boolean;
}

/** The tools that draw by clicking point after point. */
const CLICKED = new Set(['seg', 'poly3d', 'poly', 'pen']);
/** The tools that draw. */
const DRAWS = new Set(['box', 'tile', 'cylinder', 'facerect', 'shadow', 'seg', 'poly3d', 'line', 'rect', 'ellipse', 'arc', 'poly', 'pen', 'text']);

/** What a drag changes, from the numbers it started with. */
type Drag =
  | { kind: 'move'; solid: Solid; from: Point; grab: Point }
  | { kind: 'lift'; solid: Solid; from: Point }
  | { kind: 'handle'; solid: Solid; handle: string; from: Point }
  | { kind: 'node'; solid: Solid; handle: string; from: Point; moved: boolean; alt: boolean }
  | { kind: 'pending'; solid: Solid; from: Point; again: boolean };

/** A turn handle being dragged (#115): round the box of what is selected,
    about its middle, and the step it has come to. */
interface Pivot {
  handle: string;
  from: Point;
  center: Point;
  box: { lo: Point; hi: Point };
  step: Step | null;
}

type Drawing =
  | { phase: 'footprint'; tool: 'box' | 'tile'; z: number; a: Point; b: Point }
  | { phase: 'height'; tool: 'box' | 'cylinder'; z: number; a: Point; b: Point; r: number; from: number; h: number }
  | { phase: 'radius'; z: number; c: Point; r: number }
  | { phase: 'shadow'; c: Point; from: Point; rx: number; ry: number }
  | { phase: 'face'; face: 'front' | 'right'; wall: number; map: Map3; a: Point; b: Point }
  | { phase: 'seg'; a: Point3 }
  | { phase: 'points3'; z: number; pts: Point3[] }
  | { phase: 'flat'; tool: 'line' | 'rect' | 'ellipse'; a: Point; b: Point }
  | { phase: 'arc'; c: Point; r: number; a0: number }
  | { phase: 'sweep'; c: Point; r: number; a0: number; a1: number }
  | { phase: 'poly'; pts: Point[] }
  | { phase: 'pen'; path: Path; held: Point | null; closing: boolean };

export class Tools {
  /** The tool, by its id in the keymap. */
  tool = $state('select');
  /** The snap step, in tiles; 0 is off. Flat shapes snap at four units a
      tile. */
  step = $state(0.25);
  /** Lines of the shape being drawn, on the viewBox. */
  preview = $state<Point[][]>([]);
  /** The points of a polygon or a path being drawn, the first filled. */
  dots = $state<{ at: Point; first: boolean }[]>([]);
  /** The face a face rect would be drawn on, under the pointer. */
  highlight = $state<Point[] | null>(null);
  /** What a point snapped to, and where. */
  indicator = $state<{ at: Point; what: string } | null>(null);
  /** The size or place being set, beside the pointer. */
  feedback = $state<{ at: Point; text: string } | null>(null);
  /** A choice the core needs, at the handle that asked. */
  choice = $state<{ id: number; options: { label: string; detail: string }[]; at: Point } | null>(null);
  /** The point of a path or a polygon the node tool touched last, which
      Delete removes. */
  touched = $state<number | null>(null);

  private drag: Drag | null = null;
  private pivot: Pivot | null = null;
  private drawing = $state<Drawing | null>(null);
  /** The edits of a gesture, in order, each after the last has landed. */
  private queue: Promise<unknown> = Promise.resolve();
  private gesture = false;
  private sent = '';
  /** Where the selected row is in the outline, by which a drag finds its
      node again after each edit moves it in the source. */
  private path: string | null = null;

  constructor(private readonly studio: Studio) {}

  /** What is open is flat: a canvas, not a footprint (a font's sketch
      is one). */
  get flat(): boolean {
    return !!this.studio.drawing?.ir.canvas;
  }

  /** The sheet flat shapes are drawn on: output units, or the em grid of
      the glyph open (x right, y up). */
  get sheet(): Map3 {
    return this.studio.sketch?.glyph?.map ?? PLANE;
  }

  /** The step flat points snap to, in the plane's units: four units a
      tile step, or in a glyph a tenth of the em a tile step. */
  get planeStep(): number {
    const g = this.studio.sketch?.glyph;
    if (!g) return flatStep(this.step);
    return this.step ? (g.metrics.em * this.step) / 10 : 0;
  }

  /** Points of the plane, on the viewBox. */
  private inPlane(lines: Point[][]): Point[][] {
    return lines.map((l) => l.map((p) => apply(this.sheet, [p[0], p[1], 0])));
  }

  /** Something is being drawn. */
  get busy(): boolean {
    return !!this.drawing;
  }

  /** The solids drawn now, a face rect's wall from the outline. */
  get solids(): Solid[] {
    const s = this.studio;
    const d = s.drawing;
    const all = solidsOf(d?.sources ?? [], s.editingNode, d ? counted(d.ir.shapes) : []);
    for (const solid of all) {
      if (solid.kind === 'facerect' && solid.select) {
        const note = find(s.outline, solid.select)?.row?.note;
        solid.face = note === 'right' ? 'right' : 'front';
      }
    }
    return all;
  }

  /** The selected node's one solid, if it draws one. */
  get selected(): Solid | null {
    const sel = this.studio.selected;
    if (!sel) return null;
    const mine = this.solids.filter((s) => s.select && same(s.select, sel));
    return mine.length === 1 ? mine[0]! : null;
  }

  /** The selection the turn handles are shown for, after a second click
      on it (#115). */
  private turning = $state<NodeAt | null>(null);

  /** The turn handles are shown: the select tool's, on the selection
      they were asked for. */
  get pivoting(): boolean {
    const sel = this.studio.selected;
    return this.tool === 'select' && !!sel && !!this.turning && same(this.turning, sel) && !this.studio.previewing;
  }

  /** What every node selected draws, round which the turn handles go. */
  private get chosenSolids(): Solid[] {
    return this.solids.filter((sol) => sol.select && this.studio.chosen.some((n) => within(sol.select!, n)));
  }

  /** The selected solid's handles: its size with the select tool, its
      points with the node tool; after a second click on the selection,
      the turn handles round all of it. */
  get handles(): Handle[] {
    if (this.pivoting) {
      const b = around(this.chosenSolids, false);
      return b ? pivotHandles(b) : [];
    }
    // a glyph's advance, with the select tool, whatever is selected
    const g = this.studio.sketch?.glyph;
    const advance: Handle[] =
      g && this.tool === 'select' ? [{ id: 'advance', at: apply(g.map, [this.advanceAt ?? g.advance, 0, 0]), kind: 'advance', label: 'Advance' }] : [];
    return [...this.shapeHandles, ...advance];
  }

  /** Where a glyph's advance is being dragged to, in its em grid. */
  advanceAt = $state<number | null>(null);
  private advancing = false;

  private get shapeHandles(): Handle[] {
    const s = this.selected;
    // a preview is drawn from other numbers: nothing on it is the source's
    if (!s || this.studio.previewing) return [];
    if (this.tool === 'node') return this.nodeHandles(s);
    if (this.tool !== 'select') return [];
    const v = s.values;
    const out: Handle[] = [];
    if (FLAT.has(s.kind)) {
      const at = (p: Point) => apply(s.map, [p[0], p[1], 0]);
      if (s.kind === 'rect') {
        const [x, y, w, h] = v as [number, number, number, number];
        const corners: Point[] = [
          [x, y],
          [x + w, y],
          [x + w, y + h],
          [x, y + h],
        ];
        corners.forEach((p, k) => out.push({ id: `rect-${k}`, at: at(p), kind: 'corner', label: `Corner ${k + 1}` }));
      }
      if (s.kind === 'circle' || s.kind === 'arc') out.push({ id: 'round-r', at: at([v[0]! + v[2]!, v[1]!]), kind: 'radius', label: 'Radius' });
      if (s.kind === 'ellipse') {
        out.push({ id: 'round-rx', at: at([v[0]! + v[2]!, v[1]!]), kind: 'radius', label: 'Radius across' });
        out.push({ id: 'round-ry', at: at([v[0]!, v[1]! + v[3]!]), kind: 'radius', label: 'Radius down' });
      }
      return out;
    }
    const [x, y, z] = v as [number, number, number];
    const at = (p: Point3) => apply(s.map, p);
    if (s.kind === 'box' || s.kind === 'tile') {
      footprint(s, z).forEach((p, k) => out.push({ id: `corner-${k}`, at: at(p), kind: 'corner', label: `Corner ${k + 1}` }));
    }
    if (s.kind === 'box') out.push({ id: 'height', at: at([x + v[3]! / 2, y + v[4]! / 2, top(s)]), kind: 'height', label: 'Height' });
    if (s.kind === 'cylinder') {
      out.push({ id: 'height', at: at([x, y, top(s)]), kind: 'height', label: 'Height' });
      out.push({ id: 'radius', at: at([x + v[3]!, y, z]), kind: 'radius', label: 'Radius' });
    }
    if (s.kind === 'facerect') {
      const [a, b, fz, extent, h] = v as [number, number, number, number, number];
      const right = faceOf(s) === 'right';
      const along = right ? b : a;
      const pts: Point[] = [
        [along, fz],
        [along + extent, fz],
        [along + extent, fz + h],
        [along, fz + h],
      ];
      pts.forEach(([u, w], k) => out.push({ id: `face-${k}`, at: at(right ? [a, u, w] : [u, b, w]), kind: 'corner', label: `Corner ${k + 1}` }));
    }
    return out;
  }

  /** A path's, a polygon's or a line's points, and a path's curve
      handles. */
  private nodeHandles(s: Solid): Handle[] {
    const at = (p: Point) => apply(s.map, [p[0], p[1], 0]);
    const out: Handle[] = [];
    if (s.kind === 'path') {
      // every subpath's points, numbered on from one to the next
      const paths = s.d ? (parseAll(s.d) ?? []) : [];
      const knots = paths.flatMap((q) => q.knots.map((k, i) => ({ k, first: i === 0 })));
      knots.forEach(({ k }, i) => {
        if (k.in) out.push({ id: `in-${i}`, at: at(k.in), from: at(k.p), kind: 'control', label: `Handle into point ${i + 1}` });
        if (k.out) out.push({ id: `out-${i}`, at: at(k.out), from: at(k.p), kind: 'control', label: `Handle out of point ${i + 1}` });
      });
      knots.forEach(({ k, first }, i) => out.push({ id: `pt-${i}`, at: at(k.p), kind: 'point', label: `Point ${i + 1}${k.smooth ? ', smooth' : ''}`, first }));
    } else if (s.kind === 'poly' || s.kind === 'line') {
      pairs(s.values).forEach((p, i) => out.push({ id: `pt-${i}`, at: at(p), kind: 'point', label: `Point ${i + 1}`, first: i === 0 }));
    }
    return out;
  }

  // --- the pointer ---------------------------------------------------------

  down(p: Press) {
    this.choice = null;
    if (this.studio.previewing) {
      this.studio.message = 'This is a preview: set the param as its default, or reset it, to draw';
      return;
    }
    if (DRAWS.has(this.tool)) return this.downDraw(p);
    if (this.tool === 'node') return this.downNode(p);
    if (this.tool !== 'select') return;
    if (p.handle === 'advance') {
      this.advancing = true;
      return;
    }
    if (p.handle && this.pivoting && /^(rot|scale)-/.test(p.handle)) {
      const box = around(this.chosenSolids, false);
      this.drag = null;
      if (box) this.pivot = { handle: p.handle, from: p.at, center: [(box.lo[0] + box.hi[0]) / 2, (box.lo[1] + box.hi[1]) / 2], box, step: null };
      return;
    }
    const sel = this.selected;
    if (p.handle && sel) {
      this.drag = { kind: 'handle', solid: sel, handle: p.handle, from: p.at };
      return this.begin(sel);
    }
    const solid = p.shape === null ? null : (this.solids.find((s) => s.shapes.includes(p.shape!)) ?? null);
    // Shift+click adds to what is selected, or takes out, as the click
    // says: no drag
    if (p.shift) {
      this.drag = null;
      return;
    }
    if (solid?.select && !this.locked(solid.select)) {
      // a press on a shape selects it, and a drag from there moves it; one
      // drawn through what it selects (a text's glyph, a fragment's shape)
      // is not the selection's to move
      const again = !!this.studio.selected && within(solid.select, this.studio.selected);
      if (!again) this.studio.select(solid.select);
      if (!same(solid.select, solid.node)) {
        // a second click on a group selected shows its turn handles
        if (again) this.toggleTurning();
        this.drag = null;
        return;
      }
      this.drag = { kind: 'pending', solid, from: p.at, again };
      return this.begin(solid);
    }
    this.drag = null;
  }

  /** The pointer moved: with a button held (`held`), a drag; without, what
      is being drawn follows it. */
  move(p: Press, held = true) {
    if (this.drawing) return this.moveDraw(p, held);
    if (!held) return this.hover(p);
    const d = this.drag;
    if (this.advancing) return this.moveAdvance(p);
    const pv = this.pivot;
    if (pv) {
      pv.step = pivotStep(pv.handle, pv.center, pv.from, p.at, p.shift);
      this.preview = pv.step ? [stepped(pv.box, pv.center, pv.step)] : [];
      const s = pv.step;
      this.feedback = s ? { at: p.at, text: s.kind === 'rotate' ? `rotate ${s.deg}°` : s.kind === 'scale' ? `scale ${clean(s.sx * 100)}% × ${clean(s.sy * 100)}%` : '' } : null;
      return;
    }
    if (!d) return;
    if (d.kind === 'node') return this.moveNode(d, p);
    if (d.kind === 'pending') {
      if (Math.hypot(p.at[0] - d.from[0], p.at[1] - d.from[1]) * p.scale < 4) return;
      const s = d.solid;
      const grab = this.plane(s, d.from);
      if (p.alt && s.kind !== 'shadow' && !FLAT.has(s.kind)) this.drag = { kind: 'lift', solid: s, from: d.from };
      else if (grab) this.drag = { kind: 'move', solid: s, from: d.from, grab };
      else return;
      if (s.copy) this.studio.message = 'This is one copy of a repeat: an edit changes every copy';
    }
    const drag = this.drag!;
    if (FLAT.has(drag.solid.kind)) return this.dragFlat(drag, p);
    const next = this.dragged(drag, p);
    if (next) this.send(drag.solid, next, this.label(drag));
  }

  up(p: Press) {
    if (this.drawing) return this.upDraw(p);
    const d = this.drag;
    this.drag = null;
    this.indicator = null;
    this.feedback = null;
    if (d?.kind === 'node' && !d.moved && d.alt && d.handle.startsWith('pt-')) this.toggleNode(d.solid, Number(d.handle.slice(3)));
    // a click, not a drag, on what was selected: the turn handles, or not
    if (d?.kind === 'pending' && d.again) this.toggleTurning();
    const pv = this.pivot;
    this.pivot = null;
    if (pv) {
      this.preview = [];
      const step = pv.step;
      if (step && !idle(step)) {
        const n = this.studio.chosen.length;
        const verb = step.kind === 'rotate' ? 'Rotate' : 'Scale';
        // what it is wrapped in keeps the handles
        void this.transform(step, `${verb} ${n === 1 ? 'one shape' : `${n} shapes`} in ${this.studio.editing}`).then(() => (this.turning = this.studio.selected));
      }
    }
    if (this.advancing) {
      this.advancing = false;
      this.setAdvance(p);
    }
    this.end();
    // a turn handle's drag is not a click on nothing
    return !pv && (d?.kind === 'pending' || !d);
  }

  /** The turn handles shown on the selection, or its own again. */
  private toggleTurning() {
    this.turning = this.pivoting ? null : this.studio.selected;
  }

  /** A double-click: with the node tool, a point added on the side under
      the pointer. */
  double(p: Press) {
    if (this.tool !== 'node') return;
    const s = this.selected;
    if (!s) return;
    const here = onFloor(s.map, p.at, 0);
    if (!here) return;
    const reach = PULL / (p.scale * Math.abs(s.map[0] || 1));
    if (s.kind === 'path') {
      const paths = s.d ? parseAll(s.d) : null;
      if (!paths) return;
      // the nearest side of any subpath
      let best: { j: number; side: number; t: number; d: number } | null = null;
      paths.forEach((q, j) => {
        const near = nearestSide(q, here);
        if (near && (!best || near.d < best.d)) best = { j, ...near };
      });
      const b = best as { j: number; side: number; t: number; d: number } | null;
      if (!b || b.d > reach) return;
      this.begin(s);
      this.writePaths(s, replaced(paths, b.j, insertPoint(paths[b.j]!, b.side, b.t)), `Add a point to path in ${this.studio.editing}`);
      this.end();
    } else if (s.kind === 'poly') {
      const pts = pairs(s.values);
      let best: { i: number; d: number; at: Point } | null = null;
      pts.forEach((a, i) => {
        const b = pts[(i + 1) % pts.length]!;
        const q = nearest(a, b, here);
        const d = Math.hypot(q[0] - here[0], q[1] - here[1]);
        if (!best || d < best.d) best = { i, d, at: q };
      });
      const b = best as { i: number; d: number; at: Point } | null;
      if (!b || b.d > reach) return;
      const next = pts.slice();
      next.splice(b.i + 1, 0, [clean(b.at[0]), clean(b.at[1])]);
      this.begin(s);
      this.writePoly(s, next, `Add a point to poly in ${this.studio.editing}`);
      this.end();
    }
  }

  /** Esc: what is being drawn, or the drag, stops. */
  cancel(): boolean {
    const busy = !!(this.drawing || this.drag || this.choice || this.pivot);
    this.drawing = null;
    this.drag = null;
    this.pivot = null;
    this.choice = null;
    this.clear();
    this.end();
    return busy;
  }

  private clear() {
    this.preview = [];
    this.dots = [];
    this.indicator = null;
    this.feedback = null;
  }

  // --- dragging the selection ---------------------------------------------------

  /** Where a press is on the plane a solid is moved in: its floor, a face
      rect's wall as (along, z), or a flat shape's plane. */
  private plane(s: Solid, at: Point): Point | null {
    if (s.kind === 'facerect') {
      const right = faceOf(s) === 'right';
      return onWall(s.map, at, faceOf(s), right ? s.values[0]! : s.values[1]!);
    }
    if (FLAT.has(s.kind)) return onFloor(s.map, at, 0);
    return onFloor(s.map, at, s.kind === 'shadow' ? 0 : s.values[2]!);
  }

  private dragged(d: Drag, p: Press): number[] | null {
    const s = d.solid;
    const v = s.values;
    const up = (d.from[1] - p.at[1]) / zScale(s.map);
    if (d.kind === 'lift') {
      const next = lift(s, up, this.step);
      this.feedback = { at: p.at, text: `z ${clean(next[2]!)}` };
      return next;
    }
    if (d.kind === 'move') {
      const here = this.plane(s, p.at);
      if (!here) return null;
      if (s.kind === 'facerect') {
        const right = faceOf(s) === 'right';
        const start: Point = [right ? v[1]! : v[0]!, v[2]!];
        const to: Point = [snapValue(start[0] + here[0] - d.grab[0], this.step), Math.max(0, snapValue(start[1] + here[1] - d.grab[1], this.step))];
        this.feedback = { at: p.at, text: `${right ? 'b' : 'a'} ${to[0]} · z ${to[1]}` };
        return moveTo(s, to);
      }
      const z = s.kind === 'shadow' ? 0 : v[2]!;
      const snapped = this.snap(s, [v[0]! + here[0] - d.grab[0], v[1]! + here[1] - d.grab[1]], z, p.scale);
      this.feedback = { at: p.at, text: `x ${snapped.point[0]} · y ${snapped.point[1]}` };
      return moveTo(s, snapped.point);
    }
    if (d.kind === 'handle') {
      if (d.handle === 'height') {
        const next = height(s, up, this.step);
        this.feedback = { at: p.at, text: `h ${clean(next[s.kind === 'box' ? 5 : 4]!)}` };
        return next;
      }
      if (d.handle === 'radius') {
        const here = onFloor(s.map, p.at, v[2]!);
        if (!here) return null;
        const next = radius(s, here, this.step);
        this.feedback = { at: p.at, text: `r ${clean(next[3]!)}` };
        return next;
      }
      const k = Number(d.handle.split('-')[1]);
      if (d.handle.startsWith('face-')) {
        const here = this.plane(s, p.at);
        if (!here) return null;
        const next = resizeFace(s, k, [snapValue(here[0], this.step), Math.max(0, snapValue(here[1], this.step))]);
        this.feedback = { at: p.at, text: size(next[3]!, next[4]!) };
        return next;
      }
      const here = onFloor(s.map, p.at, v[2]!);
      if (!here) return null;
      const snapped = this.snap(s, here, v[2]!, p.scale);
      const next = resizeCorner(s, k, snapped.point);
      this.feedback = { at: p.at, text: size(next[3]!, next[4]!) };
      return next;
    }
    return null;
  }

  /** A flat shape moved in its plane, its first point at the flat step,
      or one of its handles dragged. */
  private dragFlat(d: Drag, p: Press) {
    const s = d.solid;
    const here = onFloor(s.map, p.at, 0);
    if (!here) return;
    const label = this.label(d);
    if (d.kind === 'handle') {
      const to = this.snapFlat(here, s, p.scale);
      const next = d.handle.startsWith('rect-') ? resizeRect(s.values, Number(d.handle.slice(5)), to) : resizeRound(s.kind, s.values, d.handle.slice(6), to);
      this.feedback = { at: p.at, text: s.kind === 'rect' ? size(next[2]!, next[3]!) : d.handle === 'round-ry' ? `ry ${next[3]}` : `${d.handle === 'round-rx' ? 'rx' : 'r'} ${next[2]}` };
      return this.send(s, next, label);
    }
    if (d.kind !== 'move') return;
    const ref = first(s);
    if (!ref) return;
    const to = this.snapFlat([ref[0] + here[0] - d.grab[0], ref[1] + here[1] - d.grab[1]], s, p.scale);
    const by: Point = [clean(to[0] - ref[0]), clean(to[1] - ref[1])];
    this.feedback = { at: p.at, text: `x ${to[0]} · y ${to[1]}` };
    this.moveFlatBy(s, by, label);
  }

  /** A flat shape moved by `by`, written as its kind is. */
  private moveFlatBy(s: Solid, by: Point, label: string) {
    const key = JSON.stringify(by);
    if (key === this.sent) return;
    this.sent = key;
    if (s.kind === 'path') {
      if (!s.d) return;
      return this.writeData(s, shiftData(s.d, by), label);
    }
    const next = moveFlat(s.kind, s.values, by);
    if (s.kind === 'poly') return this.writePoly(s, pairs(next), label);
    this.edit(label, apply(s.map, [s.values[0]!, s.values[1]!, 0]), changes(s, next).map(setArg));
  }

  private label(d: Drag): string {
    const what = `${d.solid.kind} in ${this.studio.editing}`;
    if (d.kind === 'lift') return `Lift ${what}`;
    if (d.kind === 'handle') return d.handle === 'height' ? `Change height of ${what}` : d.handle === 'radius' || d.handle.startsWith('round-') ? `Change radius of ${what}` : `Resize ${what}`;
    if (d.kind === 'node') return `Shape ${what}`;
    return `Move ${what}`;
  }

  /** Snaps a point on height `z` of a solid's floor. */
  private snap(s: Solid, p: Point, z: number, scale: number): Snapped {
    const snapped = snapPoint(
      p,
      this.step,
      targets(this.solids, s.node, z),
      (q) => apply(s.map, [q[0], q[1], z]),
      (v) => onFloor(s.map, v, z),
      scale,
    );
    this.indicator = snapped.what && snapped.at ? { at: snapped.at, what: snapped.what } : null;
    return snapped;
  }

  /** Snaps a flat point to another flat shape's point or the flat step:
      one in the plane `s` is drawn in, or, with no `s`, one of the viewBox
      in the plane drawn on (`plane`). */
  private snapFlat(at: Point, s: Solid | null, scale: number): Point {
    const map = s?.map ?? this.sheet;
    const p = s ? at : (onFloor(map, at, 0) ?? at);
    const snapped = snapPoint(
      p,
      this.planeStep,
      flatTargets(this.solids, s?.node ?? null),
      (q) => apply(map, [q[0], q[1], 0]),
      (v) => onFloor(map, v, 0),
      scale,
    );
    this.indicator = snapped.what && snapped.at ? { at: snapped.at, what: snapped.what } : null;
    return snapped.point;
  }

  private locked(node: NodeAt): boolean {
    return !!this.studio.outline?.parts.some((p) => this.studio.locked.has(p.name) && within(node, p.node));
  }

  // --- a glyph's advance -------------------------------------------------------

  private moveAdvance(p: Press) {
    const g = this.studio.sketch?.glyph;
    const at = g && onFloor(g.map, p.at, 0);
    if (!g || !at) return;
    this.advanceAt = Math.max(0, snapValue(at[0], this.planeStep));
    this.feedback = { at: p.at, text: `advance ${this.advanceAt}` };
  }

  /** The advance dragged to, written once, on release: its node's span
      changes with it. */
  private setAdvance(p: Press) {
    const to = this.advanceAt;
    const s = this.studio;
    this.advanceAt = null;
    const part = s.outline?.parts.find((q) => q.name === s.glyph);
    if (to === null || !part || to === s.sketch?.glyph?.advance) return;
    void this.change(part.node, `Set the advance of ${part.name}`, p.at, [(node) => ({ kind: 'set-prop', node, key: 'advance', value: to })]);
  }

  // --- the node tool ------------------------------------------------------------

  private downNode(p: Press) {
    const sel = this.selected;
    if (p.handle && sel) {
      if (p.handle.startsWith('pt-')) this.touched = Number(p.handle.slice(3));
      this.drag = { kind: 'node', solid: sel, handle: p.handle, from: p.at, moved: false, alt: p.alt };
      return this.begin(sel);
    }
    // a press elsewhere selects what is under it
    const solid = p.shape === null ? null : (this.solids.find((s) => s.shapes.includes(p.shape!)) ?? null);
    this.touched = null;
    if (solid?.select && !this.locked(solid.select)) this.studio.select(solid.select);
    else this.studio.select(null);
    if (solid && !['path', 'poly', 'line'].includes(solid.kind)) this.studio.message = 'The node tool shapes paths, polygons and lines';
  }

  private moveNode(d: Extract<Drag, { kind: 'node' }>, p: Press) {
    if (!d.moved && Math.hypot(p.at[0] - d.from[0], p.at[1] - d.from[1]) * p.scale < 3) return;
    d.moved = true;
    const s = d.solid;
    const here = onFloor(s.map, p.at, 0);
    if (!here) return;
    const to = this.snapFlat(here, s, p.scale);
    const [which, k] = d.handle.split('-') as ['pt' | 'in' | 'out', string];
    const i = Number(k);
    this.feedback = { at: p.at, text: `x ${to[0]} · y ${to[1]}` };
    const label = `Shape ${s.kind} in ${this.studio.editing}`;
    if (s.kind === 'path') {
      const paths = s.d ? parseAll(s.d) : null;
      const at = paths && locate(paths, i);
      if (!paths || !at) return;
      const [j, k] = at;
      const path = paths[j]!;
      return this.writePaths(s, replaced(paths, j, which === 'pt' ? movePoint(path, k, to) : moveHandle(path, k, which, to, p.alt)), label);
    }
    const pts = pairs(s.values);
    pts[i] = to;
    if (s.kind === 'poly') return this.writePoly(s, pts, label);
    const next = pts.flat();
    const key = JSON.stringify(next);
    if (key === this.sent) return;
    this.sent = key;
    this.edit(label, p.at, changes(s, next).map(setArg));
  }

  /** Alt+click on a path's point: corner to smooth, or back. */
  private toggleNode(s: Solid, i: number) {
    const paths = s.kind === 'path' && s.d ? parseAll(s.d) : null;
    const at = paths && locate(paths, i);
    if (!paths || !at) return;
    const [j, k] = at;
    this.begin(s);
    this.writePaths(s, replaced(paths, j, toggle(paths[j]!, k)), `${paths[j]!.knots[k]?.smooth ? 'Make a corner' : 'Smooth a point'} of path in ${this.studio.editing}`);
  }

  /** Delete with the node tool: the point touched last, while the shape
      keeps enough. */
  private removeNode(): boolean {
    const s = this.selected;
    const i = this.touched;
    if (!s || i === null) return false;
    this.begin(s);
    const label = `Delete a point of ${s.kind} in ${this.studio.editing}`;
    if (s.kind === 'path') {
      const paths = s.d ? parseAll(s.d) : null;
      const at = paths && locate(paths, i);
      const next = paths && at && removePoint(paths[at[0]]!, at[1]);
      if (!paths || !at || !next) {
        this.studio.message = 'A path keeps two points at least, three when closed';
        return true;
      }
      this.writePaths(s, replaced(paths, at[0], next), label);
    } else if (s.kind === 'poly') {
      const pts = pairs(s.values);
      if (pts.length <= 3) {
        this.studio.message = 'A polygon keeps three points at least';
        return true;
      }
      pts.splice(i, 1);
      this.writePoly(s, pts, label);
    } else return false;
    this.touched = null;
    this.end();
    return true;
  }

  // --- the edits ------------------------------------------------------------------

  private begin(s: Solid) {
    this.sent = '';
    this.path = s.select ? (find(this.studio.outline, s.select)?.path ?? null) : null;
  }

  /** Sends what changed since the solid was as `s` is, if it is not what
      was sent last. */
  private send(s: Solid, next: number[], label: string) {
    const all = changes(s, next);
    const key = JSON.stringify(all);
    if (key === this.sent) return;
    this.sent = key;
    const at: Point3 = FLAT.has(s.kind) ? [s.values[0]!, s.values[1]!, 0] : [s.values[0]!, s.values[1]!, s.values[2] ?? 0];
    this.edit(label, apply(s.map, at), all.map(setArg));
  }

  /** A path's data written anew. */
  /** A path's subpaths written anew. */
  private writePaths(s: Solid, paths: Path[], label: string) {
    this.writeData(s, toDAll(paths), label);
  }

  private writeData(s: Solid, d: string, label: string) {
    if (d === this.sent) return;
    this.sent = d;
    this.edit(label, apply(s.map, [first(s)?.[0] ?? 0, first(s)?.[1] ?? 0, 0]), [
      async (node) => {
        if (!plainData(await this.text(node))) return this.refuse('This path’s data is worked out from expressions: change it in the Source tab');
        return { kind: 'set-prop', node, key: 'd', value: d };
      },
    ]);
  }

  /** A polygon's points written anew. */
  private writePoly(s: Solid, pts: Point[], label: string) {
    const key = JSON.stringify(pts);
    if (key === this.sent) return;
    this.sent = key;
    this.edit(label, apply(s.map, [pts[0]![0], pts[0]![1], 0]), [
      async (node) => {
        const text = withPoints(await this.text(node), pts);
        if (text === null) return this.refuse('This polygon’s points are written as expressions or a repeat: change them in the Source tab');
        return { kind: 'type', file: node.file, at: node.start, len: node.len, text };
      },
    ]);
  }

  private refuse(why: string): null {
    this.studio.message = why;
    return null;
  }

  /** The text of a node, as the source has it now. */
  async text(node: NodeAt): Promise<string> {
    const src = await this.studio.backend.source(node.file);
    return src ? new TextDecoder().decode(new TextEncoder().encode(src.text).slice(node.start, node.start + node.len)) : '';
  }

  /** One edit of `node` from a panel, one undo step: its asks in turn,
      a choice the core needs offered on the canvas at `at`. A node the
      outline does not list (a param, a frame) is asked of as it is. */
  change(node: NodeAt, label: string, at: Point, steps: ((node: NodeAt) => Ask | null | Promise<Ask | null>)[]) {
    this.sent = '';
    this.path = find(this.studio.outline, node)?.path ?? null;
    this.edit(label, at, steps, this.path ? null : node);
    this.end();
    return this.settled();
  }

  /** Each step's ask in turn, on the node as it is after the last, the
      gesture begun first; on `fixed`, when given, as it is. */
  edit(label: string, at: Point, steps: ((node: NodeAt) => Ask | null | Promise<Ask | null>)[], fixed: NodeAt | null = null) {
    const path = this.path;
    this.queue = this.queue.then(async () => {
      if (!this.gesture) {
        this.gesture = true;
        await this.studio.backend.beginGesture();
      }
      for (const step of steps) {
        const f = path ? atPath(this.studio.outline, path) : null;
        // a row's node, or a part's: a font's glyph is one
        const node = fixed ?? f?.row?.node ?? f?.part.node;
        if (!node) return;
        const ask = await step(node);
        if (!ask) return;
        const answer = await this.studio.ask(ask, label);
        if (!(await this.answered(answer, at))) return;
      }
    });
  }

  /** What an answer means for the gesture: on with it, or stopped for a
      choice or a refusal. */
  private async answered(answer: Answer, at: Point): Promise<boolean> {
    if (answer.kind === 'applied') {
      await this.studio.refresh();
      return true;
    }
    this.drag = null;
    this.drawing = null;
    this.feedback = null;
    if (answer.kind === 'choose') this.choice = { id: answer.id, options: answer.options, at };
    return false;
  }

  /** The person's pick of the choice. */
  async choose(n: number) {
    const c = this.choice;
    this.choice = null;
    if (!c) return;
    const answer = await this.studio.backend.choose(c.id, n);
    if (answer.kind === 'refused') this.studio.message = answer.reason;
    else await this.studio.refresh();
    this.end();
  }

  private end() {
    this.queue = this.queue.then(async () => {
      if (this.gesture) {
        this.gesture = false;
        await this.studio.backend.endGesture();
      }
    });
  }

  /** Waits for every edit sent so far. */
  settled(): Promise<unknown> {
    return this.queue;
  }

  // --- the keys -------------------------------------------------------------------

  /** A key for the tool; true if it did something. */
  key(e: { key: string; shift: boolean; ctrl: boolean }): boolean {
    if (e.key === 'Enter' && this.drawing) return this.finish();
    if (this.tool === 'node' && (e.key === 'Delete' || e.key === 'Backspace')) return this.removeNode();
    const sel = this.studio.selected;
    if (!sel || this.tool !== 'select') return false;
    if (e.ctrl && e.key.toLowerCase() === 'd') {
      void this.duplicate(sel);
      return true;
    }
    if (e.key === 'Delete' || e.key === 'Backspace') {
      void this.remove(sel);
      return true;
    }
    const s = this.selected;
    if (!s) return false;
    this.begin(s);
    if (FLAT.has(s.kind)) {
      // by the step, or with Shift a tile's worth (a tenth of a glyph's em);
      // up is up, in a glyph's grid as on the canvas
      const glyph = this.studio.sketch?.glyph;
      const by = e.shift ? (glyph ? glyph.metrics.em / 10 : 4) : this.planeStep || (glyph ? glyph.metrics.em / 100 : 0.5);
      const up = glyph ? by : -by;
      const d = ({ ArrowRight: [by, 0], ArrowLeft: [-by, 0], ArrowDown: [0, -up], ArrowUp: [0, up] } as Record<string, Point>)[e.key];
      if (!d) return false;
      this.moveFlatBy(s, d, `Nudge ${s.kind} in ${this.studio.editing}`);
      this.end();
      return true;
    }
    const by = e.shift ? 1 : this.step || 0.125;
    const next = nudge(s, e.key, by);
    if (!next) return false;
    this.edit(`Nudge ${s.kind} in ${this.studio.editing}`, apply(s.map, [s.values[0]!, s.values[1]!, 0]), changes(s, next).map(setArg));
    this.end();
    return true;
  }

  /** Each solid moved by its `by` (along x, y and z, or a flat shape's
      x and y), as one undo step: Align and Distribute. A value written
      as an expression asks, as any edit does. */
  arrange(moves: { solid: Solid; by: number[] }[], label: string) {
    for (const { solid, by } of moves) {
      if (by.every((d) => !d)) continue;
      this.begin(solid);
      if (FLAT.has(solid.kind)) this.moveFlatBy(solid, [by[0]!, by[1]!], label);
      else this.edit(label, apply(solid.map, [solid.values[0]!, solid.values[1]!, solid.values[2] ?? 0]), changes(solid, shifted(solid, by)).map(setArg));
    }
    this.end();
    return this.settled();
  }

  /** The nodes chosen drawn through `step`, as one undo step (#115): a
      transform or turn selected takes the step if it is one of its kind,
      written as a number; else each run of neighbors chosen is wrapped in
      one, about the middle of what they draw. Says why when it cannot. */
  transform(step: Step, label: string): Promise<unknown> {
    this.queue = this.queue.then(async () => {
      const s = this.studio;
      if (idle(step)) return this.refuse('That changes nothing');
      const found = s.chosen.map((n) => find(s.outline, n));
      if (!found.length) return this.refuse('Select shapes to transform');
      if (found.some((f) => !f?.row)) return this.refuse('A part is not transformed: select the shapes in it');
      const rows = found.map((f) => f!);
      if (rows.some((f) => f.row!.kind === 'pt')) return this.refuse('A point of a polygon is not transformed: select the polygon');
      if (rows.length === 1) {
        const f = rows[0]!;
        const c = composed(f.row!.kind, await this.text(f.row!.node), step);
        if (c) {
          const ask: Ask = 'key' in c ? { kind: 'set-prop', node: f.row!.node, key: c.key, value: c.value } : { kind: 'set-arg', node: f.row!.node, index: c.index, value: c.value };
          if ((await s.ask(ask, label)).kind === 'applied') await s.refresh();
          return;
        }
      }
      // what is in an anchor is drawn flat: only the screen's steps
      const inAnchor = (path: string) => {
        const steps = path.split('/');
        for (let i = 2; i < steps.length; i++) if (atPath(s.outline, steps.slice(0, i).join('/'))?.row?.kind === 'at') return true;
        return false;
      };
      if (inTiles(step) && rows.some((f) => inAnchor(f.path) || FLAT.has(f.row!.kind) || f.row!.kind === 'text')) {
        return this.refuse('Only 3-D shapes turn in tiles: rotate or mirror flat shapes on the screen');
      }
      const drawn = this.solids.filter((sol) => sol.select && s.chosen.some((n) => within(sol.select!, n)));
      const origin = middle(drawn, inTiles(step)) ?? [0, 0];
      const file = rows[0]!.row!.node.file;
      const src = await s.backend.source(file);
      if (!src || rows.some((f) => f.row!.node.file !== file)) return this.refuse('The shapes are written in more than one file');
      const bytes = new TextEncoder().encode(src.text);
      const decode = (a: number, b: number) => new TextDecoder().decode(bytes.slice(a, b));
      const groups = runs(rows.map((f) => ({ ...f, at: f.row!.node.start })));
      const opened = head(step, origin);
      const asks: Ask[] = groups
        .map((g) => {
          const start = g[0]!.at;
          const last = g.at(-1)!.row!.node;
          const end = last.start + last.len;
          const lineStart = bytes.lastIndexOf(0x0a, start - 1) + 1;
          const indent = decode(lineStart, start).match(/^\s*/)![0];
          return { kind: 'type' as const, file, at: start, len: end - start, text: wrapped(decode(start, end), indent, opened) };
        })
        // the last first, so where each earlier one is stays put
        .sort((a, b) => b.at - a.at);
      const firstPath = groups[0]![0]!.path;
      await s.backend.beginGesture();
      let applied = true;
      for (const ask of asks) applied = (await s.ask(ask, label)).kind === 'applied' && applied;
      await s.backend.endGesture();
      await s.refresh();
      // what it is wrapped in is selected, to add to
      if (applied) {
        const again = atPath(s.outline, firstPath);
        if (again?.row) s.select(again.row.node);
      }
    });
    return this.settled();
  }

  /** The selected node again, beside it: after it in its part. */
  async duplicate(node: NodeAt) {
    const f = find(this.studio.outline, node);
    if (!f?.row) return;
    const text = await this.text(node);
    const parent = f.path.split('/').length > 2 ? atPath(this.studio.outline, f.path.slice(0, f.path.lastIndexOf('/')))!.row!.node : f.part.node;
    const before = f.siblings[f.index + 1]?.node ?? null;
    const answer = await this.studio.ask({ kind: 'insert', parent, text, before }, `Duplicate ${f.row.kind} in ${this.studio.editing}`);
    if (answer.kind === 'applied') {
      await this.studio.refresh();
      const path = f.path.replace(/\d+$/, String(f.index + 1));
      const again = atPath(this.studio.outline, path);
      if (again?.row) this.studio.select(again.row.node);
    }
  }

  async remove(node: NodeAt) {
    const f = find(this.studio.outline, node);
    if (!f?.row) return;
    const answer = await this.studio.ask({ kind: 'delete', node }, `Delete ${f.row.kind} in ${this.studio.editing}`);
    if (answer.kind === 'applied') this.studio.select(null);
  }

  // --- drawing ----------------------------------------------------------------------

  /** The face of a box under the pointer, for the face rect tool. */
  private hover(p: Press) {
    if (this.tool !== 'facerect') return;
    this.highlight = this.faceUnder(p)?.outline ?? null;
  }

  private faceUnder(p: Press): { face: 'front' | 'right'; wall: number; map: Map3; outline: Point[] } | null {
    if (p.shape === null) return null;
    const box = this.solids.find((s) => s.kind === 'box' && (s.shapes[0] === p.shape || s.shapes[1] === p.shape));
    if (!box) return null;
    const [x, y, z, w, d, h] = box.values as [number, number, number, number, number, number];
    const front = box.shapes[0] === p.shape;
    const corners: Point3[] = front
      ? [
          [x, y + d, z],
          [x + w, y + d, z],
          [x + w, y + d, z + h],
          [x, y + d, z + h],
        ]
      : [
          [x + w, y, z],
          [x + w, y + d, z],
          [x + w, y + d, z + h],
          [x + w, y, z + h],
        ];
    return { face: front ? 'front' : 'right', wall: front ? y + d : x + w, map: box.map, outline: [...corners, corners[0]!].map((c) => apply(box.map, c)) };
  }

  /** The floor under a press: the top face of a shape under it, or the
      ground. */
  private floorOf(p: Press): { z: number; under: Solid | null } {
    const under = p.shape === null ? null : (this.solids.find((s) => topFace(s) === p.shape) ?? null);
    return { z: under ? clean(top(under)) : 0, under };
  }

  private downDraw(p: Press) {
    const d = this.drawing;
    // a second click finishes what the first began
    if (d?.phase === 'height') {
      this.drawing = null;
      this.clear();
      if (d.tool === 'cylinder') void this.insert(cylinderText(d.a, d.z, d.r, d.h), 'cylinder');
      else void this.insert(boxText(d.a, d.b, d.z, d.h), 'box');
      return;
    }
    if (d?.phase === 'sweep') {
      this.drawing = null;
      this.clear();
      if (d.a1 !== d.a0) void this.insertFlat(arcText(d.c, d.r, d.a0, d.a1), 'arc');
      return;
    }
    switch (this.tool) {
      case 'box':
      case 'tile':
      case 'cylinder': {
        const { z, under } = this.floorOf(p);
        const at = this.floor(p, z);
        if (!at) return;
        this.drawing = this.tool === 'cylinder' ? { phase: 'radius', z, c: at, r: 0 } : { phase: 'footprint', tool: this.tool as 'box' | 'tile', z, a: at, b: at };
        this.studio.message = under ? `On top of the ${under.kind}, at z ${z}` : '';
        return;
      }
      case 'shadow': {
        const c = this.floor(p, 0);
        if (c) this.drawing = { phase: 'shadow', c, from: apply(PROJECTION, [c[0], c[1], 0]), rx: 0, ry: 0 };
        return;
      }
      case 'facerect': {
        const f = this.faceUnder(p);
        if (!f) {
          this.studio.message = 'Press on the front or the right side of a box';
          return;
        }
        const a = this.wallPoint(f, p);
        if (a) this.drawing = { phase: 'face', face: f.face, wall: f.wall, map: f.map, a, b: a };
        return;
      }
      case 'seg': {
        const { z } = this.floorOf(p);
        const at = this.floor(p, z);
        if (!at) return;
        const here: Point3 = [at[0], at[1], z];
        if (d?.phase === 'seg') {
          this.drawing = null;
          this.clear();
          if (here.some((v, i) => v !== d.a[i])) void this.insert(segText(d.a, here), 'seg');
          return;
        }
        this.drawing = { phase: 'seg', a: here };
        this.studio.message = 'Click the other end';
        return;
      }
      case 'poly3d': {
        const z = d?.phase === 'points3' ? d.z : this.floorOf(p).z;
        const at = this.floor(p, z);
        if (!at) return;
        if (d?.phase === 'points3') {
          const start = apply(PROJECTION, d.pts[0]!);
          if (d.pts.length >= 3 && Math.hypot(start[0] - p.at[0], start[1] - p.at[1]) * p.scale <= PULL) return void this.finish();
          d.pts.push([at[0], at[1], z]);
        } else this.drawing = { phase: 'points3', z, pts: [[at[0], at[1], z]] };
        this.studio.message = 'Click each point; click the first, or press Enter, to close';
        return;
      }
      case 'text': {
        // words, set in the library's first font, on the anchor drawn on
        const font = this.studio.view.tree?.fonts[0]?.name;
        if (this.studio.font) this.studio.message = 'A glyph cannot set text: draw it with the pen and the shapes';
        else if (!font) this.studio.message = 'The library has no font: import one from the Tree tab';
        else {
          const at = this.snapFlat(p.at, null, p.scale);
          void this.insertFlat(`text ${clean(at[0])} ${clean(at[1])} "Text" font="${font}" size=8`, 'text');
        }
        return;
      }
      case 'line':
      case 'rect':
      case 'ellipse': {
        const a = this.snapFlat(p.at, null, p.scale);
        this.drawing = { phase: 'flat', tool: this.tool, a, b: a };
        return;
      }
      case 'arc': {
        const c = this.snapFlat(p.at, null, p.scale);
        this.drawing = { phase: 'arc', c, r: 0, a0: 0 };
        return;
      }
      case 'poly': {
        const at = this.snapFlat(p.at, null, p.scale);
        if (d?.phase === 'poly') {
          const s = d.pts[0]!;
          if (d.pts.length >= 3 && Math.hypot(s[0] - p.at[0], s[1] - p.at[1]) * p.scale <= PULL) return void this.finish();
          d.pts.push(at);
        } else this.drawing = { phase: 'poly', pts: [at] };
        this.studio.message = 'Click each corner; click the first, or press Enter, to close';
        this.drawPoints();
        return;
      }
      case 'pen': {
        const at = this.snapFlat(p.at, null, p.scale);
        if (d?.phase === 'pen') {
          const s = d.path.knots[0]!.p;
          if (d.path.knots.length >= 2 && Math.hypot(s[0] - p.at[0], s[1] - p.at[1]) * p.scale <= PULL) {
            // closing on the first point: a drag there sets its handles
            d.closing = true;
            d.held = s;
            return;
          }
          d.path.knots.push({ p: at, in: null, out: null, smooth: false });
          d.held = at;
        } else this.drawing = { phase: 'pen', path: { knots: [{ p: at, in: null, out: null, smooth: false }], closed: false }, held: at, closing: false };
        this.studio.message = 'Click for a corner, drag for a smooth point; click the first point to close, Enter to leave it open';
        this.drawPen(null);
        return;
      }
    }
  }

  private moveDraw(p: Press, held: boolean) {
    const d = this.drawing!;
    switch (d.phase) {
      case 'footprint': {
        if (!held) return;
        const b = this.floor(p, d.z);
        if (!b) return;
        d.b = b;
        this.preview = this.outline(d.a, b, d.z, 0);
        const [, , w, depth] = rect(d.a, b);
        this.feedback = { at: p.at, text: size(w, depth) };
        return;
      }
      case 'height': {
        d.h = Math.max(0, snapValue((d.from - p.at[1]) / zScale(PROJECTION), this.step));
        if (d.tool === 'cylinder') {
          this.preview = [this.ring(d.a, d.r, d.z), this.ring(d.a, d.r, d.z + d.h)];
          this.feedback = { at: p.at, text: `r ${d.r} · h ${d.h}` };
        } else {
          this.preview = this.outline(d.a, d.b, d.z, d.h);
          const [, , w, depth] = rect(d.a, d.b);
          this.feedback = { at: p.at, text: size(w, depth, d.h) };
        }
        return;
      }
      case 'radius': {
        if (!held) return;
        const at = onFloor(PROJECTION, p.at, d.z);
        if (!at) return;
        d.r = snapValue(Math.hypot(at[0] - d.c[0], at[1] - d.c[1]), this.step);
        this.preview = [this.ring(d.c, d.r, d.z)];
        this.feedback = { at: p.at, text: `r ${d.r}` };
        return;
      }
      case 'shadow': {
        if (!held) return;
        d.rx = snapValue(Math.abs(p.at[0] - d.from[0]) / PROJECTION[0], this.step);
        d.ry = snapValue(Math.abs(p.at[1] - d.from[1]) / PROJECTION[1], this.step);
        this.preview = [ellipsePoints(d.from, d.rx * PROJECTION[0], d.ry * PROJECTION[1])];
        this.feedback = { at: p.at, text: `${d.rx} × ${d.ry}` };
        return;
      }
      case 'face': {
        if (!held) return;
        const b = this.wallPoint(d, p);
        if (!b) return;
        d.b = b;
        const [along, z, extent, h] = rect(d.a, b);
        const at = (u: number, w: number) => apply(d.map, d.face === 'front' ? [u, d.wall, w] : [d.wall, u, w]);
        this.preview = [[at(along, z), at(along + extent, z), at(along + extent, z + h), at(along, z + h), at(along, z)]];
        this.feedback = { at: p.at, text: size(extent, h) };
        return;
      }
      case 'seg': {
        const { z } = this.floorOf(p);
        const at = this.floor(p, z);
        if (at) this.preview = [[apply(PROJECTION, d.a), apply(PROJECTION, [at[0], at[1], z])]];
        return;
      }
      case 'points3': {
        const at = this.floor(p, d.z);
        if (!at) return;
        const pts = d.pts.map((q) => apply(PROJECTION, q));
        this.preview = [pts, [pts.at(-1)!, apply(PROJECTION, [at[0], at[1], d.z])]];
        this.dots = pts.map((q, i) => ({ at: q, first: i === 0 }));
        return;
      }
      case 'flat': {
        if (!held) return;
        let b = this.snapFlat(p.at, null, p.scale);
        if (p.shift) b = d.tool === 'line' ? angled(d.a, b, this.planeStep) : squared(d.a, b);
        d.b = b;
        const [x, y, w, h] = rect(d.a, b);
        if (d.tool === 'line') {
          this.preview = this.inPlane([[d.a, b]]);
          this.feedback = { at: p.at, text: `${clean(Math.hypot(b[0] - d.a[0], b[1] - d.a[1]))} · ${clean(angleOf(d.a, b))}°` };
        } else if (d.tool === 'rect') {
          this.preview = this.inPlane([
            [
              [x, y],
              [x + w, y],
              [x + w, y + h],
              [x, y + h],
              [x, y],
            ],
          ]);
          this.feedback = { at: p.at, text: size(w, h) };
        } else {
          this.preview = this.inPlane([ellipsePoints([x + w / 2, y + h / 2], w / 2, h / 2)]);
          this.feedback = { at: p.at, text: size(w, h) };
        }
        return;
      }
      case 'arc': {
        if (!held) return;
        const b = this.snapFlat(p.at, null, p.scale);
        d.r = clean(Math.hypot(b[0] - d.c[0], b[1] - d.c[1]));
        d.a0 = snapAngle(angleOf(d.c, b), p.shift);
        this.preview = this.inPlane([[d.c, atAngle(d.c, d.r, d.a0)]]);
        this.feedback = { at: p.at, text: `r ${d.r} · ${d.a0}°` };
        return;
      }
      case 'sweep': {
        const a = snapAngle(angleOf(d.c, p.at), p.shift);
        // counter-clockwise from where it starts
        d.a1 = clean(d.a0 + ((((a - d.a0) % 360) + 360) % 360));
        this.preview = d.a1 === d.a0 ? [] : this.inPlane([arcPoints(d.c, d.r, d.a0, d.a1)]);
        this.feedback = { at: p.at, text: `${d.a0}° to ${d.a1}°` };
        return;
      }
      case 'poly': {
        const at = this.snapFlat(p.at, null, p.scale);
        this.drawPoints(at);
        return;
      }
      case 'pen': {
        const at = this.snapFlat(p.at, null, p.scale);
        if (held && d.held) {
          // dragging out a smooth point's handles, mirrored
          const k = d.closing ? d.path.knots[0]! : d.path.knots.at(-1)!;
          if (Math.hypot(at[0] - k.p[0], at[1] - k.p[1]) > 0) {
            k.out = at;
            k.in = [clean(2 * k.p[0] - at[0]), clean(2 * k.p[1] - at[1])];
            k.smooth = true;
          }
          this.drawPen(null);
        } else this.drawPen(at);
        return;
      }
    }
  }

  private upDraw(p: Press) {
    const d = this.drawing!;
    this.indicator = null;
    switch (d.phase) {
      case 'footprint': {
        const [, , w, depth] = rect(d.a, d.b);
        if (!w || !depth) return this.stop();
        if (d.tool === 'tile') {
          this.stop();
          void this.insert(tileText(d.a, d.b, d.z), 'tile');
          return false;
        }
        // then the height: up from here, and a click to finish
        this.drawing = { phase: 'height', tool: 'box', z: d.z, a: d.a, b: d.b, r: 0, from: p.at[1], h: 0 };
        this.studio.message = 'Move up for the height, and click';
        return false;
      }
      case 'radius': {
        if (!d.r) return this.stop();
        this.drawing = { phase: 'height', tool: 'cylinder', z: d.z, a: d.c, b: d.c, r: d.r, from: p.at[1], h: 0 };
        this.studio.message = 'Move up for the height, and click';
        return false;
      }
      case 'shadow': {
        this.stop();
        if (d.rx && d.ry) void this.insert(shadowText(d.c, d.rx, d.ry), 'shadow');
        return false;
      }
      case 'face': {
        const [, , extent, h] = rect(d.a, d.b);
        this.stop();
        if (extent && h) void this.insert(faceRectText(d.face, d.wall, d.a, d.b), 'facerect');
        return false;
      }
      case 'flat': {
        const [, , w, h] = rect(d.a, d.b);
        const drawn = d.tool === 'line' ? w || h : w && h;
        this.stop();
        if (!drawn) return false;
        const text = d.tool === 'line' ? lineText(d.a, d.b) : d.tool === 'rect' ? rectText(d.a, d.b) : ellipseText(d.a, d.b);
        void this.insertFlat(text, text.split(' ')[0]!);
        return false;
      }
      case 'arc': {
        if (!d.r) return this.stop();
        this.drawing = { phase: 'sweep', c: d.c, r: d.r, a0: d.a0, a1: d.a0 };
        this.studio.message = 'Move round for the sweep, and click';
        return false;
      }
      case 'pen': {
        if (d.closing) {
          d.path.closed = true;
          return void this.finish();
        }
        d.held = null;
        return false;
      }
    }
    return false;
  }

  /** Nothing drawn: back to the tool's start. */
  private stop(): false {
    this.drawing = null;
    this.clear();
    return false;
  }

  /** Enter, or a click on the first point: a polygon closed, or a path
      ended open (or closed, from the first point). */
  private finish(): boolean {
    const d = this.drawing;
    if (!d) return false;
    if (d.phase === 'poly') {
      if (d.pts.length < 3) return true;
      this.stop();
      void this.insertFlat(polyText(d.pts), 'poly');
      return true;
    }
    if (d.phase === 'points3') {
      if (d.pts.length < 3) return true;
      this.stop();
      void this.insert(poly3dText(d.pts), 'poly3d');
      return true;
    }
    if (d.phase === 'pen') {
      if (d.path.knots.length < 2) return true;
      this.stop();
      void this.insertFlat(`path d="${toD(d.path)}"`, 'path');
      return true;
    }
    return false;
  }

  private drawPoints(to?: Point) {
    const d = this.drawing;
    if (d?.phase !== 'poly') return;
    this.preview = this.inPlane(to ? [d.pts, [d.pts.at(-1)!, to]] : [d.pts]);
    this.dots = this.inPlane([d.pts])[0]!.map((at, i) => ({ at, first: i === 0 }));
  }

  /** The path being drawn, the side to the pointer dashed, its points and
      their handles. */
  private drawPen(to: Point | null) {
    const d = this.drawing;
    if (d?.phase !== 'pen') return;
    const lines = [polyline(d.path)];
    if (to) {
      const last = d.path.knots.at(-1)!;
      lines.push(polyline({ knots: [last, { p: to, in: null, out: null, smooth: false }], closed: false }));
    }
    for (const k of d.path.knots) {
      if (k.in) lines.push([k.in, k.p]);
      if (k.out) lines.push([k.p, k.out]);
    }
    this.preview = this.inPlane(lines);
    this.dots = this.inPlane([d.path.knots.map((k) => k.p)])[0]!.map((at, i) => ({ at, first: i === 0 }));
  }

  /** The point on the floor at `z` under a press, snapped. */
  private floor(p: Press, z: number): Point | null {
    const at = onFloor(PROJECTION, p.at, z);
    if (!at) return null;
    const snapped = snapPoint(
      at,
      this.step,
      targets(this.solids, null, z),
      (q) => apply(PROJECTION, [q[0], q[1], z]),
      (v) => onFloor(PROJECTION, v, z),
      p.scale,
    );
    this.indicator = snapped.what && snapped.at ? { at: snapped.at, what: snapped.what } : null;
    return snapped.point;
  }

  /** The point on a box's side under a press, as (along, z) at the step. */
  private wallPoint(f: { face: 'front' | 'right'; wall: number; map: Map3 }, p: Press): Point | null {
    const at = onWall(f.map, p.at, f.face, f.wall);
    return at && [snapValue(at[0], this.step), Math.max(0, snapValue(at[1], this.step))];
  }

  /** A box's edges, or a footprint's, on the viewBox. */
  private outline(a: Point, b: Point, z: number, h: number): Point[][] {
    const [x, y, w, d] = rect(a, b);
    const corners: Point[] = [
      [x, y],
      [x + w, y],
      [x + w, y + d],
      [x, y + d],
    ];
    const ring = (zz: number): Point[] => [...corners, corners[0]!].map(([u, v]) => apply(PROJECTION, [u, v, zz]));
    if (!h) return [ring(z)];
    return [ring(z), ring(z + h), ...corners.map(([u, v]) => [apply(PROJECTION, [u, v, z]), apply(PROJECTION, [u, v, z + h])])];
  }

  /** A circle on the floor at `z`, on the viewBox. */
  private ring(c: Point, r: number, z: number): Point[] {
    return Array.from({ length: 49 }, (_, i) => apply(PROJECTION, [c[0] + r * Math.cos((i * Math.PI) / 24), c[1] + r * Math.sin((i * Math.PI) / 24), z]));
  }

  /** Writes a drawn shape into the part new shapes go into, or a new part
      when the definition has none of its own, and selects it. */
  private async insert(text: string, kind: string) {
    const s = this.studio;
    const o = s.outline;
    if (!o) return;
    const part = o.parts.find((p) => p.name === s.target && p.op !== 'remove');
    const name = part?.name ?? this.newPart();
    const answer = part
      ? await s.ask({ kind: 'insert', parent: part.node, text, before: null }, `Draw ${kind} in ${o.definition}`)
      : await s.ask({ kind: 'insert', parent: o.node, text: `part "${name}" {\n${indent(text)}\n}`, before: null }, `Draw ${kind} in ${o.definition}`);
    if (answer.kind !== 'applied') return;
    s.target = name;
    await s.refresh();
    const row = s.outline?.parts.find((q) => q.name === name)?.shapes.at(-1);
    if (row) s.select(row.node);
    s.message = '';
  }

  /** Writes a flat shape on the anchor at the origin that ends the part
      drawn in, or on a new one, and selects it. */
  private async insertFlat(text: string, kind: string) {
    const s = this.studio;
    const o = s.outline;
    if (!o) return;
    // in a font, into the glyph open: a glyph's nodes are its own plane's
    if (o.kind === 'font') {
      const glyph = o.parts.find((p) => p.name === s.glyph);
      if (!glyph) {
        s.message = 'Open a glyph to draw in it: click one';
        return;
      }
      const answer = await s.ask({ kind: 'insert', parent: glyph.node, text, before: null }, `Draw ${kind} in ${glyph.name} of ${o.definition}`);
      if (answer.kind !== 'applied') return;
      await s.refresh();
      const row = s.outline?.parts.find((q) => q.name === glyph.name)?.shapes.at(-1);
      if (row) s.select(row.node);
      s.message = '';
      return;
    }
    const part = o.parts.find((p) => p.name === s.target && p.op !== 'remove');
    const name = part?.name ?? this.newPart();
    const anchor = part?.shapes.at(-1);
    const shared = anchor?.kind === 'at' && anchor.note === 'at 0 0 0';
    const label = `Draw ${kind} in ${o.definition}`;
    const at = `at 0 0 0 {\n${indent(text)}\n}`;
    const answer = shared
      ? await s.ask({ kind: 'insert', parent: anchor.node, text, before: null }, label)
      : part
        ? await s.ask({ kind: 'insert', parent: part.node, text: at, before: null }, label)
        : await s.ask({ kind: 'insert', parent: o.node, text: `part "${name}" {\n${indent(at)}\n}`, before: null }, label);
    if (answer.kind !== 'applied') return;
    s.target = name;
    await s.refresh();
    const row = s.outline?.parts.find((q) => q.name === name)?.shapes.at(-1)?.children.at(-1);
    if (row) s.select(row.node);
    s.message = '';
  }

  /** A name for a new part, not one the definition has or inherits. */
  private newPart(): string {
    const o = this.studio.outline!;
    const taken = new Set(o.parts.map((p) => p.name).concat(o.inherited.map((p) => p.name)));
    let k = 1;
    while (taken.has(`part-${k}`)) k++;
    return `part-${k}`;
  }
}

/** Which subpath, and which of its points, point `n` is: the points of a
    path's subpaths numbered on from one to the next. */
function locate(paths: Path[], n: number): [number, number] | null {
  for (let j = 0; j < paths.length; j++) {
    const len = paths[j]!.knots.length;
    if (n < len) return [j, n];
    n -= len;
  }
  return null;
}

/** The subpaths, subpath `j` as `path`. */
const replaced = (paths: Path[], j: number, path: Path): Path[] => paths.map((q, k) => (k === j ? path : q));

/** A list of numbers as points, x then y. */
const pairs = (v: number[]): Point[] => {
  const out: Point[] = [];
  for (let i = 0; i + 1 < v.length; i += 2) out.push([v[i]!, v[i + 1]!]);
  return out;
};

/** Where a flat shape is: its first point, or its corner or center. */
function first(s: Solid): Point | null {
  if (s.kind === 'path') {
    const m = s.d?.match(/^\s*[Mm]\s*(-?[\d.]+)[\s,]+(-?[\d.]+)/);
    return m ? [Number(m[1]), Number(m[2])] : null;
  }
  return s.values.length >= 2 ? [s.values[0]!, s.values[1]!] : null;
}

/** The point of the side from `a` to `b` nearest `p`. */
function nearest(a: Point, b: Point, p: Point): Point {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len = dx * dx + dy * dy;
  const t = len ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len)) : 0;
  return [a[0] + t * dx, a[1] + t * dy];
}

const setArg = (c: Change) => (node: NodeAt): Ask => ({ kind: 'set-arg', node, index: c.index, value: c.value });

/** Text one step in. */
const indent = (text: string) =>
  text
    .split('\n')
    .map((l) => `  ${l}`)
    .join('\n');
