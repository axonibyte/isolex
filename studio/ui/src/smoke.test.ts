import { afterEach, describe as group, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import App from "./App.svelte";
import { describe, inShell } from "./about";

group("the scaffold", () => {
  let app: Record<string, unknown> | null = null;
  afterEach(() => {
    if (app) unmount(app);
    app = null;
    document.body.innerHTML = "";
  });

  it("knows it is not in the studio's window here", () => {
    expect(inShell()).toBe(false);
    expect(describe(null)).toBe("isolex studio · not in the studio's window");
  });

  it("names the versions the shell gives it", () => {
    expect(describe({ studio: "0.1.0", isolex: "0.5.0" })).toBe("isolex studio 0.1.0 · isolex 0.5.0");
  });

  it("mounts the window with its status line", () => {
    app = mount(App, { target: document.body });
    flushSync();
    expect(document.querySelector(".brand")?.textContent).toBe("isolex studio");
    expect(document.querySelector('[role="status"]')?.textContent).toContain("isolex studio");
  });
});
