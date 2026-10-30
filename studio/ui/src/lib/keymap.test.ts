// The keymap with a person's keys laid over it (#119): no key bound twice,
// before or after any rebinding; a key taken or kept refused, saying why;
// and rebindings stored that would clash, dropped.

import { describe, expect, it } from 'vitest';
import { COMMANDS, TOOLS, chord, clashes, commandFor, kept, keysOf, readBindings, rebind, reset, type Bindings } from './keymap';

/** The rebindings `r` made, or the test fails with why not. */
const ok = (r: { over: Bindings } | { refused: string }): Bindings => {
  if ('refused' in r) throw new Error(r.refused);
  return r.over;
};

describe('the commands', () => {
  it('list every tool, and the studio’s own', () => {
    expect(COMMANDS.filter((c) => c.group === 'Tools')).toHaveLength(TOOLS.filter(Boolean).length);
    expect(keysOf('save')).toEqual(['Ctrl+S']);
    expect(keysOf('preferences')).toEqual(['Ctrl+,']);
    expect(keysOf('redo')).toEqual(['Ctrl+Shift+Z', 'Ctrl+Y']);
  });

  it('bind no key twice, and none kept', () => {
    expect(clashes()).toEqual([]);
    expect(COMMANDS.flatMap((c) => c.keys).filter(kept)).toEqual([]);
  });
});

describe('a key pressed', () => {
  it('is named as the table names keys', () => {
    expect(chord({ key: 'b' })).toBe('B');
    expect(chord({ key: 'S', shiftKey: true })).toBe('Shift+S');
    expect(chord({ key: 'e', ctrlKey: true, shiftKey: true })).toBe('Ctrl+Shift+E');
    // Cmd on a Mac is Ctrl
    expect(chord({ key: 's', metaKey: true })).toBe('Ctrl+S');
    expect(chord({ key: ',', ctrlKey: true })).toBe('Ctrl+,');
    expect(chord({ key: 'F5', altKey: true })).toBe('Alt+F5');
    expect(chord({ key: ' ' })).toBe('Space');
    // a digit by where it is, whatever Shift makes of it
    expect(chord({ key: ')', code: 'Digit0', shiftKey: true })).toBe('Shift+0');
  });

  it('is nothing while only a modifier is down', () => {
    expect(chord({ key: 'Control', ctrlKey: true })).toBeNull();
    expect(chord({ key: 'Shift', shiftKey: true })).toBeNull();
  });
});

describe('rebinding', () => {
  it('moves a command to a free key, which no other runs', () => {
    const over = ok(rebind({}, 'tool:box', 'K'));
    expect(keysOf('tool:box', over)).toEqual(['K']);
    expect(commandFor('K', over)?.id).toBe('tool:box');
    expect(commandFor('B', over)).toBeNull();
    expect(clashes(over)).toEqual([]);
  });

  it('refuses a key another command has, naming it', () => {
    expect(rebind({}, 'tool:box', 'Shift+S')).toEqual({ refused: 'Shift+S is already Shadow: change that first' });
    expect(rebind({}, 'save', 'Ctrl+Z')).toEqual({ refused: 'Ctrl+Z is already Undo: change that first' });
    // its own key again is no clash
    expect(ok(rebind({}, 'tool:box', 'B'))).toEqual({});
  });

  it('refuses a key kept for what it does everywhere', () => {
    expect(rebind({}, 'tool:box', 'Escape')).toEqual({ refused: 'Escape is kept for what it does everywhere' });
    expect(rebind({}, 'save', 'Shift+ArrowUp')).toEqual({ refused: 'Shift+ArrowUp is kept for what it does everywhere' });
    expect(rebind({}, 'save', 'Ctrl+C')).toMatchObject({ refused: expect.stringContaining('kept') });
  });

  it('frees a key to give another, never binding one twice on the way', () => {
    let over = ok(rebind({}, 'tool:box', null));
    expect(keysOf('tool:box', over)).toEqual([]);
    over = ok(rebind(over, 'tool:shadow', 'B'));
    expect(commandFor('B', over)?.id).toBe('tool:shadow');
    over = ok(rebind(over, 'tool:box', 'Shift+S'));
    expect(clashes(over)).toEqual([]);
    // swapped back a step at a time: neither has its own key back while
    // the other holds it
    expect(reset(over, 'tool:box')).toEqual({ refused: 'B is Shadow now: change that first' });
    expect(reset(over, 'tool:shadow')).toEqual({ refused: 'Shift+S is Box now: change that first' });
    over = ok(rebind(over, 'tool:box', null));
    over = ok(reset(over, 'tool:shadow'));
    over = ok(reset(over, 'tool:box'));
    expect(over).toEqual({});
  });

  it('never binds a key twice, whatever is rebound to what', () => {
    // every command to every key a command has, in turn: each is refused
    // or leaves no clash
    const keys = [...new Set(COMMANDS.flatMap((c) => c.keys)), 'K', 'Ctrl+K', 'Escape'];
    let over: Bindings = {};
    for (const c of COMMANDS) {
      for (const k of keys) {
        const r = rebind(over, c.id, k);
        if ('over' in r) over = r.over;
        expect(clashes(over)).toEqual([]);
      }
    }
  });
});

describe('the keys stored', () => {
  it('are read back as kept', () => {
    expect(readBindings(JSON.stringify({ 'tool:box': ['K'], save: [] }))).toEqual({ 'tool:box': ['K'], save: [] });
  });

  it('leave out what cannot be read, and what would bind a key twice', () => {
    expect(readBindings(null)).toEqual({});
    expect(readBindings('not json')).toEqual({});
    expect(readBindings(JSON.stringify({ gone: ['K'], save: [3], 'tool:box': ['Escape'] }))).toEqual({});
    // Box rebound to M, which Measure has: dropped, so Box is B again
    const over = readBindings(JSON.stringify({ 'tool:box': ['M'], 'tool:tile': ['K'] }));
    expect(over).toEqual({ 'tool:tile': ['K'] });
    expect(clashes(over)).toEqual([]);
  });
});
