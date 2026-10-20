import { SvelteSet } from "svelte/reactivity";

/**
 * Documentos que ya han pintado alguna página.
 *
 * Vive en un módulo hoja a propósito: así `loader.ts` puede marcar y desmarcar
 * documentos al abrirlos, cerrarlos o expulsarlos del LRU sin crear un ciclo de
 * imports con los stores (`stores/viewer.svelte.ts` importa `tabs`, que importa
 * `loader`).
 */
export const loadedDocuments = new SvelteSet<string>();

export function markDocumentLoaded(docId: string): void {
  loadedDocuments.add(docId);
  settleWaiters(docId);
}

/** Al cerrar o hibernar un documento hay que desmarcarlo: si se queda marcado,
 *  una restauración de pestaña se da por lista antes de que el PDF vuelva a
 *  cargarse. */
export function unmarkDocumentLoaded(docId: string): void {
  loadedDocuments.delete(docId);
}

export function isDocumentLoaded(docId: string): boolean {
  return loadedDocuments.has(docId);
}

interface Waiter {
  resolve: () => void;
  timer: ReturnType<typeof setTimeout>;
}

/**
 * Esperas de "documento cargado". Permiten que quien hiberna y restaura una
 * pestaña espere al evento real en lugar de sondear `isDocumentLoaded` cada
 * pocos milisegundos.
 */
const waiters = new Map<string, Set<Waiter>>();

function settleWaiters(docId: string): void {
  const pending = waiters.get(docId);
  if (!pending) return;
  waiters.delete(docId);
  for (const waiter of pending) {
    clearTimeout(waiter.timer);
    waiter.resolve();
  }
}

/**
 * Se resuelve cuando el documento pinta su primera página, o cuando vence
 * `timeoutMs` (nunca se queda colgada). El temporizador retira la espera, así que
 * un documento que no llega a cargarse no deja residuos.
 */
export function whenDocumentLoaded(docId: string, timeoutMs: number): Promise<void> {
  if (loadedDocuments.has(docId)) return Promise.resolve();
  return new Promise<void>((resolve) => {
    const waiter: Waiter = {
      resolve,
      timer: setTimeout(() => {
        waiters.get(docId)?.delete(waiter);
        resolve();
      }, timeoutMs),
    };
    const pending = waiters.get(docId);
    if (pending) pending.add(waiter);
    else waiters.set(docId, new Set([waiter]));
  });
}
