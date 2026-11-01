import { beforeEach, describe, expect, it, vi } from "vitest";

const getPageCached = vi.fn();
const destroy = vi.fn().mockResolvedValue(undefined);
// `vi.hoisted`: el mock de `pdfjs-dist` se evalúa al importar `rangeTransport.ts`
// (que ahora importa la clase base), antes de que corra este cuerpo.
const getDocument = vi.hoisted(() => vi.fn());

vi.mock("pdfjs-dist", () => ({
  GlobalWorkerOptions: {},
  getDocument,
  // `rangeTransport.ts` extiende esta clase; pdf.js exige `instanceof`, así que
  // el doble tiene que existir aunque el mock no lo use.
  PDFDataRangeTransport: class {
    length: number;
    constructor(length: number) {
      this.length = length;
    }
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `lyra-pdf://${path}`,
}));

// El mock local de `@tauri-apps/api/core` sustituye el global (que sí trae
// `invoke`), así que la orden de longitud se dobla aquí.
vi.mock("$lib/tauri/commands", () => ({
  pdfFileLength: vi.fn(async () => 1234),
  readPdfRange: vi.fn(async () => new ArrayBuffer(0)),
}));

vi.mock("./renderPage", () => ({
  getPageCached,
  cleanupAllPages: vi.fn().mockResolvedValue(undefined),
}));

import { openDocument, closeDocument } from "./loader";
import { documentAspect } from "./documentAspect.svelte";
import { isDocumentLoaded, markDocumentLoaded, loadedDocuments } from "./documentState.svelte";
import { pdfFileLength } from "$lib/tauri/commands";

/** Deja que se resuelvan las microtareas y el `import()` dinámico. */
async function flush(): Promise<void> {
  for (let index = 0; index < 5; index += 1) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("preloadDocumentAspect", () => {
  beforeEach(() => {
    documentAspect.clear();
    getPageCached.mockReset();
    destroy.mockClear();
    getDocument.mockReset();
    getDocument.mockReturnValue({
      promise: Promise.resolve({
        numPages: 3,
        destroy,
      }),
    });
  });

  it("mide la página 1 y guarda la proporción", async () => {
    getPageCached.mockResolvedValue({
      getViewport: () => ({ width: 600, height: 800 }),
    });

    await openDocument("doc-open", "/libros/libro.pdf");
    await flush();

    expect(getPageCached).toHaveBeenCalledTimes(1);
    expect(documentAspect.get("doc-open")).toBeCloseTo(0.75);

    await closeDocument("doc-open");
  });

  it("no mide ni guarda nada si el documento se cerró mientras se medía", async () => {
    let releasePage = (): void => {};
    getPageCached.mockReturnValue(
      new Promise((resolve) => {
        releasePage = () => resolve({ getViewport: () => ({ width: 600, height: 800 }) });
      }),
    );

    await openDocument("doc-closed", "/libros/libro.pdf");
    await flush();
    expect(getPageCached).toHaveBeenCalledTimes(1);

    // El usuario cierra el documento antes de que termine la medición.
    await closeDocument("doc-closed");
    releasePage();
    await flush();

    expect(documentAspect.has("doc-closed")).toBe(false);
  });

  it("no mide si el documento se cerró antes de empezar", async () => {
    getPageCached.mockResolvedValue({
      getViewport: () => ({ width: 600, height: 800 }),
    });

    const opening = openDocument("doc-race", "/libros/libro.pdf");
    await closeDocument("doc-race");
    await opening;
    await flush();

    expect(documentAspect.has("doc-race")).toBe(false);
  });
});

describe("origen del documento", () => {
  it("pide rangos al disco cuando conoce la longitud", async () => {
    vi.mocked(pdfFileLength).mockResolvedValueOnce(1234);
    getDocument.mockClear();

    await openDocument("doc-rango", "/libros/grande.pdf");

    const source = getDocument.mock.calls.at(-1)?.[0] as { url?: string; range?: unknown };
    expect(source.url).toBeUndefined();
    expect(source.range).toBeDefined();
    expect((source.range as { length: number }).length).toBe(1234);
  });

  it("cae a la URL del protocolo si no puede leer la longitud", async () => {
    vi.mocked(pdfFileLength).mockRejectedValueOnce(new Error("no registrado"));
    getDocument.mockClear();

    await openDocument("doc-url", "/libros/ajeno.pdf");

    const source = getDocument.mock.calls.at(-1)?.[0] as { url?: string; range?: unknown };
    expect(source.range).toBeUndefined();
    expect(source.url).toBe("lyra-pdf:///libros/ajeno.pdf");
  });
});

describe("apertura concurrente", () => {
  beforeEach(() => {
    getDocument.mockReset();
  });

  it("comparte la promesa mientras el documento todavía está cargando", async () => {
    const pdf = { numPages: 3, destroy };
    getDocument.mockReturnValue({ promise: Promise.resolve(pdf) });

    const first = openDocument("doc-concurrente", "/libros/concurrente.pdf");
    const second = openDocument("doc-concurrente", "/libros/concurrente.pdf");
    const [openedFirst, openedSecond] = await Promise.all([first, second]);

    expect(getDocument).toHaveBeenCalledTimes(1);
    expect(openedFirst).toBe(pdf);
    expect(openedSecond).toBe(pdf);
    await closeDocument("doc-concurrente");
  });
});

describe("documentos cargados", () => {
  beforeEach(() => {
    loadedDocuments.clear();
    documentAspect.clear();
    getPageCached.mockReset();
    getPageCached.mockResolvedValue({
      getViewport: () => ({ width: 600, height: 800 }),
    });
    destroy.mockClear();
    getDocument.mockReset();
    getDocument.mockReturnValue({
      promise: Promise.resolve({ numPages: 3, destroy }),
    });
  });

  // Sin desmarcar al cerrar, `isDocumentLoaded` seguía siendo true y la espera
  // de restauración de `tabs` daba la pestaña por lista antes de recargar el PDF.
  it("deja de contar un documento al cerrarlo", async () => {
    await openDocument("doc-cierra", "/libros/libro.pdf");
    markDocumentLoaded("doc-cierra");
    expect(isDocumentLoaded("doc-cierra")).toBe(true);

    await closeDocument("doc-cierra");
    await flush();

    expect(isDocumentLoaded("doc-cierra")).toBe(false);
  });

  it("deja de contar el documento que expulsa el LRU", async () => {
    for (let index = 0; index < 6; index += 1) {
      await openDocument(`doc-${index}`, `/libros/libro-${index}.pdf`);
      markDocumentLoaded(`doc-${index}`);
    }
    await flush();

    // MAX_OPEN_DOCS es 5, así que el primero (el menos usado) ya no está abierto.
    expect(isDocumentLoaded("doc-0")).toBe(false);
    expect(isDocumentLoaded("doc-5")).toBe(true);
  });
});
