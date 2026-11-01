import type { Tab } from "$lib/types";
import { uid } from "$lib/utils/id";
import { MAX_ZOOM, MIN_ZOOM } from "$lib/constants";
import { documents } from "./documents.svelte";
import { settings, updateSettings } from "./settings.svelte";
import { closeDocument } from "$lib/pdf/loader";
import { renderDpr } from "$lib/pdf/renderPage";
import {
  clearRecentDocs as clearRecentDocsCommand,
  saveDocumentProgress,
} from "$lib/tauri/commands";
import { showToast } from "./toast.svelte";
import { ui } from "./ui.svelte";
import { clearViewerPages, viewer } from "./viewer.svelte";
import { whenDocumentLoaded } from "$lib/pdf/documentState.svelte";

export const tabs = $state<{
  items: Tab[];
  activeId: string | null;
  /** Pestañas cerradas recientemente; `zoom` solo se restaura cuando
   * `consistentDocZoom` está desactivado. */
  recentlyClosed: { docId: string; page: number; zoom: number }[];
}>({
  items: [],
  activeId: null,
  recentlyClosed: [],
});

/** Tope de espera al restaurar una pestaña: si el PDF no llega, la pestaña se da
 *  por lista igualmente para no dejarla bloqueada. */
const RESTORE_TIMEOUT_MS = 8000;

const progressTimers = new Map<string, ReturnType<typeof setTimeout>>();
const progressWrites = new Set<Promise<void>>();
let sharedZoomTimer: ReturnType<typeof setTimeout> | null = null;

/** Actualiza el progreso de lectura sin tocar `lastOpened`. */
function scheduleProgress(tab: Tab): void {
  const doc = documents.items.get(tab.docId);
  if (!doc) return;
  // Con `consistentDocZoom`, el zoom compartido es el que se persiste, no el
  // individual de la pestaña. Antes se guardaba `doc.lastZoom` (valor viejo).
  const persistedZoom = settings.consistentDocZoom
    ? Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, settings.globalZoom))
    : tab.zoom;
  // Se mutan los campos en el sitio en vez de reemplazar el objeto. `SvelteMap`
  // avisa por **identidad** de valor (los valores no se proxifican), así que
  // sustituirlo invalidaría a todo el que lee el documento —miniaturas,
  // inspector, búsqueda, filas del sidebar— en cada cambio de página, mientras
  // que mutarlo no despierta a nadie. `lastPage`/`lastZoom` no los lee nadie de
  // forma reactiva (el visor usa `tab.page`/`tab.zoom`), así que aquí lo que se
  // quiere es precisamente no notificar. Ojo: al contrario de lo que decía este
  // comentario, mutar **no** notifica; el efecto de no tocar el mapa aquí.
  doc.lastPage = Math.max(1, Math.round(tab.page));
  doc.lastZoom = persistedZoom;

  const existingTimer = progressTimers.get(doc.id);
  if (existingTimer) clearTimeout(existingTimer);
  progressTimers.set(doc.id, setTimeout(() => {
    progressTimers.delete(doc.id);
    const current = documents.items.get(doc.id);
    if (!current) return;
    const write = saveDocumentProgress(doc.id, current.lastPage, current.lastZoom);
    progressWrites.add(write);
    void write
      .catch((error: unknown) => {
        console.error("[documents] No se pudo guardar la última posición", error);
      })
      .finally(() => progressWrites.delete(write));
  }, 250));
}

/** Marca el documento como abierto y actualiza el progreso. */
function touchDocumentOpen(tab: Tab): void {
  const doc = documents.items.get(tab.docId);
  if (!doc) return;
  // Aquí sí hay que pasar por `set`: `RecentList` deriva su lista de `inRecent` y
  // `lastOpened`, y mutarlos en el sitio no avisaría a nadie (los valores de
  // `SvelteMap` no son reactivos), así que el orden de «Recientes» no se
  // actualizaba al abrir un documento. Es un camino frío —abrir o activar una
  // pestaña—, no el de cada cambio de página, así que invalidar a los lectores
  // del documento aquí no cuesta nada.
  documents.items.set(doc.id, {
    ...doc,
    lastOpened: Date.now(),
    inRecent: true,
  });
  scheduleProgress(tab);
}

