import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { fake } from './lib/fake';
import type { Backend } from './lib/backend';
import sampleIr from '../../../tests/fixtures/sample.ir.golden.json';

const names = (sampleIr as { icons: { name: string }[] }).icons.map((i) => i.name);

afterEach(() => {
  cleanup();
  localStorage.clear();
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.canvas;
});

async function opened(backend: Backend = fake()) {
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: `Canvas: ${names[0]}` });
  return backend;
}

describe('App', () => {
  it('welcomes with no library open', async () => {
    const backend = fake();
    const pick = vi.spyOn(backend, 'pickFolder');
    render(App, { backend });
    expect(screen.queryByRole('button', { name: 'Open the sample library' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Open library folder…' }));
    expect(pick).toHaveBeenCalled();
    // the fake picks the sample
    await screen.findByRole('navigation', { name: 'Tools' });
  });

  it('opens the first icon, dark around a light canvas', async () => {
    await opened();
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.dataset.canvas).toBe('light');
    expect(screen.getByRole('contentinfo').textContent).toContain('Compiled revision 0');
    expect(screen.getByRole('contentinfo').textContent).toContain('saved');
  });

  it('switches and remembers the themes', async () => {
    await opened();
    await fireEvent.click(screen.getByRole('button', { name: 'Light' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Dark canvas' }));
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(document.documentElement.dataset.canvas).toBe('dark');
    expect(localStorage.getItem('studio.theme')).toBe('light');
    expect(screen.getByRole('button', { name: 'Dark canvas' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('offers only the tools that are built', async () => {
    await opened();
    const rail = screen.getByRole('navigation', { name: 'Tools' });
    expect(within(rail).getByRole('button', { name: 'Select and move' }).hasAttribute('disabled')).toBe(false);
    expect(within(rail).getByRole('button', { name: 'Box' }).hasAttribute('disabled')).toBe(true);
    await fireEvent.keyDown(window, { key: 'h' });
    expect(within(rail).getByRole('button', { name: 'Pan' }).getAttribute('aria-pressed')).toBe('true');
    await fireEvent.keyDown(window, { key: 'b' });
    expect(within(rail).getByRole('button', { name: 'Pan' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('opens an icon from the Tree tab', async () => {
    await opened();
    await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
    const list = screen.getByRole('list', { name: 'Icons' });
    expect(within(list).getAllByRole('button')).toHaveLength(names.length);
    const last = names[names.length - 1]!;
    await fireEvent.click(within(list).getByRole('button', { name: new RegExp(last) }));
    await screen.findByRole('application', { name: `Canvas: ${last}` });
  });

  it('types in the Source tab, as one undo step a word', async () => {
    const backend = fake();
    const ask = vi.spyOn(backend, 'ask');
    const begin = vi.spyOn(backend, 'beginGesture');
    const end = vi.spyOn(backend, 'endGesture');
    await opened(backend);
    const editor = (await screen.findByRole('textbox', { name: /^Source of sample\.ilx/ })) as HTMLTextAreaElement;
    await waitFor(() => expect(editor.value).not.toBe(''));
    const original = editor.value;
    await fireEvent.input(editor, { target: { value: original + '//' } });
    await fireEvent.input(editor, { target: { value: original + '// ' } });
    await waitFor(() => expect(ask).toHaveBeenCalledTimes(2));
    expect(ask.mock.calls[0]![0]).toMatchObject({ kind: 'type', file: 'sample.ilx', len: 0, text: '//' });
    expect(ask.mock.calls[1]![0]).toMatchObject({ kind: 'type', text: ' ' });
    expect(begin).toHaveBeenCalledTimes(1);
    expect(end).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getByRole('contentinfo').textContent).toContain('unsaved changes'));

    await fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    await waitFor(() => expect(screen.getByRole('contentinfo').textContent).not.toContain('unsaved'));
  });

  it('undoes and redoes from the keyboard', async () => {
    const backend = fake();
    await opened(backend);
    await backend.ask({ kind: 'type', file: 'sample.ilx', at: 0, len: 0, text: '// a\n' }, 'Type');
    const undo = screen.getByRole('button', { name: 'Undo' });
    await waitFor(() => expect(undo.hasAttribute('disabled')).toBe(false));
    await fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    await waitFor(() => expect(undo.hasAttribute('disabled')).toBe(true));
    await screen.findByText(/^Undid /);
    await fireEvent.keyDown(window, { key: 'Z', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(undo.hasAttribute('disabled')).toBe(false));
  });

  it('offers to reload or save over a library changed on disk', async () => {
    const backend = fake();
    let tell: ((n: Parameters<Parameters<Backend['listen']>[0]>[0]) => void) | undefined;
    const listen = backend.listen;
    backend.listen = async (hear) => {
      tell = hear;
      return listen(hear);
    };
    const reload = vi.spyOn(backend, 'reload');
    await opened(backend);
    await backend.ask({ kind: 'type', file: 'sample.ilx', at: 0, len: 0, text: '// a\n' }, 'Type');
    tell!({ kind: 'out-of-sync', files: ['sample.ilx'] });
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('sample.ilx');
    await fireEvent.click(within(alert).getByRole('button', { name: 'Reload from disk' }));
    expect(reload).toHaveBeenCalled();
    tell!({ kind: 'reloaded', files: ['sample.ilx'] });
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
  });
});
