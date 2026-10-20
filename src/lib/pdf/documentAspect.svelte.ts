import { SvelteMap } from "svelte/reactivity";
import { DEFAULT_ASPECT_RATIO } from "$lib/constants";

/**
 * Proporción (ancho/alto) de la página 1 de cada documento abierto. La mide
 * `loader.ts` nada más abrir el PDF, y la comparten el visor (para colocar las
 * páginas) y las miniaturas: así ninguna de las dos tiene que abrir una página
 * por su cuenta, ni la primera colocación da un salto al llegar la medida real.
 */
export const documentAspect = new SvelteMap<string, number>();

/** ¿Se conoce ya la proporción medida de este documento? */
export function hasDocumentAspect(docId: string): boolean {
  return documentAspect.has(docId);
}

export function setDocumentAspect(docId: string, width: number, height: number): void {
  if (!(width > 0) || !(height > 0)) return;
  documentAspect.set(docId, width / height);
}

export function getDocumentAspect(docId: string | null | undefined): number {
  const value = docId ? documentAspect.get(docId) : undefined;
  return value !== undefined && value > 0 ? value : DEFAULT_ASPECT_RATIO;
}
