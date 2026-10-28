// What jsdom lacks and the window has: a ResizeObserver (Svelte's
// bind:clientWidth) that never reports a resize.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
