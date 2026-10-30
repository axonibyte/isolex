// The window's panels and where they are (#119, docs/studio-interface.md,
// Floating panels): each of the right side's tabs docked there, or
// floating over the window as Inkscape's dialogs float, where it was
// left, kept per person. Pure.

export type Panel = 'design' | 'arrange' | 'source' | 'tree' | 'preview';

/** The panels, in the order the right side's tabs list them. */
export const PANELS: [Panel, string][] = [
  ['design', 'Design'],
  ['arrange', 'Arrange'],
  ['source', 'Source'],
  ['tree', 'Tree'],
  ['preview', 'Preview'],
];

/** Where a floating panel is in the window, and its size. */
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Layout {
  /** The panels floating, by id; the rest are docked. */
  floating: Partial<Record<Panel, Box>>;
}

/** The window's size. */
export interface Size {
  w: number;
  h: number;
}

/** A floating panel's size, unless it was resized. */
export const FLOAT_SIZE: Size = { w: 360, h: 460 };
const MIN: Size = { w: 240, h: 160 };
/** How much of a panel stays in the window, however far it is dragged:
    enough of its title bar to drag it back by. */
const KEEP: Size = { w: 120, h: 32 };

/** `box` kept in a window of `size`: no smaller than a panel can be, and
    with its title bar in reach. */
export function within(box: Box, size: Size): Box {
  const w = Math.max(MIN.w, box.w);
  const h = Math.max(MIN.h, box.h);
  return {
    x: Math.round(Math.min(Math.max(box.x, KEEP.w - w), Math.max(0, size.w - KEEP.w))),
    y: Math.round(Math.min(Math.max(box.y, 0), Math.max(0, size.h - KEEP.h))),
    w: Math.round(w),
    h: Math.round(h),
  };
}

/** The panels docked, in order. */
export const docked = (l: Layout): Panel[] => PANELS.map(([id]) => id).filter((id) => !l.floating[id]);

/** The panels floating, in order. */
export const floating = (l: Layout): Panel[] => PANELS.map(([id]) => id).filter((id) => l.floating[id]);

/** `id` floating with its top left at `at`, at the size it last had. */
export function float(l: Layout, id: Panel, at: { x: number; y: number }, size: Size, last: Size = FLOAT_SIZE): Layout {
  return { floating: { ...l.floating, [id]: within({ ...at, w: last.w, h: last.h }, size) } };
}

/** `id` docked again. */
export function dock(l: Layout, id: Panel): Layout {
  const rest = { ...l.floating };
  delete rest[id];
  return { floating: rest };
}

/** A floating `id` moved by `by`, or resized to `to`. */
export function moved(l: Layout, id: Panel, by: { x: number; y: number }, size: Size): Layout {
  const b = l.floating[id];
  return b ? { floating: { ...l.floating, [id]: within({ ...b, x: b.x + by.x, y: b.y + by.y }, size) } } : l;
}

export function resized(l: Layout, id: Panel, to: Size, size: Size): Layout {
  const b = l.floating[id];
  return b ? { floating: { ...l.floating, [id]: within({ ...b, w: to.w, h: to.h }, size) } } : l;
}

/** Every floating panel kept in a window of `size`, as one smaller than
    the window it was left in. */
export function fitted(l: Layout, size: Size): Layout {
  return { floating: Object.fromEntries(Object.entries(l.floating).map(([id, b]) => [id, within(b!, size)])) };
}

/** The layout as stored, if it can be read: panels that are no more,
    and boxes that are not numbers, left out. */
export function readLayout(raw: string | null): Layout {
  try {
    const parsed = JSON.parse(raw ?? '{}') as { floating?: Record<string, Partial<Box>> };
    const ids = new Set<string>(PANELS.map(([id]) => id));
    const floating: Partial<Record<Panel, Box>> = {};
    for (const [id, b] of Object.entries(parsed?.floating ?? {})) {
      if (ids.has(id) && b && [b.x, b.y, b.w, b.h].every((n) => typeof n === 'number' && Number.isFinite(n))) floating[id as Panel] = b as Box;
    }
    return { floating };
  } catch {
    return { floating: {} };
  }
}
