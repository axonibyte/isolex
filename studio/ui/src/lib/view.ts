// The canvas's geometry (docs/studio-interface.md, The canvas): the
// projection, what the wheel does, and fitting an icon to the window.

/** One tile unit along an isometric axis, in output units (SPEC §7). */
export const S = 40;
export const COS = 0.8660254037844387;
export const SIN = 0.5;

/** A tile point on screen: x runs right and down, y left and down, z up. */
export const project = (x: number, y: number, z: number): [number, number] => [(x - y) * COS * S, (x + y) * SIN * S - z * S];

/** A screen point on the floor at height `z`, in tile units. */
export function onFloor([X, Y]: [number, number], z = 0): [number, number] {
  const a = X / (COS * S);
  const b = (Y + z * S) / (SIN * S);
  return [(a + b) / 2, (b - a) / 2];
}

/** How the canvas is seen: a scale, and where its origin is on screen. */
export interface Camera {
  k: number;
  x: number;
  y: number;
}

export type WheelAction = { kind: 'zoom'; factor: number } | { kind: 'pan'; dx: number; dy: number };

/** What a turn of the wheel does: it zooms; with Ctrl (or Cmd) it pans
    north and south, with Shift east and west. */
export function wheel(e: { deltaX: number; deltaY: number; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }): WheelAction {
  if (e.ctrlKey || e.metaKey) return { kind: 'pan', dx: 0, dy: -e.deltaY };
  if (e.shiftKey) return { kind: 'pan', dx: -(e.deltaY || e.deltaX), dy: 0 };
  return { kind: 'zoom', factor: Math.exp(-e.deltaY * 0.0025) };
}

const MIN = 0.25;
const MAX = 64;

/** The camera zoomed by `factor` about the screen point `at`. */
export function zoomAt(c: Camera, factor: number, at: [number, number]): Camera {
  const k = Math.min(MAX, Math.max(MIN, c.k * factor));
  const f = k / c.k;
  return { k, x: at[0] - (at[0] - c.x) * f, y: at[1] - (at[1] - c.y) * f };
}

/** The camera that shows `box` (x, y, width, height) in a `w` by `h` window,
    filling about two thirds of it. */
export function fit(box: [number, number, number, number], w: number, h: number): Camera {
  const [bx, by, bw, bh] = box;
  const k = Math.min(MAX, Math.max(MIN, Math.min((w * 0.66) / (bw || 1), (h * 0.66) / (bh || 1))));
  return { k, x: w / 2 - k * (bx + bw / 2), y: h / 2 - k * (by + bh / 2) };
}

/** A screen point in canvas units. */
export const toCanvas = (c: Camera, p: [number, number]): [number, number] => [(p[0] - c.x) / c.k, (p[1] - c.y) / c.k];
