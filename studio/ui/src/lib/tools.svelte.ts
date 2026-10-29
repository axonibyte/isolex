// The Select, Box and Tile tools (#69, docs/studio-interface.md, Tools):
// what a press, a drag and a release on the canvas do, and what the
// canvas draws over the icon meanwhile (handles, the shape being drawn,
// the snap indicator, the size beside the pointer). Each gesture's edits
// are asks the core plans, one undo step a gesture; one it cannot make
// alone comes back as a choice, which the popover offers.

import type { Answer, Map3, NodeAt } from './backend';
import type { Studio } from './state.svelte';
import { apply, footprint, onFloor, onWall, solids as solidsOf, targets, top, topFace, zScale, type Point, type Point3, type Solid } from './geometry';
import { boxText, changes, faceOf, height, lift, moveTo, nudge, radius, rect, resizeCorner, resizeFace, size, tileText, type Change } from './gesture';
import { clean, snapPoint, snapValue, type Snapped } from './snap';
import { atPath, find, same, within } from './select';

/** The projection a node written in the edited definition is drawn
    with, unturned (SPEC §7). */
export const PROJECTION: Map3 = [34.64101615137754, 20, -34.64101615137754, 20, 0, -40, 0, 0];

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
  kind: 'corner' | 'height' | 'radius';
  label: string;
}

/** What a drag changes, from the numbers it started with. */
type Drag =
  | { kind: 'move'; solid: Solid; from: Point; grab: Point }
  | { kind: 'lift'; solid: Solid; from: Point }
  | { kind: 'handle'; solid: Solid; handle: string; from: Point }
  | { kind: 'pending'; solid: Solid; from: Point };

type Drawing =
  | { phase: 'footprint'; tool: 'box' | 'tile'; z: number; a: Point; b: Point }
  | { phase: 'height'; z: number; a: Point; b: Point; from: number; h: number };

export class Tools {
  /** The tool: "select", "box", "tile" or "hand". */
  tool = $state('select');
  /** The snap step, in tiles; 0 is off. */
  step = $state(0.25);
  /** Lines of the shape being drawn, on the viewBox. */
  preview = $state<Point[][]>([]);
  /** What a point snapped to, and where. */
  indicator = $state<{ at: Point; what: string } | null>(null);
  /** The size or place being set, beside the pointer. */
  feedback = $state<{ at: Point; text: string } | null>(null);
  /** A choice the core needs, at the handle that asked. */
  choice = $state<{ id: number; options: { label: string; detail: string }[]; at: Point } | null>(null);

  private drag: Drag | null = null;
  private drawing: Drawing | null = null;
  /** The edits of a gesture, in order, each after the last has landed. */
  private queue: Promise<unknown> = Promise.resolve();
  private gesture = false;
  private sent = '';
  /** Where the selected row is in the outline, by which a drag finds its
      node again after each edit moves it in the source. */
  private path: string | null = null;

  constructor(private readonly studio: Studio) {}

