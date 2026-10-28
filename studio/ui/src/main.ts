import { mount } from 'svelte';
import '@fontsource/geist-sans/400.css';
import '@fontsource/geist-sans/600.css';
import '@fontsource/geist-mono/400.css';
import './app.css';
import App from './App.svelte';
import { inShell, shell } from './lib/backend';

// In the studio's window, the shell; in a plain browser, the sample library
// (for working on the interface without the app, and for its tests).
const backend = inShell() ? await shell() : (await import('./lib/fake')).fake();
mount(App, { target: document.getElementById('app')!, props: { backend, sample: !inShell() } });