/** Fuerza el guardado del progreso pendiente. Usado por `flushOnClose`. */
export async function flushPendingProgress(): Promise<void> {
  for (const timer of progressTimers.values()) clearTimeout(timer);
  progressTimers.clear();
  await Promise.allSettled([...progressWrites]);
  const writes: Promise<void>[] = [];
  for (const tab of tabs.items) {
    const doc = documents.items.get(tab.docId);
    if (!doc) continue;
    writes.push(
      saveDocumentProgress(doc.id, doc.lastPage, doc.lastZoom).catch((error: unknown) => {
        console.error("[documents] No se pudo guardar el progreso al cerrar", error);
      }),
    );
  }
  await Promise.allSettled(writes);
}

export async function clearRecentHistory(): Promise<void> {
  for (const timer of progressTimers.values()) clearTimeout(timer);
  progressTimers.clear();
  await Promise.allSettled([...progressWrites]);
  await clearRecentDocsCommand();
  // Se muta el mapa en vez de reasignarlo: `SvelteMap` avisa por clave, así que
  // solo se recalculan las filas que cambian, y quien tenga una referencia al
  // mapa (los efectos que leen `documents.items`) sigue viendo los datos.
  for (const [id, doc] of [...documents.items]) {
    if (!doc.inRecent && doc.lastOpened === null) continue;
    documents.items.set(id, { ...doc, inRecent: false, lastOpened: null });
  }
}

export function deactivateActiveTab(): void {
  const tab = getActiveTab();
  if (!tab) return;
  tab.status = "idle";
  tab.lastActive = Date.now();
  tabs.activeId = null;
}

export function getActiveTab(): Tab | null {
  return tabs.items.find((t) => t.id === tabs.activeId) ?? null;
}

export function getTabByDocId(docId: string): Tab | null {
  return tabs.items.find((t) => t.docId === docId) ?? null;
}

function finishRestore(tabId: string, token: number): void {
  const current = tabs.items.find((t) => t.id === tabId);
  if (!current || current._restoreToken !== token) return;
  if (current.status !== "restoring") return;
  current.status = tabs.activeId === tabId ? "active" : "idle";
  current.lastActive = Date.now();
}

export function openDocument(docId: string, applyLimits = true): void {
  const doc = documents.items.get(docId);
  if (!doc) return;

  const existing = getTabByDocId(docId);
  if (existing) {
    setActiveTab(existing.id);
    return;
  }

  const previous = getActiveTab();
  if (previous && previous.status === "active") {
    previous.status = "idle";
    previous.lastActive = Date.now();
  }

  const tab: Tab = {
    id: uid("tab"),
    docId,
    page: settings.openLastLocation
      ? (doc.pages > 0
          ? Math.min(Math.max(Number.isFinite(doc.lastPage) ? Math.round(doc.lastPage) : 1, 1), doc.pages)
          : (Number.isFinite(doc.lastPage) ? Math.max(1, Math.round(doc.lastPage)) : 1))
      : 1,
    zoom: settings.consistentDocZoom
      ? settings.globalZoom
      : settings.openLastLocation
        ? (Number.isFinite(doc.lastZoom) ? doc.lastZoom : 100)
        : 100,
    status: "active",
    lastActive: Date.now(),
    _restoreToken: 0,
  };

  tabs.items.push(tab);
  tabs.activeId = tab.id;
  touchDocumentOpen(tab);
  if (applyLimits) enforceLimits();
}

export function setActiveTab(tabId: string): void {
  const tab = tabs.items.find((t) => t.id === tabId);
  if (!tab) return;

  const previous = getActiveTab();
  if (previous && previous.id !== tab.id && previous.status === "active") {
    previous.status = "idle";
    previous.lastActive = Date.now();
  }

  if (tab.status === "hibernated") {
    tabs.activeId = tab.id;
    tab.status = "restoring";
    touchDocumentOpen(tab);
    tab._restoreToken += 1;
    const token = tab._restoreToken;

    // Esperar a que el PDF pinte de verdad en lugar de a un tiempo fijo.
    void (async () => {
      await whenDocumentLoaded(tab.docId, RESTORE_TIMEOUT_MS);
      const current = tabs.items.find((t) => t.id === tabId);
      if (!current || current._restoreToken !== token) return;
      finishRestore(tabId, token);
    })();
    return;
  }

  tabs.activeId = tab.id;
  tab.status = "active";
  tab.lastActive = Date.now();
  touchDocumentOpen(tab);
}

