import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, waitFor } from "@testing-library/svelte";
import ThumbPanel from "./ThumbPanel.svelte";
import { openDocument } from "$lib/pdf/loader";
import { getPageCached, renderPageToCanvas } from "$lib/pdf/renderPage";
import { documentAspect, setDocumentAspect } from "$lib/pdf/documentAspect.svelte";
import { documents } from "$lib/stores/documents.svelte";
import { setInvokeMock } from "../../test/setup";
import { tabs, updateTabPage } from "$lib/stores/tabs.svelte";
import type { DocumentRecord, Tab } from "$lib/types";

// Página A4 (595.3 × 841.9 pt): proporción 0.707, distinta de la de reserva (0.76).
const PAGE = { getViewport: () => ({ width: 595.3, height: 841.9 }) };
const A4 = () => setDocumentAspect("doc-1", 595.3, 841.9);

vi.mock("$lib/pdf/loader", () => ({
  openDocument: vi.fn(async () => ({ numPages: 2 })),
}));

// El progreso se guarda con temporizador: sin mock, la promesa se rechaza
// después de que el entorno de test se haya cerrado.
setInvokeMock("save_document_progress", () => undefined);

vi.mock("$lib/pdf/renderPage", () => ({
  getPageCached: vi.fn(async () => PAGE),
  renderPageToCanvas: vi.fn(() => ({
    promise: Promise.resolve({ cssWidth: 126, cssHeight: 178, pageWidth: 126, pageHeight: 178 }),
    cancel: vi.fn(),
  })),
  releaseCanvas: vi.fn(),
  cleanupPage: vi.fn(),
}));

/** Observador que da por visible todo lo que se le pasa. */
class VisibleObserver {
  #callback: (entries: unknown[]) => void;
  constructor(callback: (entries: unknown[]) => void) {
    this.#callback = callback;
  }
  observe(target: Element): void {
    this.#callback([{ target, isIntersecting: true }]);
  }
  unobserve(): void {}
  disconnect(): void {}
}

// Las miniaturas solo se renderizan cuando el observador las da por visibles.
vi.stubGlobal("IntersectionObserver", VisibleObserver);

function makeDoc(overrides: Partial<DocumentRecord> = {}): DocumentRecord {
  return {
    id: "doc-1",
    path: "/tmp/book.pdf",
    title: "book.pdf",
    kind: "pdf",
    pages: 500,
    sizeBytes: 1024,
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
    ...overrides,
  };
}

const ROW_HEIGHT = 198;
const ROW_PITCH = ROW_HEIGHT + 10;
const COLUMN_WIDTH = 140;
const originalElementLayout = new Map(
  (["clientWidth", "clientHeight"] as const).map((prop) => [
    prop,
    Object.getOwnPropertyDescriptor(HTMLElement.prototype, prop),
  ]),
);
const originalElementRect = Object.getOwnPropertyDescriptor(
  HTMLElement.prototype,
  "getBoundingClientRect",
);

/** jsdom no calcula rejillas ni mide cajas: se simulan celdas de 140 px. */
const extraNodes: HTMLElement[] = [];
let scrollCalls: { top: number; behavior?: ScrollBehavior }[] = [];

function stubLayout(): void {
  const realStyle = window.getComputedStyle.bind(window);
  vi.spyOn(window, "getComputedStyle").mockImplementation((element, pseudo) => {
    const style = realStyle(element, pseudo);
    if (element instanceof HTMLElement && element.classList.contains("thumb-grid")) {
      const patched = Object.create(style) as CSSStyleDeclaration;
      Object.defineProperty(patched, "gridTemplateColumns", {
        value: `${COLUMN_WIDTH}px ${COLUMN_WIDTH}px`,
      });
      return patched;
    }
    return style;
  });

  const rect = (top: number, width = 0, height = 0) =>
    ({ top, bottom: top + height, left: 0, right: width, width, height, x: 0, y: top, toJSON: () => ({}) }) as DOMRect;
  const scrollRootScrollTop = () =>
    document.getElementById("inspectorContent")?.scrollTop ?? 0;
  HTMLElement.prototype.getBoundingClientRect = function (this: HTMLElement) {
    // La rejilla sube con el scroll, como en el navegador; el resto, fijo.
    if (this.classList.contains("thumb-grid")) return rect(-scrollRootScrollTop());
    if (this.classList.contains("thumb-spacer")) return rect(0, 2 * COLUMN_WIDTH + 10, ROW_HEIGHT);
    if (this.classList.contains("thumb-card")) return rect(0, COLUMN_WIDTH, ROW_HEIGHT);
    return rect(0);
  };

  for (const prop of ["clientWidth", "clientHeight"] as const) {
    Object.defineProperty(HTMLElement.prototype, prop, {
      configurable: true,
      get(this: HTMLElement) {
        if (this.classList.contains("thumb-card")) {
          return prop === "clientWidth" ? COLUMN_WIDTH : ROW_HEIGHT - 1;
        }
        if (this.id === "inspectorContent") return prop === "clientWidth" ? 300 : 600;
        return 0;
      },
    });
  }
}

