// The tools and their keys (docs/studio-interface.md, Tools and Keys):
// one table, which the rail is drawn from and keys are looked up in. A
// test fails on any key bound twice. Pure.

/** What a tool draws in: a 3-D icon's tiles, a flat icon's units, or
    either. */
export type Dims = '3d' | 'flat' | 'any';

export interface Tool {
  id: string;
  /** Its key, as the hint writes it: "B", "Shift+S". */
  key: string;
  label: string;
  /** Its icon, a path on a 20-unit square. */
  d: string;
  dims: Dims;
  /** Built yet; a tool that is not is shown disabled. */
  ready: boolean;
}

/** The rail, in its groups: selecting, 3-D, flat. A null separates them. */
export const TOOLS: (Tool | null)[] = [
  { id: 'select', key: 'V', label: 'Select and move', d: 'M5 3v13l3.5-3.5 2.6 5 2-1-2.6-5H16z', dims: 'any', ready: true },
  { id: 'node', key: 'A', label: 'Node', d: 'M4 15C6 6 14 6 16 15M2.5 13.5h3v3h-3zM14.5 13.5h3v3h-3zM8.5 4.5h3v3h-3z', dims: 'any', ready: true },
  { id: 'measure', key: 'M', label: 'Measure', d: 'M2 13 13 2l5 5L7 18zM5 10l2 2M8 7l2 2M11 4l2 2', dims: 'any', ready: true },
  { id: 'dropper', key: 'D', label: 'Dropper: pick a paint', d: 'M3 17l1-3 8-8 2 2-8 8zM12 6l2-2a2 2 0 0 1 3 3l-2 2', dims: 'any', ready: true },
  {
    id: 'hand',
    key: 'H',
    label: 'Pan',
    d: 'M7 10V5.5a1.3 1.3 0 0 1 2.6 0V9m0-4.5V4a1.3 1.3 0 0 1 2.6 0v5m0-3.5a1.3 1.3 0 0 1 2.6 0V11m0-3a1.3 1.3 0 0 1 2.6 0v4a6 6 0 0 1-6 6h-.5a6 6 0 0 1-4.6-2.2L3.5 12a1.3 1.3 0 0 1 2-1.7L7 12',
    dims: 'any',
    ready: true,
  },
  null,
  { id: 'box', key: 'B', label: 'Box', d: 'M10 2 17 6v8l-7 4-7-4V6zM3 6l7 4 7-4M10 10v8', dims: '3d', ready: true },
  { id: 'tile', key: 'T', label: 'Tile', d: 'M10 5 18 10 10 15 2 10z', dims: '3d', ready: true },
  { id: 'cylinder', key: 'C', label: 'Cylinder', d: 'M4 6a6 2.5 0 1 0 12 0a6 2.5 0 1 0-12 0M4 6v8a6 2.5 0 0 0 12 0V6', dims: '3d', ready: true },
  { id: 'facerect', key: 'F', label: 'Face rect', d: 'M3 6l7 4v8l-7-4zM5 9.5l3 1.7v3.5l-3-1.7z', dims: '3d', ready: true },
  { id: 'shadow', key: 'Shift+S', label: 'Shadow', d: 'M2 13a8 3 0 1 0 16 0a8 3 0 1 0-16 0', dims: '3d', ready: true },
  { id: 'seg', key: 'Shift+L', label: 'Segment', d: 'M3 13 17 6M2 11.5h3v3H2zM15.5 4.5h3v3h-3z', dims: '3d', ready: true },
  { id: 'poly3d', key: 'Shift+G', label: '3-D polygon', d: 'M3 11 9 6l8 3-3 7-9 0z', dims: '3d', ready: true },
  null,
  { id: 'line', key: 'L', label: 'Line', d: 'M4 16 16 4', dims: 'any', ready: true },
  { id: 'rect', key: 'R', label: 'Rectangle', d: 'M3 5h14v10H3z', dims: 'any', ready: true },
  { id: 'ellipse', key: 'E', label: 'Ellipse', d: 'M3 10a7 5 0 1 0 14 0a7 5 0 1 0-14 0', dims: 'any', ready: true },
  { id: 'arc', key: 'Shift+A', label: 'Arc', d: 'M4 15A7 7 0 0 1 15 6', dims: 'any', ready: true },
  { id: 'poly', key: 'G', label: 'Polygon', d: 'M10 3 17 8 14 17H6L3 8z', dims: 'any', ready: true },
  { id: 'pen', key: 'P', label: 'Pen', d: 'M4 16 12 4l4 4L6 18zM12 4l2-2 4 4-2 2', dims: 'any', ready: true },
  { id: 'text', key: 'X', label: 'Text', d: 'M4 4h12M10 4v13M7 17h6', dims: 'any', ready: true },
  null,
  { id: 'insert', key: 'I', label: 'Insert a fragment', d: 'M3 6h9v11H3zM8 3h9v11M7.5 9v5M5 11.5h5', dims: 'any', ready: true },
  { id: 'badge', key: 'Shift+B', label: 'Badge an icon', d: 'M3 7h11v10H3zM15 2a3 3 0 1 0 0.01 0', dims: 'any', ready: true },
];

/** Keys bound to something other than a tool, with or without Shift. */
export const BOUND = ['0', 'Shift+0', 'Escape', 'Enter', 'Delete', 'Backspace', 'Space', 'PageUp', 'PageDown', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'];

/** The key a press names, as the table writes keys: "B", "Shift+S". */
export const keyName = (key: string, shift: boolean): string => {
  const k = key.length === 1 ? key.toUpperCase() : key;
  return shift ? `Shift+${k}` : k;
};

/** The tool a key chooses, if any. */
export function toolFor(key: string, shift: boolean): Tool | null {
  const name = keyName(key, shift);
  return TOOLS.find((t) => t?.key === name) ?? null;
}

/** A tool can draw in an icon that is flat or not. */
export const fits = (t: Tool, flat: boolean): boolean => t.dims === 'any' || (t.dims === 'flat') === flat;
