/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";

// Tauri expects a fixed dev port and must not have its Rust sources watched.
export default defineConfig({
  // svelteTesting (active only under Vitest) resolves Svelte's browser build
  // so components mount in jsdom, and cleans the DOM after each test.
  plugins: [svelte(), svelteTesting()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test-setup.ts"],
  },
});
