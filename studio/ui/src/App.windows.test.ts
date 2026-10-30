// Customizable shortcuts, floating panels and more windows (#119), in the
// window over the studio's core and the sample library: a key rebound in
// Preferences runs its command and no other, a key taken refused, both
// kept; a panel floated, moved and docked, its place kept; and two
// windows over one core seeing one edit and one undo history.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import type { Backend } from './lib/backend';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** A window over `backend`, the sample opened in it unless another
    window has it open. */
async function windowOn(backend: Backend, open = true) {
  const { container } = render(App, { backend, sample: true });
  const w = within(container as HTMLElement);
  if (open) await fireEvent.click(w.getByRole('button', { name: 'Open the sample library' }));
  await w.findByRole('application', { name: /^Canvas: / });
  return w;
}

const key = (k: string, more: Partial<KeyboardEventInit> = {}) => fireEvent.keyDown(window, { key: k, ...more });
const tool = (w: ReturnType<typeof within>, label: string) => w.getByRole('button', { name: label, exact: true });

describe('keyboard shortcuts', () => {
  async function preferences() {
    const w = await windowOn(await core());
    await key(',', { ctrlKey: true });
    const dialog = screen.getByRole('dialog', { name: 'Preferences' });
    return { w, dialog: within(dialog) };
  }

  it('are listed in Preferences, each command with its keys', async () => {
    const { dialog } = await preferences();
    expect(dialog.getByRole('table', { name: 'File' })).toBeTruthy();
    expect(dialog.getByRole('button', { name: 'Shortcut for Save: Ctrl+S' })).toBeTruthy();
    expect(dialog.getByRole('button', { name: 'Shortcut for Redo: Ctrl+Shift+Z or Ctrl+Y' })).toBeTruthy();
    expect(dialog.getByRole('button', { name: 'Shortcut for Box: B' })).toBeTruthy();
    // filtered by a name or a key
    await fireEvent.input(dialog.getByRole('searchbox', { name: 'Find a command or a key' }), { target: { value: 'shift+s' } });
    expect(dialog.getAllByRole('button', { name: /^Shortcut for / }).map((b) => b.getAttribute('aria-label'))).toEqual(['Shortcut for Shadow: Shift+S']);
  });

  it('rebinds a command: its new key runs it, its old key nothing', async () => {
    const { w, dialog } = await preferences();
    const box = dialog.getByRole('button', { name: 'Shortcut for Box: B' });
    await fireEvent.click(box);
    expect(box.textContent).toBe('Press a key…');
    // a modifier alone waits for the key
    await fireEvent.keyDown(box, { key: 'Shift', shiftKey: true });
    expect(box.textContent).toBe('Press a key…');
    await fireEvent.keyDown(box, { key: 'k' });
    expect(dialog.getByRole('status').textContent).toBe('Box is K now');
    expect(dialog.getByRole('button', { name: 'Shortcut for Box: K' })).toBeTruthy();
    // keys bound in Preferences do nothing else while it is open
    expect(tool(w, 'Box').getAttribute('aria-pressed')).toBe('false');
    await fireEvent.click(dialog.getByRole('button', { name: 'Close' }));

    expect(tool(w, 'Box').getAttribute('title')).toBe('Box (K)');
    await key('b');
    expect(tool(w, 'Box').getAttribute('aria-pressed')).toBe('false');
    await key('k');
    expect(tool(w, 'Box').getAttribute('aria-pressed')).toBe('true');
  });

  it('refuses a key another command has, saying which', async () => {
    const { dialog } = await preferences();
    const box = dialog.getByRole('button', { name: 'Shortcut for Box: B' });
    await fireEvent.click(box);
    await fireEvent.keyDown(box, { key: 'S', shiftKey: true });
    expect(dialog.getByRole('alert').textContent).toBe('Shift+S is already Shadow: change that first');
    // still waiting for a key it can have
    expect(box.getAttribute('aria-label')).toBe('Shortcut for Box: Press a key…');
    // and one kept for what it does everywhere
    await fireEvent.keyDown(box, { key: 'Delete' });
    expect(dialog.getByRole('alert').textContent).toBe('Delete is kept for what it does everywhere');
    // Escape gives up, Preferences staying open
    await fireEvent.keyDown(box, { key: 'Escape' });
    expect(box.getAttribute('aria-label')).toBe('Shortcut for Box: B');
    expect(screen.getByRole('dialog', { name: 'Preferences' })).toBeTruthy();
  });

  it('keeps a rebinding for next time, until it is reset', async () => {
    const { dialog } = await preferences();
    const save = dialog.getByRole('button', { name: 'Shortcut for Save: Ctrl+S' });
    await fireEvent.click(save);
    await fireEvent.keyDown(save, { key: 'w', ctrlKey: true, altKey: true });
    expect(JSON.parse(localStorage.getItem('shortcuts')!)).toEqual({ save: ['Ctrl+Alt+W'] });

    cleanup();
    const again = await preferences();
    expect(again.dialog.getByRole('button', { name: 'Shortcut for Save: Ctrl+Alt+W' })).toBeTruthy();
    await fireEvent.click(again.dialog.getByRole('button', { name: 'Reset Save' }));
    expect(again.dialog.getByRole('button', { name: 'Shortcut for Save: Ctrl+S' })).toBeTruthy();
    expect(again.dialog.queryByRole('button', { name: 'Reset Save' })).toBeNull();
    expect(JSON.parse(localStorage.getItem('shortcuts')!)).toEqual({});
  });

  it('leaves a command without a key with Backspace', async () => {
    const { w, dialog } = await preferences();
    const tile = dialog.getByRole('button', { name: 'Shortcut for Tile: T' });
    await fireEvent.click(tile);
    await fireEvent.keyDown(tile, { key: 'Backspace' });
    expect(dialog.getByRole('button', { name: 'Shortcut for Tile: none' })).toBeTruthy();
    await fireEvent.click(dialog.getByRole('button', { name: 'Close' }));
    await key('t');
    expect(tool(w, 'Tile').getAttribute('aria-pressed')).toBe('false');
    expect(tool(w, 'Tile').getAttribute('title')).toBe('Tile');
  });
});

