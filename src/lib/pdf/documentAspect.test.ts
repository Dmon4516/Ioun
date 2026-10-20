import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_ASPECT_RATIO } from "$lib/constants";
import {
  documentAspect,
  getDocumentAspect,
  hasDocumentAspect,
  setDocumentAspect,
} from "./documentAspect.svelte";

describe("documentAspect", () => {
  beforeEach(() => {
    documentAspect.clear();
  });

  it("guarda la proporción medida al abrir el documento", () => {
    setDocumentAspect("doc-1", 595.3, 841.9);

    expect(hasDocumentAspect("doc-1")).toBe(true);
    expect(getDocumentAspect("doc-1")).toBeCloseTo(0.707, 3);
  });

  it("usa la proporción de reserva mientras no se conoce", () => {
    expect(hasDocumentAspect("doc-1")).toBe(false);
    expect(getDocumentAspect("doc-1")).toBe(DEFAULT_ASPECT_RATIO);
    expect(getDocumentAspect(null)).toBe(DEFAULT_ASPECT_RATIO);
    expect(getDocumentAspect(undefined)).toBe(DEFAULT_ASPECT_RATIO);
  });

  it("ignora medidas imposibles sin pisar la anterior", () => {
    setDocumentAspect("doc-1", 595.3, 841.9);
    setDocumentAspect("doc-1", 0, 841.9);
    setDocumentAspect("doc-1", Number.NaN, Number.NaN);

    expect(getDocumentAspect("doc-1")).toBeCloseTo(0.707, 3);
  });

  it("cada documento tiene la suya", () => {
    setDocumentAspect("doc-1", 595.3, 841.9);
    setDocumentAspect("doc-2", 841.9, 595.3);

    expect(getDocumentAspect("doc-1")).toBeCloseTo(0.707, 3);
    expect(getDocumentAspect("doc-2")).toBeCloseTo(1.414, 3);
  });
});
