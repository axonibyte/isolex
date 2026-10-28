<script lang="ts">
  import { onMount } from "svelte";
  import { about, describe, type About } from "./about";

  // The scaffold's window: the frame (#67) replaces it with the editor
  // docs/studio-interface.md specifies.
  let versions: About | null = $state(null);
  onMount(async () => {
    versions = await about();
  });
</script>

<div class="app">
  <header>
    <span class="brand">isolex studio</span>
  </header>
  <main aria-label="Canvas">
    <p>No library open.</p>
  </main>
  <footer role="status">{describe(versions)}</footer>
</div>

<style>
  :global(:root) {
    --bg: #121317;
    --panel: #1a1c21;
    --line: #2c3038;
    --fg: #e7e9ee;
    --muted: #9aa1ad;
    --canvas: #f3f4f7;
    --canvas-fg: #3a4250;
    color-scheme: dark;
  }
  :global(html, body) {
    height: 100%;
    margin: 0;
  }
  :global(body) {
    background: var(--bg);
    color: var(--fg);
    font: 13px/1.4 system-ui, sans-serif;
  }
  .app {
    height: 100%;
    display: grid;
    grid-template-rows: 48px minmax(0, 1fr) 30px;
  }
  header,
  footer {
    display: flex;
    align-items: center;
    padding: 0 12px;
    background: var(--panel);
  }
  header {
    border-bottom: 1px solid var(--line);
  }
  footer {
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 12px;
  }
  .brand {
    font-weight: 600;
  }
  main {
    display: grid;
    place-items: center;
    background: var(--canvas);
    color: var(--canvas-fg);
  }
</style>
