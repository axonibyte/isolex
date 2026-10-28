import { invoke } from "@tauri-apps/api/core";

/** This build's versions, as the shell's `about` command gives them. */
export interface About {
  studio: string;
  isolex: string;
}

/** Whether the interface is in the studio's window, not a plain browser. */
export const inShell = (): boolean => "__TAURI_INTERNALS__" in globalThis;

/** The versions from the shell, or none outside it. */
export async function about(): Promise<About | null> {
  return inShell() ? invoke<About>("about") : null;
}

/** The line the status bar shows for them. */
export function describe(a: About | null): string {
  return a ? `isolex studio ${a.studio} · isolex ${a.isolex}` : "isolex studio · not in the studio's window";
}