export function closeTab(tabId: string): void {
  const index = tabs.items.findIndex((t) => t.id === tabId);
  if (index === -1) return;
  const tab = tabs.items[index]!;

  tabs.recentlyClosed.unshift({ docId: tab.docId, page: tab.page, zoom: tab.zoom });
  if (tabs.recentlyClosed.length > settings.maxClosedTabs) {
    tabs.recentlyClosed.length = settings.maxClosedTabs;
  }

  tabs.items.splice(index, 1);

  if (tabs.activeId === tabId) {
    const replacement = tabs.items.find((t) => t.status !== "hibernated") ?? tabs.items[0] ?? null;
    tabs.activeId = null;
    if (replacement) setActiveTab(replacement.id);
  }

  if (!getTabByDocId(tab.docId)) {
    queueMicrotask(() => {
      if (!getTabByDocId(tab.docId)) {
        clearViewerPages(tab.docId);
        void closeDocument(tab.docId);
      }
    });
  }
}

export function forgetDocuments(docIds: string[]): void {
  const forgotten = new Set(docIds);
  for (const tab of [...tabs.items]) {
    if (forgotten.has(tab.docId)) closeTab(tab.id);
  }
  tabs.recentlyClosed = tabs.recentlyClosed.filter((item) => !forgotten.has(item.docId));
}

export function restoreLastClosedTab(): boolean {
  let item: (typeof tabs.recentlyClosed)[number] | undefined;
  for (let index = 0; index < tabs.recentlyClosed.length;) {
    const candidate = tabs.recentlyClosed[index]!;
    if (!documents.items.has(candidate.docId)) {
      tabs.recentlyClosed.splice(index, 1);
      continue;
    }
    item = candidate;
    tabs.recentlyClosed.splice(index, 1);
    break;
  }
  if (!item) {
    showToast("No se puede restaurar la pestaña: el documento ya no está disponible.");
    return false;
  }

  const existing = getTabByDocId(item.docId);
  if (existing) {
    setActiveTab(existing.id);
    showToast("Pestaña restaurada.");
    return true;
  }

  const previous = getActiveTab();
  if (previous && previous.status === "active") {
    previous.status = "idle";
    previous.lastActive = Date.now();
  }

  const tab: Tab = {
    id: uid("tab"),
    docId: item.docId,
    page: item.page,
    zoom: settings.consistentDocZoom ? settings.globalZoom : item.zoom,
    status: "active",
    lastActive: Date.now(),
    _restoreToken: 0,
  };
  tabs.items.push(tab);
  tabs.activeId = tab.id;
  touchDocumentOpen(tab);
  enforceLimits();
  showToast("Pestaña restaurada.");
  return true;
}

export function hibernateTab(tabId: string): void {
  const tab = tabs.items.find((t) => t.id === tabId);
  if (!tab || tab.status === "hibernated") return;
  if (tab.id === tabs.activeId) return;
  tab.status = "hibernated";
  clearViewerPages(tab.docId);
  void closeDocument(tab.docId);
}

export function enforceLimits(): void {
  while (tabs.items.length > settings.maxActiveFiles) {
    const candidate = tabs.items
      .filter((t) => t.id !== tabs.activeId)
      .sort((a, b) => a.lastActive - b.lastActive)[0];
    if (!candidate) break;
    closeTab(candidate.id);
  }

  while (true) {
    const loaded = tabs.items.filter((t) => t.status !== "hibernated");
    if (loaded.length <= settings.maxLoadedTabs || loaded.length <= 1) break;
    const candidate = loaded
      .filter((t) => t.id !== tabs.activeId)
      .sort((a, b) => a.lastActive - b.lastActive)[0];
    if (!candidate) break;
    candidate.status = "hibernated";
    clearViewerPages(candidate.docId);
    void closeDocument(candidate.docId);
  }
  enforceMemoryLimit();
}

export function estimateMemoryMb(): number {
  let bytes = 0;
  for (const tab of tabs.items) {
    if (tab.status === "hibernated") continue;
    const doc = documents.items.get(tab.docId);
    if (doc?.sizeBytes) bytes += Math.round(doc.sizeBytes * 1.5);
  }
  // Cada página montada ocupa dos lienzos (doble búfer) de `ancho * dpr` por
  // `alto * dpr` píxeles; sin el DPR la estimación se quedaba corta por `dpr²`
  // (hasta 4× en pantallas Retina) y el límite de RAM no llegaba a activarse.
  const { pageWidth, pageHeight } = viewer.dims;
  const dpr = renderDpr();
  bytes += viewer.loadedPages.size * pageWidth * dpr * pageHeight * dpr * 4 * 2;
  return bytes / (1024 * 1024);
}

