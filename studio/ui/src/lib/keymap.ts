// The commands and their keys (docs/studio-interface.md, Tools and Keys,
// and Preferences): one table, which the rail and Preferences are drawn
// from and keys are looked up in, with the keys a person has rebound laid
// over it (#119). A key is never bound twice: a test fails on the
// defaults, and a rebinding that would is refused. Pure.

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

/** A command a key runs: one of the studio's, or a tool chosen. */
export interface Command {
  id: string;
  label: string;
  /** Where Preferences lists it. */
  group: 'File' | 'Edit' | 'View' | 'Tools';
  /** Its keys unless rebound, as the table writes keys: "Ctrl+Shift+E". */
  keys: string[];
  /** It runs from a text field too, as saving does; else the field's own
      keys come first. */
  anywhere?: boolean;
}

/** Every command, in Preferences' order: the studio's, then the tools,
    as the rail has them. Ctrl is Cmd on a Mac. */
export const COMMANDS: Command[] = [
  { id: 'open', label: 'Open a library folder', group: 'File', keys: ['Ctrl+O'], anywhere: true },
  { id: 'new', label: 'New library', group: 'File', keys: ['Ctrl+N'], anywhere: true },
  { id: 'window', label: 'New window', group: 'File', keys: ['Ctrl+Shift+N'], anywhere: true },
  { id: 'save', label: 'Save', group: 'File', keys: ['Ctrl+S'], anywhere: true },
  { id: 'export', label: 'Export', group: 'File', keys: ['Ctrl+Shift+E'], anywhere: true },
  { id: 'preferences', label: 'Preferences', group: 'File', keys: ['Ctrl+,'], anywhere: true },
  { id: 'undo', label: 'Undo', group: 'Edit', keys: ['Ctrl+Z'] },
  { id: 'redo', label: 'Redo', group: 'Edit', keys: ['Ctrl+Shift+Z', 'Ctrl+Y'] },
  { id: 'duplicate', label: 'Duplicate', group: 'Edit', keys: ['Ctrl+D'] },
  { id: 'find', label: 'Find', group: 'Edit', keys: ['Ctrl+F'], anywhere: true },
  { id: 'arrange', label: 'Align and Distribute', group: 'View', keys: ['Ctrl+Shift+A'], anywhere: true },
  { id: 'fit', label: 'Fit the icon', group: 'View', keys: ['0'] },
  ...TOOLS.flatMap((t): Command[] => (t ? [{ id: `tool:${t.id}`, label: t.label, group: 'Tools', keys: [t.key] }] : [])),
];

/** Keys kept for what they do everywhere, with or without Shift, which
    no command takes: cancelling, finishing, deleting, panning with Space,
    nudging and reordering, moving between fields, renaming, and a text
    field's clipboard. */
export const KEPT = ['Escape', 'Enter', 'Delete', 'Backspace', 'Space', 'Tab', 'PageUp', 'PageDown', 'Home', 'End', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'F2', 'Ctrl+C', 'Ctrl+V', 'Ctrl+X'];

/** `key` is kept, and no command may take it. */
export const kept = (key: string): boolean => KEPT.includes(key.replace('Shift+', ''));

const MODIFIERS = new Set(['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'CapsLock', 'Fn', 'OS']);

/** The key a press names, as the table writes keys: "B", "Shift+S",
    "Ctrl+Shift+E"; none for a modifier alone. A digit is named by where
    it is, as Shift turns it into another character. */
export function chord(e: { key: string; code?: string; ctrlKey?: boolean; metaKey?: boolean; altKey?: boolean; shiftKey?: boolean }): string | null {
  if (MODIFIERS.has(e.key)) return null;
  const digit = /^Digit(\d)$/.exec(e.code ?? '');
  const key = digit ? digit[1]! : e.key === ' ' ? 'Space' : e.key.length === 1 ? e.key.toUpperCase() : e.key;
  return [(e.ctrlKey || e.metaKey) && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', key].filter(Boolean).join('+');
}

/** The keys a person has rebound, by command; a command not in it has
    its own. */
export type Bindings = Record<string, string[]>;

/** The command `id`. */
export const command = (id: string): Command | undefined => COMMANDS.find((c) => c.id === id);

/** The keys of `id`, as rebound. */
export const keysOf = (id: string, over: Bindings = {}): string[] => over[id] ?? command(id)?.keys ?? [];

/** The command `key` runs, as rebound. */
export function commandFor(key: string, over: Bindings = {}): Command | null {
  return COMMANDS.find((c) => keysOf(c.id, over).includes(key)) ?? null;
}

/** The keys bound to more than one command, or kept: none, unless
    something is wrong. */
export function clashes(over: Bindings = {}): string[] {
  const all = COMMANDS.flatMap((c) => keysOf(c.id, over));
  return [...new Set(all.filter((k, i) => kept(k) || all.indexOf(k) !== i))];
}

/** The overrides, less any that is its command's own keys. */
const tidy = (over: Bindings): Bindings =>
  Object.fromEntries(
    Object.entries(over).filter(([id, keys]) => {
      const own = command(id)?.keys ?? [];
      return !(keys.length === own.length && keys.every((k, i) => k === own[i]));
    }),
  );

/** `id` bound to `key` alone, or to none; refused, saying why, if
    another command has the key or it is kept. */
export function rebind(over: Bindings, id: string, key: string | null): { over: Bindings } | { refused: string } {
  if (key !== null) {
    if (kept(key)) return { refused: `${key} is kept for what it does everywhere` };
    const holder = commandFor(key, over);
    if (holder && holder.id !== id) return { refused: `${key} is already ${holder.label}: change that first` };
  }
  return { over: tidy({ ...over, [id]: key === null ? [] : [key] }) };
}

/** `id` given its own keys again; refused if another command took one. */
export function reset(over: Bindings, id: string): { over: Bindings } | { refused: string } {
  const rest = { ...over };
  delete rest[id];
  const taken = keysOf(id)
    .map((k) => commandFor(k, over))
    .find((c) => c && c.id !== id);
  if (taken) return { refused: `${keysOf(id).join(', ')} is ${taken.label} now: change that first` };
  return { over: rest };
}

/** The rebindings as stored, if they can be read: those of commands that
    are no more, or of keys that are kept, left out, and any that would
    bind a key twice dropped, as when a later studio gives a command of
    its own a key someone had rebound another to. */
export function readBindings(raw: string | null): Bindings {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw ?? '{}');
  } catch {
    return {};
  }
  if (!parsed || typeof parsed !== 'object') return {};
  const over: Bindings = {};
  for (const [id, keys] of Object.entries(parsed as Record<string, unknown>)) {
    if (command(id) && Array.isArray(keys) && keys.every((k) => typeof k === 'string' && k && !kept(k))) over[id] = keys as string[];
  }
  for (;;) {
    const twice = clashes(over)[0];
    if (twice === undefined) return tidy(over);
    const id = Object.keys(over).find((i) => over[i]!.includes(twice))!;
    delete over[id];
  }
}

/** A tool can draw in an icon that is flat or not. */
export const fits = (t: Tool, flat: boolean): boolean => t.dims === 'any' || (t.dims === 'flat') === flat;