function mountScrollRoot(): HTMLElement {
  const root = document.createElement("div");
  root.id = "inspectorContent";
  // jsdom no implementa scrollTo: se aplica el destino y se anota el modo.
  root.scrollTo = ((options: ScrollToOptions) => {
    if (typeof options?.top === "number") {
      scrollCalls.push({ top: options.top, behavior: options.behavior });
      root.scrollTop = options.top;
    }
  }) as never;
  document.body.append(root);
  extraNodes.push(root);
  return root;
}

/** Abre una pestaña con el documento dado en el store real. */
function openTab(page: number, doc = makeDoc()): void {
  documents.items.set(doc.id, doc);
  const tab: Tab = {
    id: "tab-1",
    docId: doc.id,
    page,
    zoom: 100,
    status: "active",
    lastActive: Date.now(),
    _restoreToken: 0,
  };
  tabs.items = [tab];
  tabs.activeId = tab.id;
}

/** ¿Queda la fila de `page` entera dentro de la ventana del panel? */
function rowIsVisible(root: HTMLElement, page: number): boolean {
  const rowTop = Math.floor((page - 1) / 2) * ROW_PITCH;
  return root.scrollTop <= rowTop && root.scrollTop + 600 >= rowTop + ROW_HEIGHT;
}

/** ¿Está centrada en la ventana la fila de `page`? */
function rowIsCentered(root: HTMLElement, page: number): boolean {
  const rowTop = Math.floor((page - 1) / 2) * ROW_PITCH;
  const expected = Math.max(0, rowTop - (600 - ROW_HEIGHT) / 2);
  return Math.abs(root.scrollTop - expected) <= 1;
}