describe('floating panels', () => {
  const tabs = (w: ReturnType<typeof within>) => w.getAllByRole('tab').map((t: HTMLElement) => t.textContent);

  it('float out of the tabs and dock back, kept for next time', async () => {
    const w = await windowOn(await core());
    await fireEvent.click(w.getByRole('tab', { name: 'Tree' }));
    await fireEvent.click(w.getByRole('button', { name: 'Float the Tree panel' }));
    expect(tabs(w)).toEqual(['Design', 'Arrange', 'Source', 'Preview']);
    const tree = w.getByRole('dialog', { name: 'Tree' });
    // the panel works floating as docked
    expect(within(tree).getByRole('list', { name: 'Icons' })).toBeTruthy();
    expect(JSON.parse(localStorage.getItem('layout')!).floating.tree).toMatchObject({ w: 360, h: 460 });

    cleanup();
    const again = await windowOn(await core());
    expect(again.getByRole('dialog', { name: 'Tree' })).toBeTruthy();
    expect(tabs(again)).not.toContain('Tree');
    await fireEvent.click(again.getByRole('button', { name: 'Dock the Tree panel' }));
    expect(again.queryByRole('dialog', { name: 'Tree' })).toBeNull();
    expect(tabs(again)).toEqual(['Design', 'Arrange', 'Source', 'Tree', 'Preview']);
    expect(again.getByRole('tab', { name: 'Tree' }).getAttribute('aria-selected')).toBe('true');
  });

  it('float by a tab dragged out, and move by the title bar', async () => {
    const w = await windowOn(await core());
    const design = w.getByRole('tab', { name: 'Design' });
    await fireEvent.pointerDown(design, { button: 0, clientX: 800, clientY: 60 });
    // a little way is still a click
    await fireEvent.pointerMove(window, { clientX: 790, clientY: 70 });
    expect(w.queryByRole('dialog', { name: 'Design' })).toBeNull();
    await fireEvent.pointerMove(window, { clientX: 500, clientY: 300 });
    const panel = w.getByRole('dialog', { name: 'Design' });
    expect(panel.style.left).toBe('440px');
    expect(panel.style.top).toBe('286px');
    // still held: it follows the pointer
    await fireEvent.pointerMove(window, { clientX: 520, clientY: 310 });
    await fireEvent.pointerUp(window, { clientX: 520, clientY: 310 });
    expect(panel.style.left).toBe('460px');
    expect(JSON.parse(localStorage.getItem('layout')!).floating.design).toMatchObject({ x: 460, y: 296 });

    // by its title bar
    const head = within(panel).getByRole('heading', { name: 'Design' }).parentElement!;
    await fireEvent.pointerDown(head, { button: 0, clientX: 470, clientY: 300 });
    await fireEvent.pointerMove(window, { clientX: 370, clientY: 200 });
    await fireEvent.pointerUp(window, { clientX: 370, clientY: 200 });
    expect([panel.style.left, panel.style.top]).toEqual(['360px', '196px']);
  });

  it('leave the canvas the room when every one floats', async () => {
    const w = await windowOn(await core());
    for (const name of ['Design', 'Arrange', 'Source', 'Tree', 'Preview']) {
      await fireEvent.click(w.getByRole('tab', { name }));
      await fireEvent.click(w.getByRole('button', { name: `Float the ${name} panel` }));
    }
    expect(w.queryByRole('complementary', { name: 'Inspector' })).toBeNull();
    expect(w.getAllByRole('dialog')).toHaveLength(5);
    // Align and Distribute's key shows it where it is
    await key('a', { ctrlKey: true, shiftKey: true });
    expect(w.getByRole('dialog', { name: 'Arrange' })).toBeTruthy();
  });
});

