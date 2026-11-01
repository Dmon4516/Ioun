import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";
import { isPageProtected, unprotectPage } from "./protectedPages";

export interface RenderedInfo {
  cssWidth: number;
  cssHeight: number;
  pageWidth: number;
  pageHeight: number;
}

const MAX_PAGES_PER_DOC = 80;

/**
 * Factor de resolución de los canvas, acotado a 2. Lo comparten el render de
 * `PageSlot` y la estimación de memoria: el lienzo guarda `ancho * dpr` por
 * `alto * dpr` píxeles, así que ignorarlo subestima la memoria por `dpr²`.
 */
export function renderDpr(): number {
  return Math.min(window.devicePixelRatio || 1, 2);
}

interface PageEntry {
  promise: Promise<PDFPageProxy>;
  lastUsed: number;
}

const pageCache = new WeakMap<PDFDocumentProxy, Map<number, PageEntry>>();
let pageAccessCounter = 0;

function getPage(
  pdf: PDFDocumentProxy,
  pageNumber: number,
  docId: string,
): Promise<PDFPageProxy> {
  let map = pageCache.get(pdf);
  if (!map) {
    map = new Map();
    pageCache.set(pdf, map);
  }

  const existing = map.get(pageNumber);
  if (existing) {
    existing.lastUsed = ++pageAccessCounter;
    return existing.promise;
  }

  const promise = pdf.getPage(pageNumber);
  map.set(pageNumber, { promise, lastUsed: ++pageAccessCounter });
  void promise.catch(() => {
    const entry = map?.get(pageNumber);
    if (entry?.promise === promise) map?.delete(pageNumber);
  });

  while (map.size > MAX_PAGES_PER_DOC) {
    let oldest: number | null = null;
    let oldestTime = Infinity;
    for (const [num, entry] of map) {
      // Las páginas visibles o en render no se expulsan. Si todas lo están, el
      // bucle sale sin candidato: el caché crece por encima del límite de forma
      // temporal, que es preferible a invalidar recursos en uso.
      if (isPageProtected(docId, num)) continue;
      if (entry.lastUsed < oldestTime) {
        oldestTime = entry.lastUsed;
        oldest = num;
      }
    }
    if (oldest === null) break;
    const victim = map.get(oldest);
    map.delete(oldest);
    void victim?.promise.then((page) => page.cleanup()).catch(() => undefined);
  }

  return promise;
}

export function getPageCached(
  pdf: PDFDocumentProxy,
  pageNumber: number,
  docId: string,
): Promise<PDFPageProxy> {
  return getPage(pdf, pageNumber, docId);
}

export interface RenderHandle {
  promise: Promise<RenderedInfo>;
  cancel: () => void;
}

export function renderPageToCanvas(
  pdf: PDFDocumentProxy,
  pageNumber: number,
  canvas: HTMLCanvasElement,
  scale: number,
  dpr: number,
  docId: string,
): RenderHandle {
  let cancelled = false;
  let renderTask: { cancel: () => void; promise: Promise<unknown> } | null = null;

  const promise = (async () => {
    const page = await getPage(pdf, pageNumber, docId);
    if (cancelled) throw new Error("Render cancelled");

    const cssViewport = page.getViewport({ scale });
    const maxCanvasEdge = 8192;
    const maxCanvasPixels = 32_000_000;
    const maxDprByEdge = maxCanvasEdge / Math.max(cssViewport.width, cssViewport.height);
    const maxDprByArea = Math.sqrt(maxCanvasPixels / (cssViewport.width * cssViewport.height));
    // El DPR real del render es el menor entre el de pantalla y los límites del canvas.
    const effectiveDpr = Math.min(dpr, maxDprByEdge, maxDprByArea);
    const pxViewport = page.getViewport({ scale: scale * effectiveDpr });

    const pxWidth = Math.max(1, Math.ceil(pxViewport.width));
    const pxHeight = Math.max(1, Math.ceil(pxViewport.height));

    canvas.style.removeProperty("width");
    canvas.style.removeProperty("height");
    if (canvas.width !== pxWidth) canvas.width = pxWidth;
    if (canvas.height !== pxHeight) canvas.height = pxHeight;

    const ctx = canvas.getContext("2d", { alpha: false });
    if (!ctx) throw new Error("[renderPage] no 2d context");

    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, pxWidth, pxHeight);
    if (cancelled) throw new Error("Render cancelled");

    renderTask = page.render({ canvasContext: ctx, viewport: pxViewport });
    await renderTask.promise;

    return {
      cssWidth: cssViewport.width,
      cssHeight: cssViewport.height,
      pageWidth: pxWidth,
      pageHeight: pxHeight,
    };
  })();

  return {
    promise,
    cancel() {
      cancelled = true;
      try { renderTask?.cancel(); } catch { /* ya cancelado */ }
    },
  };
}

export function releaseCanvas(canvas: HTMLCanvasElement): void {
  canvas.width = 1;
  canvas.height = 1;
  canvas.style.removeProperty("width");
  canvas.style.removeProperty("height");
}

export async function cleanupPage(
  pdf: PDFDocumentProxy,
  pageNumber: number,
  docId: string,
): Promise<void> {
  const map = pageCache.get(pdf);
  if (!map) return;
  const entry = map.get(pageNumber);
  if (!entry) return;
  map.delete(pageNumber);
  unprotectPage(docId, pageNumber);
  try {
    const page = await entry.promise;
    if (isPageProtected(docId, pageNumber)) {
      // La página volvió a ser visible mientras esperaba getPage(); no se debe
      // limpiar un recurso que acaba de quedar protegido por otro slot.
      map.set(pageNumber, entry);
      return;
    }
    page.cleanup();
  } catch {
    // La página ya se había liberado o el documento se cerró.
  }
}

export async function cleanupAllPages(pdf: PDFDocumentProxy): Promise<void> {
  const map = pageCache.get(pdf);
  if (!map) return;
  const pages = [...map.values()];
  map.clear();
  await Promise.allSettled(
    pages.map((entry) =>
      entry.promise.then((page) => page.cleanup()).catch(() => undefined),
    ),
  );
}