describe("ThumbPanel", () => {
  beforeEach(() => {
    vi.mocked(openDocument).mockClear();
    vi.mocked(getPageCached).mockClear();
    vi.mocked(renderPageToCanvas).mockClear();
    stubLayout();
    scrollCalls = [];
    openTab(1);
    A4();
  });

  afterEach(() => {
    if (originalElementRect) {
      Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", originalElementRect);
    }
    for (const [prop, descriptor] of originalElementLayout) {
      if (descriptor) Object.defineProperty(HTMLElement.prototype, prop, descriptor);
      else delete (HTMLElement.prototype as unknown as Record<string, unknown>)[prop];
    }
    for (const node of extraNodes) node.remove();
    extraNodes.length = 0;
    tabs.items = [];
    tabs.activeId = null;
    documents.items.clear();
    documentAspect.clear();
    vi.restoreAllMocks();
  });

  it("ajusta el alto de fila a la proporción precargada del documento", async () => {
    render(ThumbPanel, { props: { doc: makeDoc() } });

    const grid = document.querySelector(".thumb-grid") as HTMLElement;
    // 140 px de columna / 0.707 ≈ 198 px (con la proporción de reserva serían
    // 184 px y la hoja quedaría con una forma que no es la suya).
    await waitFor(() =>
      expect(grid.style.getPropertyValue("--thumb-row-height")).toBe("198px"),
    );
    // (Las llamadas a `getPageCached` que hay son de las tarjetas al pintarse:
    // la medida la trae el cargador, el panel no abre ninguna página.)
  });

  it("no mide el espaciador superior como si fuera una celda", async () => {
    openTab(401);
    mountScrollRoot();
    const doc = makeDoc();
    render(ThumbPanel, { props: { doc } });

    const grid = document.querySelector(".thumb-grid") as HTMLElement;
    await waitFor(() => expect(grid.querySelector(".thumb-spacer")).not.toBeNull());
    await waitFor(() =>
      expect(grid.style.getPropertyValue("--thumb-row-height")).toBe("198px"),
    );
  });

  it("mide en cuanto llega la proporción si aún no estaba", async () => {
    documentAspect.clear();
    render(ThumbPanel, { props: { doc: makeDoc() } });

    const grid = document.querySelector(".thumb-grid") as HTMLElement;
    expect(grid.style.getPropertyValue("--thumb-row-height")).toBe("160px");

    A4();
    await waitFor(() =>
      expect(grid.style.getPropertyValue("--thumb-row-height")).toBe("198px"),
    );
  });

  it("dimensiona la hoja con la proporción real de la página", async () => {
    render(ThumbPanel, { props: { doc: makeDoc() } });

    // 140 de celda − 14 de marco = 126 de ancho; 126 / 0.707 ≈ 178 de alto.
    await waitFor(() => {
      const sheet = document.querySelector(".thumb-sheet") as HTMLElement;
      expect(sheet.style.getPropertyValue("--sheet-w")).toBe("126px");
      expect(sheet.style.getPropertyValue("--sheet-h")).toBe("178px");
    });
  });

  it("mide con la proporción de la página cuando el documento es apaisado", async () => {
    setDocumentAspect("doc-1", 841.9, 595.3);
    render(ThumbPanel, { props: { doc: makeDoc() } });

    const grid = document.querySelector(".thumb-grid") as HTMLElement;
    await waitFor(() =>
      expect(grid.style.getPropertyValue("--thumb-row-height")).toBe("99px"),
    );
  });

  it("se abre con la página actual a la vista", async () => {
    openTab(37);
    const root = mountScrollRoot();
    const doc = makeDoc();
    render(ThumbPanel, { props: { doc } });

    await waitFor(() => expect(rowIsVisible(root, 37)).toBe(true));
    expect(root.scrollTop).toBeGreaterThan(0);
  });

  it("sigue a la página actual cuando el visor avanza", async () => {
    const root = mountScrollRoot();
    const doc = makeDoc();
    render(ThumbPanel, { props: { doc } });
    await waitFor(() => expect(rowIsVisible(root, 1)).toBe(true));

    // El visor avanza hasta la página 121 (fila 60).
    updateTabPage("tab-1", 121, 100);

    await waitFor(() => expect(rowIsCentered(root, 121)).toBe(true));
  });

  it("no recoloca la lista mientras el usuario la está navegando", async () => {
    const root = mountScrollRoot();
    const doc = makeDoc();
    render(ThumbPanel, { props: { doc } });
    await waitFor(() => expect(rowIsVisible(root, 1)).toBe(true));

    // El usuario empieza a navegar la lista por su cuenta.
    const grid = document.querySelector(".thumb-grid") as HTMLElement;
    grid.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    const before = scrollCalls.length;

    // El visor avanza mientras el gesto está en curso: la lista no se le mueve.
    updateTabPage("tab-1", 121, 100);
    await new Promise((resolve) => setTimeout(resolve, 40));

    expect(scrollCalls.length).toBe(before);
    expect(rowIsVisible(root, 1)).toBe(true);
  });

  it("acompaña con el mismo desplazamiento: una fila por página", async () => {
    // Lejos del principio, donde el centrado no queda recortado por el tope.
    openTab(201);
    const root = mountScrollRoot();
    const doc = makeDoc();
    render(ThumbPanel, { props: { doc } });
    await waitFor(() => expect(rowIsCentered(root, 201)).toBe(true));
    const before = root.scrollTop;

    // De la página 201 a la 203 hay exactamente una fila de miniaturas.
    updateTabPage("tab-1", 203, 100);

    await waitFor(() => expect(rowIsCentered(root, 203)).toBe(true));
    expect(root.scrollTop - before).toBe(ROW_PITCH);
  });

  it("acompaña al visor con un desplazamiento suave, no de golpe", async () => {
    const root = mountScrollRoot();
    const doc = makeDoc();
    render(ThumbPanel, { props: { doc } });
    await waitFor(() => expect(rowIsVisible(root, 1)).toBe(true));
    const before = scrollCalls.length;

    updateTabPage("tab-1", 121, 100);

    await waitFor(() => expect(scrollCalls.length).toBeGreaterThan(before));
    const last = scrollCalls.at(-1);
    expect(last?.behavior).toBe("smooth");
    // Centra la fila de la página 121 (fila 60), no la deja al borde.
    expect(last?.top).toBe(60 * ROW_PITCH - (600 - ROW_HEIGHT) / 2);
  });

  it("no mueve el scroll si la página cae en la misma fila", async () => {
    openTab(11);
    const root = mountScrollRoot();
    const doc = makeDoc();
    render(ThumbPanel, { props: { doc } });
    await waitFor(() => expect(rowIsCentered(root, 11)).toBe(true));
    const before = root.scrollTop;

    // La página 12 vive en la misma fila: la posición centrada es la misma.
    updateTabPage("tab-1", 12, 100);
    await new Promise((resolve) => setTimeout(resolve, 30));

    expect(root.scrollTop).toBe(before);
  });

  it("no vuelve a centrar la página actual si el usuario mueve la lista", async () => {
    const root = mountScrollRoot();
    const doc = makeDoc();
    render(ThumbPanel, { props: { doc } });
    await waitFor(() => expect(rowIsVisible(root, 1)).toBe(true));
    scrollCalls.length = 0;

    // El usuario se aleja de la página actual hacia el principio de la lista.
    root.scrollTop = 0;
    root.dispatchEvent(new Event("scroll"));
    await new Promise((resolve) => setTimeout(resolve, 40));

    // La lista no debe haber saltado de vuelta a la página actual.
    expect(scrollCalls.length).toBe(0);
  });

  it("no vuelve a renderizar las miniaturas cuando solo cambia el progreso", async () => {
    mountScrollRoot();
    const doc = makeDoc({ pages: 6 });
    const { rerender } = render(ThumbPanel, { props: { doc } });

    await waitFor(() => expect(vi.mocked(renderPageToCanvas).mock.calls.length).toBeGreaterThan(0));
    const renders = vi.mocked(renderPageToCanvas).mock.calls.length;
    const firstCard = document.querySelector(".thumb-card");

    // El visor guarda el progreso sustituyendo el objeto del documento.
    await rerender({ doc: makeDoc({ pages: 6, lastPage: 4, lastZoom: 150 }) });
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(vi.mocked(renderPageToCanvas).mock.calls.length).toBe(renders);
    expect(document.querySelector(".thumb-card")).toBe(firstCard);
  });
});
