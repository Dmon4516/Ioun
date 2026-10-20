import type { DocumentRecord } from "$lib/types";
import { settings } from "$lib/stores/settings.svelte";

export function effectivePagesPerRow(
  doc: DocumentRecord | null | undefined,
): 1 | 2 | 3 | 4 {
  return doc?.view?.pagesPerRow ?? settings.pagesPerRow;
}

export function effectiveContinuousScroll(
  doc: DocumentRecord | null | undefined,
): boolean {
  return doc?.view?.continuousScroll ?? settings.continuousScroll;
}

export function docHasViewOverrides(doc: DocumentRecord | null | undefined): boolean {
  return doc?.view?.pagesPerRow !== undefined ||
    doc?.view?.continuousScroll !== undefined;
}

export function spreadStartPage(
  activePage: number,
  totalPages: number,
  pagesPerRow: 1 | 2 | 3 | 4,
): number {
  const page = Math.max(1, Math.round(activePage));
  const rowStart = Math.floor((page - 1) / pagesPerRow) * pagesPerRow + 1;
  const maxStart = Math.max(1, totalPages - pagesPerRow + 1);
  return Math.min(rowStart, maxStart);
}
