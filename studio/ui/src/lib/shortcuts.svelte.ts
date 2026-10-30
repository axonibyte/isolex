// The keys as a person has bound them (#119): the keymap's table with
// their rebindings laid over it, kept per person and read again in every
// window.

import { COMMANDS, commandFor, keysOf, readBindings, rebind, reset, type Bindings, type Command } from './keymap';

const KEY = 'shortcuts';

export class Shortcuts {
  over = $state<Bindings>(readBindings(stored()));

  /** The keys of `id`. */
  keys(id: string): string[] {
    return keysOf(id, this.over);
  }

  /** A label with the first key of `id`, as a tooltip names it. */
  titled(label: string, id: string): string {
    const key = this.keys(id)[0];
    return key ? `${label} (${key})` : label;
  }

  /** The command `key` runs. */
  find(key: string): Command | null {
    return commandFor(key, this.over);
  }

  /** `id` bound to `key` alone, or to none: why not, if refused. */
  set(id: string, key: string | null): string | null {
    return this.apply(rebind(this.over, id, key));
  }

  /** `id` given its own keys again: why not, if refused. */
  reset(id: string): string | null {
    return this.apply(reset(this.over, id));
  }

  /** Every command given its own keys again. */
  resetAll() {
    this.apply({ over: {} });
  }

  /** The command `id` has been rebound. */
  changed(id: string): boolean {
    return id in this.over;
  }

  get commands(): Command[] {
    return COMMANDS;
  }

  private apply(r: { over: Bindings } | { refused: string }): string | null {
    if ('refused' in r) return r.refused;
    this.over = r.over;
    try {
      localStorage.setItem(KEY, JSON.stringify(r.over));
    } catch {
      // kept for this window only
    }
    return null;
  }
}

function stored(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}
