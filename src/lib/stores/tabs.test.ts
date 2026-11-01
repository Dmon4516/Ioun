import { beforeEach, describe, expect, it, vi } from "vitest";
import { documents } from "./documents.svelte";
import {
  clearRecentHistory,
  clampPagesForDocument,
  estimateMemoryMb,
  openDocument,
  setConsistentDocZoom,
  tabs,
  updateTabPage,
} from "./tabs.svelte";
import { settings } from "./settings.svelte";
import { pageKey, registerLoaded, viewer } from "./viewer.svelte";
import { setInvokeMock } from "../../test/setup";

function seedDocument(
  id: string,
  extra: Partial<{ inRecent: boolean; lastOpened: number | null; sizeBytes: number; pages: number; lastPage: number }> = {},
): void {
  documents.items.set(id, {
    id,
    path: `/libros/${id}.pdf`,
    title: `${id}.pdf`,
    kind: "pdf",
    pages: extra.pages ?? 10,
    sizeBytes: extra.sizeBytes ?? 0,
    hash: null,
    categoryIds: [],
    lastPage: extra.lastPage ?? 1,
    lastZoom: 100,
    lastOpened: extra.lastOpened === undefined ? 1 : extra.lastOpened,
    inRecent: extra.inRecent ?? true,
    inLibrary: true,
    createdAt: 0,
    view: {},
    viewState: {},
    position: 0,
  });
}

beforeEach(() => {
  documents.items.clear();
  viewer.loadedPages.clear();
  viewer.visiblePages.clear();
  viewer.inFlightRenders.clear();
  vi.stubGlobal("devicePixelRatio", 1);
  setInvokeMock("clear_recent_docs", () => null);
});

describe("clearRecentHistory", () => {
  it("conserva la identidad del mapa y solo toca las filas que cambian", async () => {
    seedDocument("reciente", { inRecent: true, lastOpened: 99 });
    seedDocument("normal", { inRecent: false, lastOpened: null });
    const map = documents.items;
    const untouched = documents.items.get("normal");

    await clearRecentHistory();

    expect(documents.items).toBe(map);
    const cleared = documents.items.get("reciente");
    expect(cleared?.inRecent).toBe(false);
    expect(cleared?.lastOpened).toBeNull();
    expect(documents.items.get("normal")).toBe(untouched);
  });
});

describe("estimateMemoryMb", () => {
  it("cuenta los dos lienzos por página con el factor de resolución", () => {
    viewer.dims = { pageWidth: 500, pageHeight: 1000, pagesPerRow: 1, gap: 0 };
    registerLoaded("doc-a", 1);
    registerLoaded("doc-a", 2);

    vi.stubGlobal("devicePixelRatio", 1);
    const dpr1 = estimateMemoryMb();

    vi.stubGlobal("devicePixelRatio", 2);
    const dpr2 = estimateMemoryMb();

    expect(dpr1).toBeCloseTo((2 * 500 * 1000 * 4 * 2) / (1024 * 1024), 5);
    expect(dpr2).toBeCloseTo(dpr1 * 4, 5);
  });

  it("acota el factor de resolución a 2", () => {
    viewer.dims = { pageWidth: 100, pageHeight: 100, pagesPerRow: 1, gap: 0 };
    registerLoaded("doc-a", 1);

    vi.stubGlobal("devicePixelRatio", 2);
    const double = estimateMemoryMb();
    vi.stubGlobal("devicePixelRatio", 3);
    expect(estimateMemoryMb()).toBeCloseTo(double, 5);
  });

  it("cuenta páginas de documentos distintos sin colisión", () => {
    viewer.dims = { pageWidth: 100, pageHeight: 100, pagesPerRow: 1, gap: 0 };
    registerLoaded("doc-a", 5);
    registerLoaded("doc-b", 5);

    expect(viewer.loadedPages.size).toBe(2);
    expect(viewer.loadedPages.has(pageKey("doc-a", 5))).toBe(true);
    expect(viewer.loadedPages.has(pageKey("doc-b", 5))).toBe(true);
  });
});

describe("progreso del documento", () => {
  // Sustituir el objeto invalidaba a todos los consumidores del documento
  // (miniaturas, inspector, búsqueda) en cada cambio de página. Mutar los campos
  // en el sitio no despierta a nadie, que es justo lo que se quiere aquí: ni el
  // progreso ni el asomo lo lee nadie de forma reactiva.
  it("actualiza la página sin reemplazar el objeto del documento", () => {
    seedDocument("doc-progreso");
    openDocument("doc-progreso");
    const before = documents.items.get("doc-progreso");
    const tab = tabs.items.at(-1)!;

    updateTabPage(tab.id, 7, 120);

    const after = documents.items.get("doc-progreso");
    expect(after).toBe(before);
    expect(after?.lastPage).toBe(7);
    expect(after?.lastZoom).toBe(120);
  });
  // Al contrario que el progreso, `inRecent`/`lastOpened` sí se leen de forma
  // reactiva (`RecentList` ordena por `lastOpened`): si se mutaran en el sitio,
  // el mapa no avisaría y la lista no se reordenaría al abrir un documento.
  it("avisa al mapa al abrir un documento, para que Recientes se reordene", () => {
    seedDocument("doc-reciente", { inRecent: true, lastOpened: 1 });
    const before = documents.items.get("doc-reciente");

    openDocument("doc-reciente");

    const after = documents.items.get("doc-reciente");
    expect(after).not.toBe(before);
    expect(after?.lastOpened).toBeGreaterThan(1);
  });

  it("permite navegar mientras el total de páginas aún es desconocido", () => {
    seedDocument("doc-sin-conteo", { pages: 0, lastPage: 7 });

    openDocument("doc-sin-conteo");
    const tab = tabs.items.at(-1)!;
    expect(tab.page).toBe(7);

    updateTabPage(tab.id, 12, 100);
    expect(tab.page).toBe(12);
  });

  it("sanea páginas no finitas y persiste al acotar el total", async () => {
    const save = vi.fn(async () => undefined);
    setInvokeMock("save_document_progress", save);
    seedDocument("doc-clamp", { pages: 0, lastPage: 1 });
    openDocument("doc-clamp");
    const tab = tabs.items.at(-1)!;

    updateTabPage(tab.id, Number.NaN, Number.NaN);
    expect(tab.page).toBe(1);
    expect(tab.zoom).toBe(100);

    tab.page = 99;
    clampPagesForDocument("doc-clamp", 4);
    expect(tab.page).toBe(4);
    await new Promise((resolve) => setTimeout(resolve, 300));
    expect(save).toHaveBeenCalledWith({ docId: "doc-clamp", page: 4, zoom: 100 });
  });

  it("no marca todas las pestañas como recientes al cambiar el zoom compartido", () => {
    tabs.items = [];
    tabs.activeId = null;
    seedDocument("doc-zoom-a", { inRecent: false, lastOpened: null });
    seedDocument("doc-zoom-b", { inRecent: false, lastOpened: null });
    openDocument("doc-zoom-a");
    openDocument("doc-zoom-b");

    // Simula documentos que están abiertos pero no forman parte de Recientes.
    for (const id of ["doc-zoom-a", "doc-zoom-b"]) {
      const doc = documents.items.get(id)!;
      documents.items.set(id, { ...doc, inRecent: false, lastOpened: null });
    }

    settings.consistentDocZoom = true;
    setConsistentDocZoom(false);

    expect(documents.items.get("doc-zoom-a")?.inRecent).toBe(false);
    expect(documents.items.get("doc-zoom-b")?.inRecent).toBe(false);
    settings.consistentDocZoom = false;
  });
});
