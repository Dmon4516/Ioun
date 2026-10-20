import { describe, expect, it } from "vitest";
import { pageWindow, pageWindowSpacers } from "./pageWindow";

describe("pageWindow", () => {
  it("monta solo la ventana visible más el margen", () => {
    const window = pageWindow(400, 402, 1, 900, 3);

    expect(window).toEqual({ firstRow: 396, lastRow: 404, firstPage: 397, lastPage: 405 });
    expect(window.lastPage - window.firstPage + 1).toBe(9);
  });

  it("recorta en los extremos del documento", () => {
    expect(pageWindow(1, 1, 1, 900, 3)).toEqual({
      firstRow: 0,
      lastRow: 3,
      firstPage: 1,
      lastPage: 4,
    });
    expect(pageWindow(900, 900, 1, 900, 3)).toEqual({
      firstRow: 896,
      lastRow: 899,
      firstPage: 897,
      lastPage: 900,
    });
  });

  it("agrupa por filas cuando hay varias páginas por fila", () => {
    const window = pageWindow(11, 12, 2, 900, 1);

    expect(window.firstRow).toBe(4);
    expect(window.lastRow).toBe(6);
    expect(window.firstPage).toBe(9);
    expect(window.lastPage).toBe(14);
  });

  it("aguanta medidas imposibles sin montar nada vacío", () => {
    const window = pageWindow(Number.NaN, 0, 0, 0, 3);

    expect(window.firstPage).toBe(1);
    expect(window.lastPage).toBe(1);
    expect(window.firstRow).toBe(0);
  });
});

describe("pageWindowSpacers", () => {
  /**
   * Altura real que produce la rejilla (`ContinuousGrid`) con estos huecos:
   * `gap` entre cada par de elementos, incluidos los huecos. Los huecos de
   * `pageWindowSpacers` ya descuentan su propio `gap`, así que sumando todos los
   * `gap` de la rejilla el total debe coincidir con el documento real.
   */
  function gridHeight(spacers: { top: number; bottom: number }, mounted: number, pageHeight: number, gap: number): number {
    // Elementos: hueco-superior (si top > 0), `mounted` páginas, hueco-inferior (si bottom > 0).
    const items = mounted + (spacers.top > 0 ? 1 : 0) + (spacers.bottom > 0 ? 1 : 0);
    return spacers.top + mounted * pageHeight + (items - 1) * gap + spacers.bottom;
  }

  it("los huecos mantienen la altura total del documento", () => {
    const pages = 900;
    const pageHeight = 1120;
    const gap = 26;
    const window = pageWindow(400, 402, 1, pages, 3);
    const spacers = pageWindowSpacers(window.firstRow, window.lastRow, pages, 1, pageHeight, gap);

    const mounted = window.lastPage - window.firstPage + 1;
    const total = gridHeight(spacers, mounted, pageHeight, gap);
    const expected = pages * pageHeight + (pages - 1) * gap;
    expect(total).toBe(expected);
  });

  it("mover la ventana no cambia la altura total", () => {
    const spacerTotal = (page: number) => {
      const window = pageWindow(page, page, 1, 900, 3);
      const spacers = pageWindowSpacers(window.firstRow, window.lastRow, 900, 1, 1120, 26);
      const mounted = window.lastPage - window.firstPage + 1;
      return gridHeight(spacers, mounted, 1120, 26);
    };

    expect(spacerTotal(2)).toBe(spacerTotal(450));
  });

  it("cada página queda en su fila exacta: fila × (alto + hueco)", () => {
    const pageHeight = 1120;
    const gap = 26;
    const pitch = pageHeight + gap;
    const window = pageWindow(400, 402, 1, 900, 3);
    const spacers = pageWindowSpacers(window.firstRow, window.lastRow, 900, 1, pageHeight, gap);

    // La primera página montada (fila `firstRow`) debe arrancar en su offset real.
    const firstMountedOffset = spacers.top + gap;
    expect(firstMountedOffset).toBe(window.firstRow * pitch);
    // La última debe terminar donde acaba su fila: la página mide `pitch - gap`.
    const mounted = window.lastPage - window.firstPage + 1;
    const lastMountedBottom = spacers.top + gap + (mounted - 1) * pitch + pageHeight;
    expect(lastMountedBottom).toBe((window.lastRow + 1) * pitch - gap);
  });

  it("en los extremos no deja huecos negativos", () => {
    const atTop = pageWindowSpacers(0, 3, 900, 1, 1120, 26);
    expect(atTop.top).toBe(0);
    expect(atTop.bottom).toBeGreaterThan(0);

    const atBottom = pageWindowSpacers(896, 899, 900, 1, 1120, 26);
    expect(atBottom.bottom).toBe(0);
    expect(atBottom.top).toBeGreaterThan(0);
  });
});
