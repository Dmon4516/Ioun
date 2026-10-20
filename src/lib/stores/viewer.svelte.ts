import type { FitMode } from "$lib/types";
import { DEFAULT_ASPECT_RATIO, PAGE_GAP, VIEWER_PADDING_TOP } from "$lib/constants";
import { SvelteSet } from "svelte/reactivity";

export interface PageDims {
  pageWidth: number;
  pageHeight: number;
  pagesPerRow: number;
  gap: number;
}

/** `top` deja la página al principio del área; `center` la centra (recentrado). */
export type ScrollAlign = "top" | "center";

export interface ScrollTarget {
  page: number;
  smooth: boolean;
  align: ScrollAlign;
}

interface ViewerState {
  contentWidth: number;
  contentHeight: number;
  dims: PageDims;
  /** Claves `"docId:page"` para separar páginas de distintos documentos. */
  loadedPages: SvelteSet<string>;
  visiblePages: SvelteSet<string>;
  inFlightRenders: SvelteSet<string>;
  scrollTarget: ScrollTarget | null;
  viewportPaddingTop: number;
  scrollAtTop: boolean;
  scrollAtBottom: boolean;
  zooming: boolean;
  renderBudgetFull: boolean;
}

export const viewer = $state<ViewerState>({
  contentWidth: 800,
  contentHeight: 800,
  dims: { pageWidth: 760, pageHeight: 1000, pagesPerRow: 1, gap: PAGE_GAP },
  loadedPages: new SvelteSet<string>(),
  visiblePages: new SvelteSet<string>(),
  inFlightRenders: new SvelteSet<string>(),
  scrollTarget: null,
  viewportPaddingTop: VIEWER_PADDING_TOP,
  scrollAtTop: true,
  scrollAtBottom: false,
  zooming: false,
  renderBudgetFull: false,
});

export const viewerCommands = $state<{
  zoomToCenter: { zoom: number; token: number } | null;
  fitPreserveCenter: { token: number } | null;
}>({
  zoomToCenter: null,
  fitPreserveCenter: null,
});

let nextCommandToken = 0;

export function commandZoomToCenter(zoom: number): void {
  viewerCommands.zoomToCenter = { zoom, token: ++nextCommandToken };
}

export function commandFitPreserveCenter(): void {
  viewerCommands.fitPreserveCenter = { token: ++nextCommandToken };
}

/**
 * Calcula las dimensiones de página dadas las condiciones actuales.
 * Es pura: no lee ni escribe el store.
 */
export function computeDims(
  zoom: number,
  pagesPerRow: number,
  fitMode: FitMode,
  contentWidth: number,
  contentHeight: number,
  pageAspectRatio = DEFAULT_ASPECT_RATIO,
): PageDims {
  const gap = PAGE_GAP;
  const aspectRatio = Number.isFinite(pageAspectRatio) && pageAspectRatio > 0
    ? pageAspectRatio
    : DEFAULT_ASPECT_RATIO;
  const widthForRow = Math.max(1, contentWidth - (pagesPerRow - 1) * gap) /
    pagesPerRow;
  const heightForPage = Math.max(1, contentHeight) * aspectRatio;
  let baseWidth: number;

  switch (fitMode) {
    case "page":
      baseWidth = Math.min(widthForRow, heightForPage);
      break;
    case "fill":
      baseWidth = Math.max(widthForRow, heightForPage);
      break;
    default:
      baseWidth = widthForRow;
  }

  const pageWidth = Math.max(1, Math.round(baseWidth * (zoom / 100)));
  const pageHeight = Math.max(1, Math.round(pageWidth / aspectRatio));
  return { pageWidth, pageHeight, pagesPerRow, gap };
}

/** Construye la clave compartida por los conjuntos de páginas del visor. */
export function pageKey(docId: string, page: number): string {
  return `${docId}:${page}`;
}

export function registerLoaded(docId: string, page: number): void {
  viewer.loadedPages.add(pageKey(docId, page));
}

export function unregisterLoaded(docId: string, page: number): void {
  viewer.loadedPages.delete(pageKey(docId, page));
}

export function beginRender(docId: string, page: number, maxInFlight: number): void {
  viewer.inFlightRenders.add(pageKey(docId, page));
  syncRenderBudget(maxInFlight);
}

export function endRender(docId: string, page: number, maxInFlight: number): void {
  viewer.inFlightRenders.delete(pageKey(docId, page));
  syncRenderBudget(maxInFlight);
}

/**
 * Refresca el aviso de "presupuesto de renders lleno". Es un booleano y no
 * `inFlightRenders.size` a propósito: los slots lo observan para reintentar lo
 * que se saltaron por presupuesto, y leer el tamaño haría que cada
 * `beginRender`/`endRender` de cualquier slot invalidara el efecto de todos los
 * demás. Al ser un booleano, solo avisa al llenarse y al liberarse.
 */
function syncRenderBudget(maxInFlight: number): void {
  viewer.renderBudgetFull =
    Number.isFinite(maxInFlight) && maxInFlight > 0 && viewer.inFlightRenders.size >= maxInFlight;
}

export function isRenderBudgetExhausted(docId: string, max: number, page: number): boolean {
  if (!Number.isFinite(max) || max <= 0) return false;
  return viewer.inFlightRenders.size >= max &&
    !viewer.visiblePages.has(pageKey(docId, page));
}

export function setVisiblePages(docId: string, pages: Set<number>): void {
  const prefix = `${docId}:`;
  for (const key of [...viewer.visiblePages]) {
    if (key.startsWith(prefix)) viewer.visiblePages.delete(key);
  }
  for (const page of pages) viewer.visiblePages.add(pageKey(docId, page));
}

/** Limpia las páginas registradas de un documento que ya no está montado. */
export function clearViewerPages(docId: string): void {
  const prefix = `${docId}:`;
  for (const set of [viewer.loadedPages, viewer.visiblePages, viewer.inFlightRenders]) {
    for (const key of [...set]) {
      if (key.startsWith(prefix)) set.delete(key);
    }
  }
}

/**
 * Desplaza el visor una fracción de su alto (una pantalla por defecto), que es
 * el paso que usan las flechas del teclado y los botones arriba/abajo.
 */
export function scrollViewerBy(
  direction: 1 | -1,
  fraction = 0.85,
  minStep = 80,
): void {
  const area = document.getElementById("viewerArea");
  if (!area) return;
  const step = Math.max(minStep, area.clientHeight * fraction);
  area.scrollBy({ top: direction * step, behavior: "smooth" });
}
