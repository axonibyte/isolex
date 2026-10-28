// The window's theme and the canvas's (docs/studio-interface.md, Look),
// remembered per person: dark around light by default.

export type Theme = 'dark' | 'light' | 'system';
export type CanvasTheme = 'light' | 'dark';

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // remembered or not, it applies now
  }
}

export function theme(): Theme {
  const t = read('studio.theme');
  return t === 'light' || t === 'system' ? t : 'dark';
}

export function canvasTheme(): CanvasTheme {
  return read('studio.canvas') === 'dark' ? 'dark' : 'light';
}

export function setTheme(t: Theme) {
  document.documentElement.dataset.theme = t;
  write('studio.theme', t);
}

export function setCanvasTheme(t: CanvasTheme) {
  document.documentElement.dataset.canvas = t;
  write('studio.canvas', t);
}

/** isolex's built-in palette (`isolex render`'s), with dark colors of the
    studio's own, for a library with no `palette` block. */
export const DEFAULT_PALETTE: Record<string, { light: string; dark: string }> = {
  'body-top': { light: '#e4e7eb', dark: '#52606d' },
  'body-left': { light: '#cbd2d9', dark: '#3e4c59' },
  'body-right': { light: '#9aa5b1', dark: '#323f4b' },
  outline: { light: '#1f2933', dark: '#0b0d10' },
  ink: { light: '#1f2933', dark: '#e4e7eb' },
  'ink-muted': { light: '#7b8794', dark: '#9aa5b1' },
  shadow: { light: '#000000', dark: '#000000' },
  virt: { light: '#d9e2ec', dark: '#3e4c59' },
  accent: { light: '#d64545', dark: '#ef8a8a' },
  'accent-dim': { light: '#e8a0a0', dark: '#a05a5a' },
};
