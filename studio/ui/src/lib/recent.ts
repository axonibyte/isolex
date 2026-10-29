// The libraries opened lately, and whether the welcome panel shows at
// startup (docs/studio-interface.md, A library and the disk), remembered
// per person as the themes are. Pure but for where they are kept.

export interface Recent {
  path: string;
  /** Its folder's name. */
  name: string;
  definitions: number;
  /** When it was opened, in milliseconds since the epoch. */
  opened: number;
}

const KEY = 'studio.recent';
const STARTUP = 'studio.welcome';
const KEEP = 8;

function read<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // remembered or not, it applies now
  }
}

export const recent = (): Recent[] => read<Recent[]>(KEY, []).filter((r) => r && typeof r.path === 'string');

/** A library opened: first in the list, once, the list kept short. */
export function remember(r: Recent): Recent[] {
  const next = [r, ...recent().filter((x) => x.path !== r.path)].slice(0, KEEP);
  write(KEY, next);
  return next;
}

export function forget(path: string): Recent[] {
  const next = recent().filter((x) => x.path !== path);
  write(KEY, next);
  return next;
}

/** Whether the welcome panel shows at startup; when not, the library
    opened last opens. */
export const showAtStartup = (): boolean => read<boolean>(STARTUP, true);
export const setShowAtStartup = (on: boolean) => write(STARTUP, on);

/** A folder's name from its path, either slash. */
export const folderName = (path: string): string => path.replace(/[\\/]+$/, '').split(/[\\/]/).at(-1) ?? path;

/** When, as the panel says it: today's time, else the day. */
export function when(at: number, now = Date.now()): string {
  const d = new Date(at);
  const same = new Date(now).toDateString() === d.toDateString();
  return same ? d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : d.toLocaleDateString();
}