describe('more windows', () => {
  it('are offered in the app alone', async () => {
    const backend = await core();
    const w = await windowOn(backend);
    const opened = vi.spyOn(backend, 'newWindow');
    expect((w.getByRole('button', { name: 'New window' }) as HTMLButtonElement).disabled).toBe(true);
    await key('n', { ctrlKey: true, shiftKey: true });
    expect(opened).not.toHaveBeenCalled();
  });

  it('open on the library open, at the icon asked for', async () => {
    const backend = await core();
    await windowOn(backend);
    vi.spyOn(backend, 'openedOn').mockResolvedValue('switch-l2');
    const second = await windowOn(backend, false);
    await second.findByRole('application', { name: 'Canvas: switch-l2' });
    // the library as the first has it, not the welcome
    expect(second.queryByRole('button', { name: 'Open the sample library' })).toBeNull();
    expect(second.getByText('saved')).toBeTruthy();
  });

  it('see one edit, and share one undo history', async () => {
    const backend = await core();
    const first = await windowOn(backend);
    const editing = first.getByRole('application').getAttribute('aria-label')!.replace('Canvas: ', '');
    vi.spyOn(backend, 'openedOn').mockResolvedValue(editing);
    const second = await windowOn(backend, false);
    await second.findByRole('application', { name: `Canvas: ${editing}` });
    const ask = vi.spyOn(backend, 'ask');

    // an edit in the first, in the definition it edits
    const parts = first.getByRole('complementary', { name: 'Parts' });
    await waitFor(() => expect(within(parts).queryByText('Reading the definition…')).toBeNull());
    await fireEvent.click(within(parts).getByRole('button', { name: 'Add a part' }));
    expect(ask.mock.lastCall![2]).toBe(editing);
    // the second sees it: unsaved, its part listed, and undoable there
    await waitFor(() => expect(second.getByText('● unsaved changes')).toBeTruthy());
    const theirs = second.getByRole('complementary', { name: 'Parts' });
    await within(theirs).findByRole('button', { name: /^part-1/ });
    const undo = second.getByRole('button', { name: 'Undo' }) as HTMLButtonElement;
    expect(undo.disabled).toBe(false);

    // undone in the second: gone from both
    await fireEvent.click(undo);
    await waitFor(() => expect(first.getByText('saved')).toBeTruthy());
    await waitFor(() => expect(within(parts).queryByRole('button', { name: /^part-1/ })).toBeNull());
    expect((first.getByRole('button', { name: 'Redo' }) as HTMLButtonElement).disabled).toBe(false);
  });
});
