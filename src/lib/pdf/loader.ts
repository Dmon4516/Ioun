import type { PDFDataRangeTransport, PDFDocumentProxy } from "pdfjs-dist";
import { convertFileSrc } from "@tauri-apps/api/core";
import { settings } from "$lib/stores/settings.svelte";
import { pdfFileLength } from "$lib/tauri/commands";
import { createRangeTransport } from "./rangeTransport";
import { setDocumentAspect } from "./documentAspect.svelte";
import { unmarkDocumentLoaded } from "./documentState.svelte";

let pdfjsModule: typeof import("pdfjs-dist") | null = null;
let pdfjsPromise: Promise<typeof import("pdfjs-dist")> | null = null;

async function loadPdfJs(): Promise<typeof import("pdfjs-dist")> {
  if (pdfjsModule) return pdfjsModule;
  if (!pdfjsPromise) {
    pdfjsPromise = import("pdfjs-dist").then((mod) => {
      mod.GlobalWorkerOptions.workerSrc = new URL(
        "pdfjs-dist/build/pdf.worker.min.mjs",
        import.meta.url,
      ).toString();
      pdfjsModule = mod;
      return mod;
    }).catch((error) => {
      pdfjsPromise = null;
      throw error;
    });
  }
  return pdfjsPromise;
}

interface CachedDoc {
  path: string;
  promise: Promise<PDFDocumentProxy>;
  lastUsed: number;
  ready: boolean;
}

const DEFAULT_MAX_OPEN_DOCS = 5;

/**
 * Cuántos documentos se mantienen abiertos a la vez. Se sigue de
 * `settings.maxLoadedTabs` para que el LRU y el límite de pestañas cargadas no se
 * contradigan: con un `maxLoadedTabs` mayor, el LRU expulsaba documentos cuyas
 * pestañas seguían «cargadas» y había que reabrirlos en el siguiente render.
 */
function maxOpenDocs(): number {
  const configured = settings.maxLoadedTabs;
  if (!Number.isFinite(configured) || configured <= 0) return DEFAULT_MAX_OPEN_DOCS;
  return Math.max(2, Math.round(configured));
}
const openDocs = new Map<string, CachedDoc>();
let accessCounter = 0;

function destroyQuietly(promise: Promise<PDFDocumentProxy>): void {
  void promise.then((pdf) => pdf.destroy()).catch(() => undefined);
}

function evictIfNeeded(): void {
  while (openDocs.size > maxOpenDocs()) {
    let oldestId: string | null = null;
    let oldest = Infinity;
    for (const [id, entry] of openDocs) {
      // Never evict an in-flight load. Keeping it in the cache is what makes
      // concurrent callers share the same promise instead of opening twice.
      if (!entry.ready) continue;
      if (entry.lastUsed < oldest) {
        oldest = entry.lastUsed;
        oldestId = id;
      }
    }
    if (!oldestId) return;
    const entry = openDocs.get(oldestId);
    openDocs.delete(oldestId);
    // El documento sale del LRU: si se quedara marcado como cargado, la espera
    // de restauración lo daría por listo sin tener el PDF abierto.
    unmarkDocumentLoaded(oldestId);
    if (entry) destroyQuietly(entry.promise);
  }
}

/**
 * Cómo pedir el PDF a pdf.js.
 *
 * Con el esquema propio (`lyra-pdf://`) pdf.js no usa `Range` y se trae el
 * archivo entero, así que se prefiere el transporte por rangos sobre el disco.
 * Si no se puede conocer la longitud (por ejemplo si el documento ya no está
 * registrado), se cae a la URL del protocolo, que sigue funcionando aunque
 * materialice el archivo completo.
 */
async function documentSource(
  pdfjs: typeof import("pdfjs-dist"),
  path: string,
): Promise<{ url: string } | { range: PDFDataRangeTransport }> {
  try {
    const length = await pdfFileLength(path);
    if (length > 0) {
      return { range: createRangeTransport(pdfjs.PDFDataRangeTransport, path, length) };
    }
    // Sin longitud (archivo vacío o ya no registrado) no hay rangos posibles.
    console.warn(`[pdf] ${path} sin longitud legible; se sirve por protocolo`);
  } catch (error) {
    console.error(`[pdf] Sin lectura por rangos para ${path}; se sirve por protocolo`, error);
  }
  return { url: convertFileSrc(path, "lyra-pdf") };
}

/**
 * Carga el módulo de pdf.js (y su chunk) sin abrir ningún documento. Sirve para
 * precalentarlo al arrancar el visor y para el smoke test de extremo a extremo.
 */
export function warmUpPdfJs(): Promise<typeof import("pdfjs-dist")> {
  return loadPdfJs();
}

