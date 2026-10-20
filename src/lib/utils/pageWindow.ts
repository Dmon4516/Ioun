/** Margen de filas que se montan además de las visibles, a cada lado. */
export const PAGE_WINDOW_MARGIN_ROWS = 3;

export interface PageWindow {
  firstPage: number;
  lastPage: number;
  firstRow: number;
  lastRow: number;
}

/**
 * Rango de páginas a montar en la vista continua: la ventana visible más un
 * margen. Sin ventana, un documento de 900 páginas monta 900 componentes.
 */
export function pageWindow(
  firstVisiblePage: number,
  lastVisiblePage: number,
  pagesPerRow: number,
  totalPages: number,
  marginRows: number,
): PageWindow {
  const perRow = Math.max(1, Math.floor(pagesPerRow));
  const total = Math.max(1, Math.floor(totalPages));
  const totalRows = Math.ceil(total / perRow);
  const first = Math.min(Math.max(1, Math.floor(firstVisiblePage) || 1), total);
  const last = Math.min(Math.max(first, Math.floor(lastVisiblePage) || first), total);
  const firstRow = Math.max(0, Math.floor((first - 1) / perRow) - marginRows);
  const lastRow = Math.min(totalRows - 1, Math.floor((last - 1) / perRow) + marginRows);
  return {
    firstRow,
    lastRow,
    firstPage: firstRow * perRow + 1,
    lastPage: Math.min(total, (lastRow + 1) * perRow),
  };
}

/**
 * Altura de los huecos de arriba y abajo que sustituyen a las filas no montadas,
 * para que la barra de scroll y la posición no cambien al mover la ventana.
 */
export function pageWindowSpacers(
  firstRow: number,
  lastRow: number,
  totalPages: number,
  pagesPerRow: number,
  pageHeight: number,
  gap: number,
): { top: number; bottom: number } {
  const perRow = Math.max(1, Math.floor(pagesPerRow));
  const totalRows = Math.ceil(Math.max(1, Math.floor(totalPages)) / perRow);
  const pitch = pageHeight + gap;
  // El hueco sustituye filas enteras dentro de una rejilla con `gap`: entre el
  // hueco y la primera miniatura montada la rejilla añade un `gap` más, así que
  // la altura del hueco debe descontar ese hueco para que la fila `r` quede
  // exactamente en `r * pitch` (sin esto, todo el contenido se desplazaba un
  // `gap` hacia abajo en cuanto el hueco superior existía).
  return {
    top: Math.max(0, firstRow * pitch - gap),
    bottom: Math.max(0, (totalRows - 1 - lastRow) * pitch - gap),
  };
}
