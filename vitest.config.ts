import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";
import path from "node:path";

export default defineConfig({
  // `svelteTesting()` resuelve el build de cliente de Svelte (sin él, montar un
  // componente en los tests falla con "mount(...) is not available on the server")
  // y limpia el DOM entre tests.
  plugins: [svelte({ hot: false }), svelteTesting()],
  resolve: {
    alias: {
      $lib: path.resolve("./src/lib"),
      $components: path.resolve("./src/components"),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.{test,spec}.{ts,js}"],
    exclude: ["node_modules", "src-tauri", "dist"],
  },
});
