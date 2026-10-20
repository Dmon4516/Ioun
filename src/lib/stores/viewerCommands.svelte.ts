import { viewer, type ScrollAlign } from "./viewer.svelte";
import { getActiveTab, updateTabPage } from "./tabs.svelte";

/**
 * Solicita un scroll programático a una página. `Viewer.svelte` observa
 * `viewer.scrollTarget` y ejecuta el scroll real.
 *
 * Vive fuera de `viewer.svelte.ts` a propósito: `viewer` no debe depender de
 * `tabs` (que sí depende de `viewer` para estimar memoria). Este módulo es el
 * único punto donde ambos se tocan, y no lo importa ninguno de los dos.
 */
export function requestScrollToPage(
  page: number,
  smooth = true,
  align: ScrollAlign = "top",
): void {
  viewer.scrollTarget = { page, smooth, align };
  const tab = getActiveTab();
  if (tab && tab.page !== page) {
    updateTabPage(tab.id, page, tab.zoom);
  }
}
