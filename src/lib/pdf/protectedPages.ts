/**
 * Registro de páginas "protegidas": las que el LRU de `renderPage.ts` no debe
 * expulsar. Una página está protegida mientras sea visible en el visor o
 * mientras haya un render en curso para ella.
 *
 * Vive fuera de pdf.js para poder limpiarlo sin tocar el caché de páginas.
 */
const protectedPages = new Set<string>();

function key(docId: string, page: number): string {
  return `${docId}:${page}`;
}

export function protectPage(docId: string, page: number): void {
  protectedPages.add(key(docId, page));
}

export function unprotectPage(docId: string, page: number): void {
  protectedPages.delete(key(docId, page));
}

export function isPageProtected(docId: string, page: number): boolean {
  return protectedPages.has(key(docId, page));
}

/** Limpia todas las entradas de un documento. Usar al cerrarlo o hibernarlo. */
export function clearProtectedPages(docId: string): void {
  const prefix = `${docId}:`;
  const stale: string[] = [];
  for (const entry of protectedPages) {
    if (entry.startsWith(prefix)) stale.push(entry);
  }
  for (const entry of stale) protectedPages.delete(entry);
}
