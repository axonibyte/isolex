// Measure (#117, docs/studio-interface.md, Tools), as Inkscape's measure
// tool: the distance and the angle from one point to another, in tiles on
// a 3-D floor (or a top face, at its height), or in output units on a flat
// canvas, with how far along each axis. Pure.

import { clean } from './snap';

export interface Measured {
  /** The two points: x, y and z in tiles, or x and y in units. */
  from: number[];
  to: number[];
  /** How far along each axis. */
  delta: number[];
  distance: number;
  /** Degrees, 0 to 360: on a floor from +x round toward +y; flat,
      counter-clockwise from +x on a screen whose y runs down, as an arc
      measures it. */
  angle: number;
}

export function measure(from: number[], to: number[]): Measured {
  const delta = to.map((v, i) => clean(v - from[i]!));
  const distance = clean(Math.hypot(...delta));
  const flat = from.length === 2;
  const [dx, dy] = [delta[0]!, delta[1]!];
  const raw = dx || dy ? (Math.atan2(flat ? -dy : dy, dx) * 180) / Math.PI : 0;
  return { from, to, delta, distance, angle: clean(Math.round((((raw % 360) + 360) % 360) * 10) / 10) };
}

const f = (v: number) => (Math.round(v * 100) / 100).toString();

/** What it says beside the pointer: "1.41 tiles · 45° · Δx 1 Δy 1". */
export function said(m: Measured): string {
  const flat = m.from.length === 2;
  const unit = flat ? (m.distance === 1 ? 'unit' : 'units') : m.distance === 1 ? 'tile' : 'tiles';
  const axes = (flat ? ['x', 'y'] : ['x', 'y', 'z']).map((a, i) => `Δ${a} ${f(m.delta[i]!)}`).filter((_, i) => flat || i < 2 || m.delta[2]);
  return `${f(m.distance)} ${unit} · ${f(m.angle)}° · ${axes.join(' ')}`;
}
