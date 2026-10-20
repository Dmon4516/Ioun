import { defineConfig } from "@playwright/test";

/**
 * Pruebas de extremo a extremo contra el frontend servido por Vite.
 *
 * Aquí no hay backend de Tauri, así que la biblioteca no carga: esto comprueba el
 * arranque (shell, tema, ajustes) y, sobre todo, que el **worker de pdf.js** se
 * sirve con 200, que es la parte que no se puede verificar con tests unitarios.
 * Están separadas de `vitest` (ver `e2e/`) y no entran en `npm test`.
 */
export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  expect: { timeout: 5_000 },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://localhost:1420",
    trace: "on-first-retry",
  },
  webServer: {
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: true,
    timeout: 60_000,
  },
});
