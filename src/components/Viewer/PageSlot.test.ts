import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, waitFor } from "@testing-library/svelte";
import PageSlot from "./PageSlot.svelte";
import { pageKey, viewer } from "$lib/stores/viewer.svelte";
import { documents } from "$lib/stores/documents.svelte";
import { setInvokeMock } from "../../test/setup";
import { PageVisibilityRegistry } from "$lib/pdf/pageVisibility.svelte";
import { settings } from "$lib/stores/settings.svelte";

/** IntersectionObserver que da por visible todo lo que se le pasa. */
class VisibleObserver {
  #callback: IntersectionObserverCallback;
  constructor(callback: IntersectionObserverCallback) {
    this.#callback = callback;
  }
  observe(target: Element): void {
    this.#callback(
      [{ target, isIntersecting: true } as unknown as IntersectionObserverEntry],
      this as unknown as IntersectionObserver,
    );
  }
  unobserve(): void {}
  disconnect(): void {}
}

/** El contexto que en la app monta `Viewer.svelte`. */
function visibilityContext(): Map<unknown, unknown> {
  vi.stubGlobal("IntersectionObserver", VisibleObserver);
  const registry = new PageVisibilityRegistry();
  registry.attach(null);
  return new Map([["pageVisibility", registry]]);
}

const pdfPage = { getViewport: () => ({ width: 595.3, height: 841.9 }) };
const originalCanvasLayout = new Map(
  (["clientWidth", "clientHeight"] as const).map((prop) => [
    prop,
    Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, prop),
  ]),
);

const textLayerMocks = vi.hoisted(() => ({
  created: 0,
  destroyed: 0,
  pending: [] as Array<() => void>,
  hold: false,
}));

// La ruta real la transforma Vite en cada primer import, y eso hace que la
// primera construcción de la capa tarde más que el `waitFor` de las pruebas.
vi.mock("pdfjs-dist/web/pdf_viewer.css", () => ({}));

vi.mock("$lib/pdf/textLayer.svelte", () => ({
  PageTextLayer: class {
    async render(): Promise<void> {
      if (textLayerMocks.hold) {
        await new Promise<void>((resolve) => textLayerMocks.pending.push(resolve));
      }
      textLayerMocks.created += 1;
    }
    destroy(): void {
      textLayerMocks.destroyed += 1;
    }
  },
}));

vi.mock("$lib/pdf/loader", () => ({
  openDocument: vi.fn(async () => ({ getPage: async () => pdfPage })),
  getLoadedDocument: vi.fn(async () => ({ getPage: async () => pdfPage })),
}));

vi.mock("$lib/pdf/renderPage", () => ({
  renderDpr: () => 1,
  getPageCached: vi.fn(async () => pdfPage),
  renderPageToCanvas: vi.fn(() => ({
    promise: Promise.resolve({ cssWidth: 126, cssHeight: 178, pageWidth: 126, pageHeight: 178 }),
    cancel: vi.fn(),
  })),
  releaseCanvas: vi.fn(),
  cleanupPage: vi.fn(async () => undefined),
}));

setInvokeMock("save_document_progress", () => undefined);