export async function openDocument(docId: string, path: string): Promise<PDFDocumentProxy> {
  const existing = openDocs.get(docId);
  if (existing && existing.path === path) {
    existing.lastUsed = ++accessCounter;
    return existing.promise;
  }
  if (existing) {
    openDocs.delete(docId);
    destroyQuietly(existing.promise);
  }

  const promise = (async () => {
    const pdfjs = await loadPdfJs();
    const pdf = await pdfjs.getDocument({
      ...(await documentSource(pdfjs, path)),
      isEvalSupported: false,
      useSystemFonts: true,
      disableRange: false,
      disableStream: false,
      disableAutoFetch: true,
      rangeChunkSize: 64 * 1024,
    }).promise;

    queueMicrotask(() => {
      void syncPageCountWithBackend(docId, pdf.numPages);
      void preloadDocumentAspect(docId, pdf, promise);
    });

    return pdf;
  })();

  const entry: CachedDoc = { path, promise, lastUsed: ++accessCounter, ready: false };
  openDocs.set(docId, entry);
  void promise.then(
    () => {
      if (openDocs.get(docId)?.promise !== promise) return;
      entry.ready = true;
      evictIfNeeded();
    },
    () => undefined,
  );
  evictIfNeeded();

  try {
    return await promise;
  } catch (err) {
    const current = openDocs.get(docId);
    if (current?.promise === promise) openDocs.delete(docId);
    throw err;
  }
}

/**
 * Mide la proporción de la página 1 en cuanto se abre el documento. El visor y
 * las miniaturas la comparten, así que la colocación inicial ya es la correcta
 * y ninguna de las dos tiene que abrir una página por su cuenta.
 */
async function preloadDocumentAspect(
  docId: string,
  pdf: PDFDocumentProxy,
  promise: Promise<PDFDocumentProxy>,
): Promise<void> {
  // Si el documento se cierra (o lo expulsa el LRU) mientras se mide, `pdf` ya
  // no es el vigente: medir sobre un documento destruido solo deja ruido en la
  // consola y una entrada de un documento que ya no está abierto.
  const isCurrent = (): boolean => openDocs.get(docId)?.promise === promise;
  if (!isCurrent()) return;

  try {
    const { getPageCached } = await import("./renderPage");
    const page = await getPageCached(pdf, 1, docId);
    if (!isCurrent()) return;
    const viewport = page.getViewport({ scale: 1 });
    setDocumentAspect(docId, viewport.width, viewport.height);
  } catch (error) {
    if (isCurrent()) console.error(`[pdf] No se pudo medir el formato de ${docId}`, error);
  }
}

async function syncPageCountWithBackend(docId: string, pages: number): Promise<void> {
  if (!Number.isFinite(pages) || pages <= 0) return;
  const { documents } = await import("$lib/stores/documents.svelte");
  const doc = documents.items.get(docId);
  if (!doc) return;
  if (doc.pages === pages) {
    const { clampPagesForDocument } = await import("$lib/stores/tabs.svelte");
    clampPagesForDocument(docId, pages);
    return;
  }

  // Persistir primero. Si la BD falla, no tocamos el store.
  const { updateDocumentPages } = await import("$lib/tauri/commands");
  try {
    await updateDocumentPages(docId, pages);
  } catch (error) {
    console.error("[pdf] No se pudo persistir el número de páginas", error);
    return;
  }
  const current = documents.items.get(docId);
  if (!current || current.pages === pages) return;
  const clampedLastPage = Math.min(
    Math.max(Number.isFinite(current.lastPage) ? Math.round(current.lastPage) : 1, 1),
    pages,
  );
  documents.items.set(docId, { ...current, pages, lastPage: clampedLastPage });
  if (current.lastPage !== clampedLastPage) {
    const { saveDocumentProgress } = await import("$lib/tauri/commands");
    void saveDocumentProgress(docId, clampedLastPage, current.lastZoom).catch((error: unknown) => {
      console.error("[pdf] No se pudo acotar la última página", error);
    });
  }
  const { clampPagesForDocument } = await import("$lib/stores/tabs.svelte");
  clampPagesForDocument(docId, pages);
}

export async function closeDocument(docId: string): Promise<void> {
  const entry = openDocs.get(docId);
  if (!entry) return;
  openDocs.delete(docId);
  unmarkDocumentLoaded(docId);
  try {
    const pdf = await entry.promise;
    try {
      const { cleanupAllPages } = await import("./renderPage");
      const { clearProtectedPages } = await import("./protectedPages");
      clearProtectedPages(docId);
      await cleanupAllPages(pdf);
    } catch (error) {
      // A failed lazy chunk must not prevent the underlying PDF proxy from
      // being released.
      console.error(`[pdf] No se pudieron limpiar las páginas de ${docId}`, error);
    } finally {
      await pdf.destroy();
    }
  } catch (error) {
    console.error(`[pdf] No se pudo liberar el documento ${docId}`, error);
  }
}

export async function getLoadedDocument(docId: string): Promise<PDFDocumentProxy | null> {
  const entry = openDocs.get(docId);
  if (!entry) return null;
  entry.lastUsed = ++accessCounter;
  try {
    return await entry.promise;
  } catch {
    return null;
  }
}
