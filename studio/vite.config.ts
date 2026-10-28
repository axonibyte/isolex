import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The interface's source is in ui/, and builds to dist/, which the
// shell (src-tauri) serves; `npm run tauri dev` serves it from Vite.
export default defineConfig({
  root: "ui",
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { outDir: "../dist", emptyOutDir: true, target: "es2022" },
  // components mount as they do in the window, not as on a server
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
  test: { environment: "jsdom", include: ["src/**/*.test.ts"] },
});
