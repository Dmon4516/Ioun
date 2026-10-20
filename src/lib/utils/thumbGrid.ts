import { DEFAULT_ASPECT_RATIO } from "$lib/constants";

/** Proporción (ancho/alto) de reserva mientras no se conoce la real: la misma que
 *  usa el visor, para que la miniatura y la página no tengan formas distintas
 *  antes de medirse. */
export const FALLBACK_PAGE_ASPECT = DEFAULT_ASPECT_RATIO;

function validAspect(aspect: number): number {
  return Number.isFinite(aspect) && aspect > 0 ? aspect : FALLBACK_PAGE_ASPECT;
}

/** Proporción ancho/alto de una página a partir de su viewport en pdf.js. */
export function pageAspect(width: number, height: number): number {
  if (!(width > 0) || !(height > 0)) return FALLBACK_PAGE_ASPECT;
  return width / height;
}

/** Alto de fila para que la hoja más alta de la rejilla entre completa. */
export function thumbRowHeight(cellWidth: number, aspect: number): number {
  return Math.max(1, Math.round(cellWidth / validAspect(aspect)));
}

/**
 * Tamaño de la hoja dentro de la celda de una miniatura: se ajusta al hueco
 * disponible conservando su proporción, sin deformarla ni desbordar la fila.
 */
export function thumbSheetSize(
  boxWidth: number,
  boxHeight: number,
  aspect: number,
): { width: number; height: number } {
  if (!(boxWidth > 0) || !(boxHeight > 0)) return { width: 0, height: 0 };
  const ratio = validAspect(aspect);
  const width = Math.max(1, Math.floor(Math.min(boxWidth, boxHeight * ratio)));
  return { width, height: Math.max(1, Math.floor(width / ratio)) };
}