  /** The solids drawn now, a face rect's wall from the outline. */
  get solids(): Solid[] {
    const s = this.studio;
    const all = solidsOf(s.icon ? (s.view.sources[s.icon] ?? []) : [], s.editingNode);
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

  /** The selected solid's handles, with the select tool. */
  get handles(): Handle[] {
    const s = this.selected;
    if (!s || this.tool !== 'select') return [];
    const [x, y, z] = s.values as [number, number, number];
    const v = s.values;
    const out: Handle[] = [];
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

  // --- the pointer ---------------------------------------------------------

  down(p: Press) {
    this.choice = null;
    if (this.tool === 'box' || this.tool === 'tile') return this.downDraw(p);
    if (this.tool !== 'select') return;
    const sel = this.selected;
    if (p.handle && sel) {
      this.drag = { kind: 'handle', solid: sel, handle: p.handle, from: p.at };
      return this.begin(sel);
    }
    const solid = p.shape === null ? null : (this.solids.find((s) => s.shapes.includes(p.shape!)) ?? null);
    if (solid?.select && !this.locked(solid.select)) {
      // a press on a shape selects it, and a drag from there moves it
      if (!this.studio.selected || !within(solid.select, this.studio.selected)) this.studio.select(solid.select);
      this.drag = { kind: 'pending', solid, from: p.at };
      return this.begin(solid);
    }
    this.drag = null;
  }

  move(p: Press) {
    if (this.drawing) return this.moveDraw(p);
    const d = this.drag;
    if (!d) return;
    if (d.kind === 'pending') {
      if (Math.hypot(p.at[0] - d.from[0], p.at[1] - d.from[1]) * p.scale < 4) return;
      const s = d.solid;
      const grab = this.plane(s, d.from);
      if (p.alt && s.kind !== 'shadow') this.drag = { kind: 'lift', solid: s, from: d.from };
      else if (grab) this.drag = { kind: 'move', solid: s, from: d.from, grab };
      else return;
      if (s.copy) this.studio.message = 'This is one copy of a repeat: an edit changes every copy';
    }
    const next = this.dragged(this.drag!, p);
    if (next) this.send(this.drag!.solid, next, this.label(this.drag!));
  }

  up(p: Press) {
    if (this.drawing) return this.upDraw(p);
    const d = this.drag;
    this.drag = null;
    this.indicator = null;
    this.feedback = null;
    this.end();
    return d?.kind === 'pending' || !d;
  }

  /** Esc: what is being drawn, or the drag, stops. */
  cancel(): boolean {
    const busy = !!(this.drawing || this.drag || this.choice);
    this.drawing = null;
    this.drag = null;
    this.choice = null;
    this.preview = [];
    this.indicator = null;
    this.feedback = null;
    this.end();
    return busy;
  }

  // --- dragging the selection ---------------------------------------------------

  /** Where a press is on the plane a solid is moved in: its floor, or a
      face rect's wall as (along, z). */
  private plane(s: Solid, at: Point): Point | null {
    if (s.kind === 'facerect') {
      const right = faceOf(s) === 'right';
      return onWall(s.map, at, faceOf(s), right ? s.values[0]! : s.values[1]!);
    }
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

  private label(d: Drag): string {
    const what = `${d.solid.kind} in ${this.studio.editing}`;
    if (d.kind === 'lift') return `Lift ${what}`;
    if (d.kind === 'handle') return d.handle === 'height' ? `Change height of ${what}` : d.handle === 'radius' ? `Change radius of ${what}` : `Resize ${what}`;
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

  private locked(node: NodeAt): boolean {
    return !!this.studio.outline?.parts.some((p) => this.studio.locked.has(p.name) && within(node, p.node));
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
    this.edit(all, label, apply(s.map, [s.values[0]!, s.values[1]!, s.values[2] ?? 0]));
  }

  /** Sets each of `all` in turn, the gesture begun first. */
  edit(all: Change[], label: string, at: Point) {
    const path = this.path;
    this.queue = this.queue.then(async () => {
      if (!this.gesture) {
        this.gesture = true;
        await this.studio.backend.beginGesture();
      }
      for (const c of all) {
        const f = path ? atPath(this.studio.outline, path) : null;
        const node = f?.row?.node;
        if (!node) return;
        const answer = await this.studio.ask({ kind: 'set-arg', node, index: c.index, value: c.value }, label);
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

  /** A key with the Select tool, on the selection; true if it did
      something. */
  key(e: { key: string; shift: boolean; ctrl: boolean }): boolean {
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
    const by = e.shift ? 1 : this.step || 0.125;
    const next = nudge(s, e.key, by);
    if (!next) return false;
    this.begin(s);
    this.edit(changes(s, next), `Nudge ${s.kind} in ${this.studio.editing}`, apply(s.map, [s.values[0]!, s.values[1]!, 0]));
    this.end();
    return true;
  }

  /** The selected node again, beside it: after it in its part. */
  private async duplicate(node: NodeAt) {
    const f = find(this.studio.outline, node);
    const src = await this.studio.backend.source(node.file);
    if (!f?.row || !src) return;
    const text = new TextDecoder().decode(new TextEncoder().encode(src.text).slice(node.start, node.start + node.len));
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

  private async remove(node: NodeAt) {
    const f = find(this.studio.outline, node);
    if (!f?.row) return;
    const answer = await this.studio.ask({ kind: 'delete', node }, `Delete ${f.row.kind} in ${this.studio.editing}`);
    if (answer.kind === 'applied') this.studio.select(null);
  }

  // --- drawing ----------------------------------------------------------------------

  private downDraw(p: Press) {
    const d = this.drawing;
    if (d?.phase === 'height') {
      this.drawing = null;
      this.preview = [];
      this.feedback = null;
      void this.insert(boxText(d.a, d.b, d.z, d.h), 'box');
      return;
    }
    // over a shape's top face, it stands on that face
    const under = p.shape === null ? null : this.solids.find((s) => topFace(s) === p.shape);
    const z = under ? clean(top(under)) : 0;
    const at = this.floor(p, z);
    if (!at) return;
    this.drawing = { phase: 'footprint', tool: this.tool as 'box' | 'tile', z, a: at, b: at };
    this.studio.message = under ? `On top of the ${under.kind}, at z ${z}` : '';
  }

  private moveDraw(p: Press) {
    const d = this.drawing!;
    if (d.phase === 'footprint') {
      const b = this.floor(p, d.z);
      if (!b) return;
      d.b = b;
      this.preview = this.outline(d.a, b, d.z, 0);
      const [, , w, depth] = rect(d.a, b);
      this.feedback = { at: p.at, text: size(w, depth) };
    } else {
      d.h = Math.max(0, snapValue((d.from - p.at[1]) / zScale(PROJECTION), this.step));
      this.preview = this.outline(d.a, d.b, d.z, d.h);
      const [, , w, depth] = rect(d.a, d.b);
      this.feedback = { at: p.at, text: size(w, depth, d.h) };
    }
  }

  private upDraw(p: Press) {
    const d = this.drawing!;
    if (d.phase !== 'footprint') return false;
    const [, , w, depth] = rect(d.a, d.b);
    this.indicator = null;
    if (!w || !depth) {
      // a click, not a drag: nothing drawn
      this.drawing = null;
      this.preview = [];
      this.feedback = null;
      return false;
    }
    if (d.tool === 'tile') {
      this.drawing = null;
      this.preview = [];
      this.feedback = null;
      void this.insert(tileText(d.a, d.b, d.z), 'tile');
      return false;
    }
    // then the height: up from here, and a click to finish
    this.drawing = { phase: 'height', z: d.z, a: d.a, b: d.b, from: p.at[1], h: 0 };
    this.studio.message = 'Move up for the height, and click';
    return false;
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

  /** A box's edges, or a footprint's, on the viewBox. */
  private outline(a: Point, b: Point, z: number, h: number): Point[][] {
    const [x, y, w, d] = rect(a, b);
    const ring = (zz: number): Point[] => {
      const r = (
        [
          [x, y],
          [x + w, y],
          [x + w, y + d],
          [x, y + d],
          [x, y],
        ] as Point[]
      ).map(([u, v]) => apply(PROJECTION, [u, v, zz]));
      return r;
    };
    if (!h) return [ring(z)];
    const lines: Point[][] = [ring(z), ring(z + h)];
    for (const [u, v] of [
      [x, y],
      [x + w, y],
      [x + w, y + d],
      [x, y + d],
    ] as Point[]) {
      lines.push([apply(PROJECTION, [u, v, z]), apply(PROJECTION, [u, v, z + h])]);
    }
    return lines;
  }

  /** Writes a drawn shape into the part new shapes go into, or a new part
      when the definition has none of its own, and selects it. */
  private async insert(text: string, kind: string) {
    const s = this.studio;
    const o = s.outline;
    if (!o) return;
    const part = o.parts.find((p) => p.name === s.target && p.op !== 'remove');
    let answer: Answer;
    let name = part?.name;
    if (part) {
      answer = await s.ask({ kind: 'insert', parent: part.node, text, before: null }, `Draw ${kind} in ${o.definition}`);
    } else {
      const taken = new Set(o.parts.map((p) => p.name).concat(o.inherited.map((p) => p.name)));
      let k = 1;
      while (taken.has(`part-${k}`)) k++;
      name = `part-${k}`;
      answer = await s.ask({ kind: 'insert', parent: o.node, text: `part "${name}" {\n  ${text}\n}`, before: null }, `Draw ${kind} in ${o.definition}`);
    }
    if (answer.kind !== 'applied') return;
    s.target = name ?? null;
    await s.refresh();
    const p = s.outline?.parts.find((q) => q.name === name);
    const row = p?.shapes.at(-1);
    if (row) s.select(row.node);
    s.message = '';
  }
}
