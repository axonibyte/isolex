// Export (#112) in the window, over the studio's core: the open icon,
// chosen icons or every icon as SVG on either ground, the IR, and the
// library as a zip, each downloaded as the file it would be in the app.
// PNG is drawn by the browser, so the Playwright suite checks it.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import type { Backend } from './lib/backend';
import source from '../../../tests/fixtures/sample.ilx?raw';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

async function opened(files?: [string, string][]) {
  const saved: { name: string; data: string | Uint8Array }[] = [];
  const backend: Backend = await core(files, (name, data) => saved.push({ name, data }));
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  // a library that does not compile opens with no icon on the canvas
  await screen.findByRole('button', { name: 'Export…' });
  return { backend, saved };
}

async function exporting() {
  await fireEvent.keyDown(window, { key: 'E', ctrlKey: true, shiftKey: true });
  return screen.getByRole('dialog', { name: 'Export' });
}

describe('Export', () => {
  it('writes the open icon as SVG, as the core draws it', async () => {
    const { backend, saved } = await opened();
    const open = (await screen.findByRole('application', { name: /^Canvas: / })).getAttribute('aria-label')!.replace('Canvas: ', '');
    const dialog = await exporting();
    expect(within(dialog).getByRole('radio', { name: `The open icon, ${open}` })).toHaveProperty('checked', true);
    expect(within(dialog).getByText('One file, downloaded.')).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Export' }));
    await waitFor(() => expect(within(dialog).getByRole('status').textContent).toBe(`Exported ${open}.svg`));
    const [want] = await backend.exportSvgs([open], 'light');
    expect(saved).toEqual([{ name: `${open}.svg`, data: want.svg }]);
  });

  it('writes every icon on both grounds, and chosen icons only', async () => {
    const { backend, saved } = await opened();
    const every = await backend.exportSvgs(null, 'light');
    const dialog = await exporting();
    await fireEvent.click(within(dialog).getByRole('radio', { name: `Every icon (${every.length})` }));
    await fireEvent.click(within(dialog).getByRole('checkbox', { name: 'Dark' }));
    expect(within(dialog).getByText(`${every.length * 2} files, downloaded.`)).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Export' }));
    await waitFor(() => expect(saved).toHaveLength(every.length * 2));
    expect(saved.map((f) => f.name)).toEqual([...every.map((s) => `${s.name}.svg`), ...every.map((s) => `${s.name}@dark.svg`)]);
    expect(saved[every.length].data).not.toBe(saved[0].data);

    saved.length = 0;
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'Chosen icons' }));
    expect((within(dialog).getByRole('button', { name: 'Export' }) as HTMLButtonElement).disabled).toBe(true);
    const list = within(dialog).getByRole('list', { name: 'Icons' });
    await fireEvent.click(within(list).getByRole('checkbox', { name: 'switch-l3' }));
    await fireEvent.click(within(list).getByRole('checkbox', { name: 'switch-poe' }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Export' }));
    await waitFor(() => expect(saved).toHaveLength(4));
    expect(saved.map((f) => f.name).sort()).toEqual(['switch-l3.svg', 'switch-l3@dark.svg', 'switch-poe.svg', 'switch-poe@dark.svg']);
  });

  it('writes the IR as isolex compile does, and the library as a zip', async () => {
    const { backend, saved } = await opened();
    const dialog = await exporting();
    await fireEvent.change(within(dialog).getByRole('combobox', { name: 'As' }), { target: { value: 'ir' } });
    await fireEvent.click(within(dialog).getByRole('radio', { name: /^Every icon/ }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Export' }));
    await waitFor(() => expect(saved).toHaveLength(1));
    expect(saved[0]).toEqual({ name: 'library.json', data: await backend.exportIr(null) });
    expect(JSON.parse(saved[0].data as string).icons.length).toBeGreaterThan(5);

    await fireEvent.change(within(dialog).getByRole('combobox', { name: 'As' }), { target: { value: 'zip' } });
    expect(within(dialog).queryByRole('radiogroup', { name: 'Icons to export' })).toBeNull();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Export' }));
    await waitFor(() => expect(saved).toHaveLength(2));
    expect(saved[1].name).toBe('library.zip');
    const zip = saved[1].data as Uint8Array;
    // a zip: its first local header, and the sample's text deflated in it
    expect([...zip.slice(0, 4)]).toEqual([0x50, 0x4b, 0x03, 0x04]);
    expect(zip.length).toBeLessThan(source.length);
  });

  it('says why a library with errors is not exported', async () => {
    const { saved } = await opened([['a.ilx', 'icon "x" {\n']]);
    const dialog = await exporting();
    await fireEvent.change(within(dialog).getByRole('combobox', { name: 'As' }), { target: { value: 'ir' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Export' }));
    await waitFor(() => expect(within(dialog).getByRole('alert').textContent).toBe('the library has an error; the Problems list says where'));
    expect(saved).toEqual([]);
  });

  it('opens from the top bar too, and closes', async () => {
    await opened();
    await fireEvent.click(screen.getByRole('button', { name: 'Export…' }));
    const dialog = screen.getByRole('dialog', { name: 'Export' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('dialog', { name: 'Export' })).toBeNull();
  });
});