function seedDocument(id: string): void {
  documents.items.set(id, {
    id,
    path: `/libros/${id}.pdf`,
    title: `${id}.pdf`,
    kind: "pdf",
    pages: 5,
    sizeBytes: 0,
    hash: null,
    categoryIds: [],
    lastPage: 1,
    lastZoom: 100,
    lastOpened: null,
    inRecent: false,
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
  seedDocument("doc-slot");
  // jsdom no mide nada: sin estas cajas la capa de texto no se construye.
  for (const prop of ["clientWidth", "clientHeight"] as const) {
    Object.defineProperty(HTMLCanvasElement.prototype, prop, {
      configurable: true,
      get: () => (prop === "clientWidth" ? 126 : 178),
    });
  }
  textLayerMocks.created = 0;
  textLayerMocks.destroyed = 0;
  textLayerMocks.pending.length = 0;
  textLayerMocks.hold = false;
  settings.textLayerEnabled = true;
});

afterEach(() => {
  textLayerMocks.pending.length = 0;
  textLayerMocks.hold = false;
  for (const [prop, descriptor] of originalCanvasLayout) {
    if (descriptor) Object.defineProperty(HTMLCanvasElement.prototype, prop, descriptor);
    else delete (HTMLCanvasElement.prototype as unknown as Record<string, unknown>)[prop];
  }
  documents.items.clear();
  viewer.loadedPages.clear();
  viewer.visiblePages.clear();
  viewer.inFlightRenders.clear();
  settings.textLayerEnabled = true;
});

describe("PageSlot", () => {
  it("registra la página al pintarla y la desregistra al desmontar", async () => {
    const context = visibilityContext();
    const { unmount } = render(PageSlot, {
      props: { page: 3, docId: "doc-slot", current: true },
      context,
    });

    await waitFor(() => expect(viewer.loadedPages.has(pageKey("doc-slot", 3))).toBe(true));

    unmount();

    // Sin el `unregisterLoaded` de `onDestroy`, la página se quedaba contada para
    // siempre e inflaba la estimación de memoria al recorrer un documento largo.
    expect(viewer.loadedPages.has(pageKey("doc-slot", 3))).toBe(false);
  });

  it("aísla la misma página cuando pertenece a documentos distintos", async () => {
    const context = visibilityContext();
    seedDocument("doc-other");
    const first = render(PageSlot, {
      props: { page: 3, docId: "doc-slot", current: true },
      context,
    });
    const second = render(PageSlot, {
      props: { page: 3, docId: "doc-other", current: false },
      context,
    });

    await waitFor(() => {
      expect(viewer.loadedPages.has(pageKey("doc-slot", 3))).toBe(true);
      expect(viewer.loadedPages.has(pageKey("doc-other", 3))).toBe(true);
    });
    expect(viewer.loadedPages.size).toBe(2);

    first.unmount();
    await waitFor(() =>
      expect(viewer.loadedPages.has(pageKey("doc-slot", 3))).toBe(false),
    );
    expect(viewer.loadedPages.has(pageKey("doc-other", 3))).toBe(true);

    second.unmount();
  });

  it("solo marca como activa la página recibida como current", async () => {
    const context = visibilityContext();
    const first = render(PageSlot, {
      props: { page: 3, docId: "doc-slot", current: false },
      context,
    });

    expect(first.container.querySelector(".page-slot.current")).toBeNull();
    expect(first.container.querySelector("[aria-current='page']")).toBeNull();

    first.unmount();
    const active = render(PageSlot, {
      props: { page: 3, docId: "doc-slot", current: true },
      context: visibilityContext(),
    });
    expect(active.container.querySelector(".page-slot.current")).not.toBeNull();
    expect(active.container.querySelector("[aria-current='page']")).not.toBeNull();
    active.unmount();
  });

  it("destruye la capa de texto al apagar el ajuste, sin esperar a otro render", async () => {
    const context = visibilityContext();
    render(PageSlot, {
      props: { page: 3, docId: "doc-slot", current: true },
      context,
    });
    await waitFor(() => expect(textLayerMocks.created).toBe(1));

    // El ajuste se apaga desde Ajustes › Avanzado: el contenedor se desmonta,
    // pero la instancia que pdf.js llenó de `span` seguiría viva apuntando a un
    // nodo desprendido hasta el siguiente render de la página.
    settings.textLayerEnabled = false;

    await waitFor(() => expect(textLayerMocks.destroyed).toBe(1));
  });

  it("reconstruye la capa al volver a encender el ajuste", async () => {
    const context = visibilityContext();
    render(PageSlot, {
      props: { page: 3, docId: "doc-slot", current: true },
      context,
    });
    await waitFor(() => expect(textLayerMocks.created).toBe(1));

    settings.textLayerEnabled = false;
    await waitFor(() => expect(textLayerMocks.destroyed).toBe(1));
    settings.textLayerEnabled = true;

    // Sin esta reconstrucción el texto no volvía a ser seleccionable hasta hacer
    // zoom o desplazar la página.
    await waitFor(() => expect(textLayerMocks.created).toBe(2));
  });

  it("no marca sincronizada una capa destruida durante su render", async () => {
    textLayerMocks.hold = true;
    const context = visibilityContext();
    const { unmount } = render(PageSlot, {
      props: { page: 3, docId: "doc-slot", current: true },
      context,
    });

    await waitFor(() => expect(textLayerMocks.pending.length).toBeGreaterThan(0));
    unmount();
    await waitFor(() => expect(textLayerMocks.destroyed).toBe(1));

    for (const resolve of textLayerMocks.pending) resolve();
    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(textLayerMocks.created).toBe(1);
    expect(document.querySelector(".page-text-layer")).toBeNull();
  });
});