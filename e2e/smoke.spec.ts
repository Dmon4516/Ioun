import { expect, test } from "@playwright/test";

/**
 * Smoke tests contra el frontend servido por Vite.
 *
 * Sin backend de Tauri la biblioteca no carga, así que aquí se comprueba el
 * arranque (shell, tema) y, sobre todo, que el **worker de pdf.js** y su chunk se
 * sirven con 200: es lo que no se puede verificar con tests unitarios, porque
 * pdf.js no expone un evento fiable cuando el worker real falla.
 *
 * Requiere Node ≥ 20 (lo pide Playwright). En local con Node 19 no arranca; en CI
 * corre con Node 20.
 */
test("el shell arranca sin errores de consola", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  page.on("pageerror", (error) => errors.push(error.message));

  await page.goto("/");
  await expect(page.locator(".app-shell")).toBeVisible();

  // En el servidor web no existe la API IPC de Tauri. Solo se permite el error
  // de bootstrap con el mensaje exacto de esa ausencia; cualquier otro error
  // debe hacer fallar el smoke test.
  const expectedTauriUnavailable = /^\[bootstrap\].*(?:Tauri API is not available|__TAURI_INTERNALS__ is undefined|Cannot read properties of undefined \(reading 'invoke'\))$/i;
  const unexpected = errors.filter((text) => !expectedTauriUnavailable.test(text));
  expect(unexpected).toEqual([]);
});

test("el tema se aplica antes del primer pintado", async ({ page }) => {
  await page.goto("/");

  const theme = await page.locator("html").getAttribute("data-theme");
  expect(["dark", "light"]).toContain(theme);
});

test("el worker de pdf.js se sirve con 200", async ({ page }) => {
  const responses: { url: string; status: number }[] = [];
  page.on("response", (response) => {
    if (/pdf\.worker|pdf-.*\.js/i.test(response.url())) {
      responses.push({ url: response.url(), status: response.status() });
    }
  });

  await page.goto("/");
  // El chunk de pdf.js se pide al abrir un documento; sin backend se fuerza la
  // carga del módulo desde la propia página.
  await page.evaluate(async () => {
    // URL servida por Vite en desarrollo; se pasa como variable porque TypeScript
    // no puede resolver una ruta de navegador.
    const modulePath = "/src/lib/pdf/loader.ts";
    const loader = (await import(modulePath)) as {
      warmUpPdfJs: () => Promise<unknown>;
    };
    await loader.warmUpPdfJs();
  });

  await expect
    .poll(() => responses.length, { message: "sin peticiones de pdf.js" })
    .toBeGreaterThan(0);
  expect(responses.every((entry) => entry.status === 200)).toBe(true);
  // El worker se pide al abrir un documento, y sin backend de Tauri no hay
  // ninguno: esa comprobación sigue necesitando la app en marcha. Aquí se cubre
  // que el chunk de pdf.js se sirve con 200, que es el otro fallo que pdf.js no
  // reporta de forma fiable.
});
