import { mount } from 'svelte';
import '@fontsource/geist-sans/400.css';
import '@fontsource/geist-sans/600.css';
import '@fontsource/geist-mono/400.css';
import './app.css';
import App from './App.svelte';
import { inShell, shell } from './lib/backend';

// In the studio's window, the shell. In a plain browser, while working on
// the interface (`npm run dev`, which its browser tests use), the studio's
// core as a WebAssembly module over the sample library, or with
// `?library=text` the compiler's text fixture, which has fonts; a build is
// for the shell alone, and leaves the module out.
const target = document.getElementById('app')!;
if (inShell()) {
  mount(App, { target, props: { backend: await shell() } });
} else if (import.meta.env.DEV) {
  const { core } = await import('./lib/core');
  const text = new URLSearchParams(location.search).get('library') === 'text';
  const files: [string, string][] | undefined = text ? [['text.ilx', (await import('../../../tests/fixtures/text.ilx?raw')).default]] : undefined;
  mount(App, { target, props: { backend: await core(files), sample: true } });
} else {
  target.textContent = 'This is isolex studio’s interface: open it in the app.';
}