export function enforceMemoryLimit(): void {
  const limitMb = settings.ramLimitMB;
  if (!Number.isFinite(limitMb) || limitMb <= 0) return;
  let guard = 0;
  while (estimateMemoryMb() > limitMb && guard++ < tabs.items.length) {
    const candidate = tabs.items
      .filter((tab) =>
        tab.id !== tabs.activeId &&
        tab.status !== "hibernated" &&
        tab.status !== "restoring"
      )
      .sort((a, b) => a.lastActive - b.lastActive)[0];
    if (!candidate) break;
    candidate.status = "hibernated";
    clearViewerPages(candidate.docId);
    void closeDocument(candidate.docId);
  }
}

export function hibernateInactiveTabs(): void {
  const timeoutSec = settings.inactiveTimeoutSec;
  if (!Number.isFinite(timeoutSec) || timeoutSec <= 0) return;
  if (Date.now() - ui.lastActivity < timeoutSec * 1000) return;
  const cutoff = Date.now() - timeoutSec * 1000;
  for (const tab of tabs.items) {
    if (
      tab.id === tabs.activeId ||
      tab.status === "hibernated" ||
      tab.status === "restoring"
    ) continue;
    if (tab.lastActive > cutoff) continue;
    tab.status = "hibernated";
    clearViewerPages(tab.docId);
    void closeDocument(tab.docId);
  }
}

export function updateTabPage(tabId: string, page: number, zoom: number): void {
  const tab = tabs.items.find((t) => t.id === tabId);
  if (!tab) return;
  const doc = documents.items.get(tab.docId);
  // Con `pages = 0` todavía no hay límite superior: pdf.js corregirá el total
  // al abrir el documento. Acotar en este estado bloquea la navegación.
  const upper = doc && doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
  const safePage = Number.isFinite(page) ? Math.round(page) : 1;
  tab.page = doc
    ? Math.min(Math.max(safePage, 1), upper)
    : safePage;
  const nextZoom = Number.isFinite(zoom) ? Math.min(Math.max(zoom, MIN_ZOOM), MAX_ZOOM) : 100;
  const zoomChanged = tab.zoom !== nextZoom;
  tab.zoom = nextZoom;
  tab.lastActive = Date.now();
  scheduleProgress(tab);

  if (zoomChanged && settings.consistentDocZoom) {
    if (settings.globalZoom !== nextZoom) updateSettings("globalZoom", nextZoom);
    for (const otherTab of tabs.items) {
      if (otherTab.id === tab.id || otherTab.zoom === nextZoom) continue;
      otherTab.zoom = nextZoom;
    }

    if (sharedZoomTimer) clearTimeout(sharedZoomTimer);
    sharedZoomTimer = setTimeout(() => {
      sharedZoomTimer = null;
      // Updating every tab's zoom must not make every document appear in
      // Recientes. Only persist the changed progress; opening/activating a tab
      // is the operation that updates lastOpened.
      for (const otherTab of tabs.items) scheduleProgress(otherTab);
    }, 300);
  }
}

export function clampPagesForDocument(docId: string, pages: number): void {
  if (!Number.isFinite(pages) || pages <= 0) return;
  for (const tab of tabs.items) {
    if (tab.docId !== docId) continue;
    const nextPage = Math.min(
      Math.max(Number.isFinite(tab.page) ? Math.round(tab.page) : 1, 1),
      Math.floor(pages),
    );
    if (tab.page === nextPage) continue;
    tab.page = nextPage;
    scheduleProgress(tab);
  }
}

export function setConsistentDocZoom(enabled: boolean): void {
  if (enabled === settings.consistentDocZoom) return;
  const activeTab = getActiveTab();
  const sharedZoom = activeTab?.zoom ?? settings.globalZoom;
  updateSettings("consistentDocZoom", enabled);

  if (!enabled) {
    if (sharedZoomTimer) clearTimeout(sharedZoomTimer);
    sharedZoomTimer = null;
    for (const tab of tabs.items) {
      tab.zoom = documents.items.get(tab.docId)?.lastZoom ?? tab.zoom;
      scheduleProgress(tab);
    }
    return;
  }

  settings.globalZoom = Number.isFinite(sharedZoom)
    ? Math.min(999, Math.max(40, sharedZoom))
    : 100;
  updateSettings("globalZoom", settings.globalZoom);
  for (const tab of tabs.items) {
    tab.zoom = settings.globalZoom;
  }
}
