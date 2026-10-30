// The Parts panel and selection (#68), in the window over the stand-in
// for the core: selecting in each of the three places, several with Shift
// (#114), and what the panel asks the core for.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import type { Backend, Origin, Outline } from './lib/backend';
import source from '../../../tests/fixtures/sample.ilx?raw';
import sampleStudio from '../../../tests/fixtures/sample.studio.golden.json';

const studio = sampleStudio as unknown as { outlines: Record<string, Outline>; sources: Record<string, Origin[]> };
const lineOfText = (needle: string) => source.slice(0, source.indexOf(needle)).split('\n').length;

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** The window, editing the icon `name`. */
async function editing(name: string, given?: Backend) {
  const backend = given ?? (await core());
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: /^Canvas: / });
  await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
  await fireEvent.click(within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: new RegExp(`^${name}$`) }));
  await screen.findByRole('application', { name: `Canvas: ${name}` });
  await fireEvent.click(screen.getByRole('tab', { name: 'Source' }));
  const parts = screen.getByRole('complementary', { name: 'Parts' });
  await waitFor(() => expect(within(parts).queryByText('Reading the definition…')).toBeNull());
  return parts;
}

const canvas = () => screen.getByRole('application');
const shapes = () => [...canvas().querySelectorAll('[data-shape]')];
/** The `nth` shape on the canvas written as `needle…`: the drawing's
    shapes are tagged in the order its sources list them. */
function shapeAt(needle: string, nth = 0): Element {
  const name = canvas().getAttribute('aria-label')!.replace('Canvas: ', '');
  const found = studio.sources[name]!.map((o, k) => [o, k] as const).filter(([o]) => source.slice(o.node.start).startsWith(needle));
  return canvas().querySelector(`[data-shape="${found[nth]![1]}"]`)!;
}
async function click(el: Element, shiftKey = false) {
  await fireEvent.pointerDown(el, { button: 0, clientX: 5, clientY: 5, shiftKey });
  await fireEvent.pointerUp(el, { button: 0, clientX: 5, clientY: 5, shiftKey });
}
const highlighted = () => document.querySelector('.ln.hl .num')?.textContent;
const current = (parts: HTMLElement, name: RegExp) => within(parts).getByRole('button', { name }).getAttribute('aria-current');

