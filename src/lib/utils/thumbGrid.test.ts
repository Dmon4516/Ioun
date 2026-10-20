import { describe, expect, it } from "vitest";
import {
  FALLBACK_PAGE_ASPECT,
  pageAspect,
  thumbRowHeight,
  thumbSheetSize,
} from "./thumbGrid";

// Tamaños reales de los PDFs de prueba (en puntos).
const A4 = pageAspect(595.3, 841.9);
const LETTER = pageAspect(612, 792);
const TEXTBOOK = pageAspect(558, 684);

describe("pageAspect", () => {
  it("devuelve la proporción real de la página", () => {
    expect(A4).toBeCloseTo(0.707, 3);
    expect(LETTER).toBeCloseTo(0.773, 3);
    expect(TEXTBOOK).toBeCloseTo(0.816, 3);
  });

  it("cae en la proporción de reserva si las medidas no son válidas", () => {
    expect(pageAspect(0, 841.9)).toBe(FALLBACK_PAGE_ASPECT);
    expect(pageAspect(595.3, 0)).toBe(FALLBACK_PAGE_ASPECT);
    expect(pageAspect(Number.NaN, 841.9)).toBe(FALLBACK_PAGE_ASPECT);
    expect(pageAspect(-595.3, -841.9)).toBe(FALLBACK_PAGE_ASPECT);
  });
});

describe("thumbRowHeight", () => {
  it("da filas más altas cuanto más vertical es la página", () => {
    expect(thumbRowHeight(140, A4)).toBe(Math.round(140 / 0.707));
    expect(thumbRowHeight(140, LETTER)).toBe(Math.round(140 / 0.773));
  });

  it("nunca devuelve 0 ni valores negativos", () => {
    expect(thumbRowHeight(140, 0)).toBeGreaterThan(0);
    expect(thumbRowHeight(140, Number.NaN)).toBe(Math.round(140 / FALLBACK_PAGE_ASPECT));
    expect(thumbRowHeight(0, A4)).toBe(1);
  });
});

describe("thumbSheetSize", () => {
  it("ocupa todo el ancho cuando la proporción coincide con la celda", () => {
    // Celda del alto justo para una A4: la hoja llena el ancho disponible.
    const size = thumbSheetSize(132, Math.round(132 / A4), A4);
    expect(size.width).toBe(132);
    expect(size.height).toBe(Math.floor(132 / A4));
    expect(size.width / size.height).toBeCloseTo(A4, 2);
  });

  it("se limita por el alto en páginas más verticales que la celda", () => {
    const size = thumbSheetSize(132, 120, A4);
    expect(size.width).toBe(Math.floor(120 * A4));
    expect(size.width).toBeLessThan(132);
  });

  it("se limita por el ancho en páginas apaisadas", () => {
    const landscape = 1.414;
    const size = thumbSheetSize(132, 190, landscape);
    expect(size.width).toBe(132);
    expect(size.height).toBe(Math.floor(132 / landscape));
  });

  it("devuelve un tamaño vacío si la celda no tiene medidas", () => {
    expect(thumbSheetSize(0, 190, A4)).toEqual({ width: 0, height: 0 });
    expect(thumbSheetSize(132, 0, A4)).toEqual({ width: 0, height: 0 });
  });
});