describe('Parts and selection', () => {
  it('lists the definition’s parts, and those it inherits', async () => {
    const parts = await editing('switch-l3');
    expect(within(parts).getByRole('button', { name: /^routing-badge/ })).toBeTruthy();
    expect(within(parts).getByText('drawing here')).toBeTruthy();
    expect(within(parts).getByRole('button', { name: /^at\s*at 1 0\.5 0\.28/ })).toBeTruthy();
    expect(within(parts).getByRole('button', { name: /^chassis\s*from rack-unit/ })).toBeTruthy();
    expect(within(parts).getByRole('button', { name: /^ports\s*from switch/ })).toBeTruthy();
  });

  it('selects a row: its shapes marked, its line highlighted', async () => {
    const parts = await editing('switch-l3');
    await fireEvent.click(within(parts).getByRole('button', { name: /^path/ }));
    expect(current(parts, /^path/)).toBe('true');
    expect(canvas().querySelectorAll('.marked')).toHaveLength(1);
    expect(highlighted()).toBe(String(lineOfText('path d="M{x-10}')));
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(canvas().querySelectorAll('.marked')).toHaveLength(0);
  });

  it('selects by a click on the canvas, and says where an inherited shape is from', async () => {
    const parts = await editing('switch-l3');
    await click(shapeAt('path'));
    expect(current(parts, /^path/)).toBe('true');
    await click(shapeAt('box'));
    expect(screen.getByText(/^From rack-unit, part chassis/)).toBeTruthy();
    expect(current(parts, /^path/)).toBe('true');
    // a click on nothing clears it
    await click(canvas());
    expect(current(parts, /^path/)).toBe('false');
  });

  it('selects several with Shift, in the panel and on the canvas, and Esc lets them all go', async () => {
    const parts = await editing('switch-poe');
    const row = (name: RegExp) => within(parts).getByRole('button', { name });
    const marked = () => canvas().querySelectorAll('.marked').length;
    await fireEvent.click(row(/^tile/));
    const one = marked();
    await fireEvent.click(row(/^cylinder/), { shiftKey: true });
    expect([current(parts, /^tile/), current(parts, /^cylinder/)]).toEqual(['true', 'true']);
    expect(marked()).toBeGreaterThan(one);
    // again, and it is taken out; Ctrl adds as Shift does
    await fireEvent.click(row(/^cylinder/), { shiftKey: true });
    expect([current(parts, /^tile/), current(parts, /^cylinder/)]).toEqual(['true', 'false']);
    expect(marked()).toBe(one);
    await fireEvent.click(row(/^shadow/), { ctrlKey: true });
    expect(current(parts, /^shadow/)).toBe('true');
    // a plain click selects only what it is on
    await fireEvent.click(row(/^cylinder/));
    expect([current(parts, /^tile/), current(parts, /^shadow/), current(parts, /^cylinder/)]).toEqual(['false', 'false', 'true']);

    // on the canvas
    await click(shapeAt('tile'));
    await click(shapeAt('shadow'), true);
    expect([current(parts, /^tile/), current(parts, /^shadow/), current(parts, /^cylinder/)]).toEqual(['true', 'true', 'false']);
    // Shift+click on nothing keeps them
    await click(canvas(), true);
    expect(current(parts, /^tile/)).toBe('true');
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect([current(parts, /^tile/), current(parts, /^shadow/)]).toEqual(['false', 'false']);
    expect(marked()).toBe(0);
  });

  it('selects by the caret in the source', async () => {
    const parts = await editing('switch-l3');
    const editor = (await screen.findByRole('textbox', { name: /^Source of sample\.ilx/ })) as HTMLTextAreaElement;
    await waitFor(() => expect(editor.value).toBe(source));
    const at = source.indexOf('path d="M{x-10}') + 3;
    editor.setSelectionRange(at, at);
    await fireEvent.mouseUp(editor);
    expect(current(parts, /^path/)).toBe('true');
  });

  it('opens the definition an inherited part comes from, drawn as before', async () => {
    const parts = await editing('switch-l3');
    await fireEvent.click(within(parts).getByRole('button', { name: /^chassis\s*from rack-unit/ }));
    await waitFor(() => expect(within(parts).getByRole('button', { name: /^chassis$/ })).toBeTruthy());
    expect(screen.getByRole('banner').textContent).toMatch(/base\s*rack-unit\s*as drawn in\s*switch-l3/);
    // now the box is its own, and the path is from elsewhere
    await click(shapeAt('box'));
    expect(current(parts, /^box/)).toBe('true');
    expect(highlighted()).toBe(String(lineOfText('box 0 0 0 2 1')));
    await click(shapeAt('path'));
    expect(screen.getByText(/^From switch-l3, part routing-badge/)).toBeTruthy();
  });

  it('hides and locks a part in the editor only', async () => {
    const backend = await core();
    const ask = vi.spyOn(backend, 'ask');
    const parts = await editing('switch-l3', backend);
    const before = shapes().length;
    await fireEvent.click(within(parts).getByRole('button', { name: 'Hide part routing-badge' }));
    expect(shapes().length).toBe(before - 1);
    await fireEvent.click(within(parts).getByRole('button', { name: 'Hide part routing-badge' }));
    await fireEvent.click(within(parts).getByRole('button', { name: 'Lock part routing-badge' }));
    await click(shapeAt('path'));
    expect(screen.getByText('Part routing-badge is locked')).toBeTruthy();
    expect(current(parts, /^path/)).toBe('false');
    expect(ask).not.toHaveBeenCalled();
  });

  it('reorders by key, renames and adds parts, each one undo step', async () => {
    const backend = await core();
    const ask = vi.spyOn(backend, 'ask');
    const parts = await editing('switch-poe', backend);
    const text = async () => (await backend.source('sample.ilx'))!.text;
    const outline = studio.outlines['switch-poe']!;
    const rows = outline.parts[2]!.shapes;
    const listed = () => within(parts).getAllByRole('button').map((b) => b.querySelector('.kind')?.textContent);

    await fireEvent.keyDown(within(parts).getByRole('button', { name: /^seg/ }), { key: 'PageUp' });
    expect(ask).toHaveBeenLastCalledWith({ kind: 'move-before', node: rows[4]!.node, before: rows[6]!.node }, 'Move seg in switch-poe', 'switch-poe');
    // drawn after the at now: listed above it
    await waitFor(async () => expect((await text()).indexOf('seg 0 0 0 2')).toBeGreaterThan((await text()).indexOf('at 0.5 0.5 0.6')));
    await waitFor(() => expect(listed().indexOf('seg')).toBeLessThan(listed().indexOf('at')));
    // the bottom shape cannot go lower
    await fireEvent.keyDown(within(parts).getByRole('button', { name: /^tile/ }), { key: 'End' });
    expect(ask).toHaveBeenCalledTimes(1);

    await fireEvent.dblClick(within(parts).getByRole('button', { name: /^everything-else/ }));
    const input = within(parts).getByRole('textbox', { name: 'New name for part everything-else' });
    await fireEvent.input(input, { target: { value: 'rest' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    expect(ask).toHaveBeenLastCalledWith(
      { kind: 'set-arg', node: outline.parts[2]!.node, index: 0, value: 'rest' },
      'Rename part everything-else to rest',
      'switch-poe',
    );
    await within(parts).findByRole('button', { name: /^rest/ });

    await fireEvent.click(within(parts).getByRole('button', { name: 'Add a part' }));
    expect(ask.mock.lastCall![0]).toMatchObject({ kind: 'insert', text: 'part "part-1" {\n}', before: null });
    await within(parts).findByRole('button', { name: /^part-1/ });
    expect(within(parts).getByText('drawing here').closest('.head')!.textContent).toContain('part-1');
    expect(await text()).toContain('  part "part-1" {\n  }\n}');

    // three steps back is the sample again
    for (let k = 0; k < 3; k++) await fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    await waitFor(async () => expect(await text()).toBe(source));
  });
});
