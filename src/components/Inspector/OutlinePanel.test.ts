import { beforeEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import OutlinePanel from "./OutlinePanel.svelte";
import { openDocument } from "$lib/pdf/loader";
import type { DocumentRecord } from "$lib/types";

vi.mock("$lib/pdf/loader", () => ({
  openDocument: vi.fn(async () => ({
    getOutline: async () => [{ title: "Capítulo 1", dest: null, items: [] }],
  })),
}));

function makeDoc(overrides: Partial<DocumentRecord> = {}): DocumentRecord {
  return {
    id: "doc-1",
    path: "/tmp/book.pdf",
    title: "book.pdf",
    kind: "pdf",
    pages: 100,
    sizeBytes: 1024,
    hash: null,
    categoryIds: [],
    lastPage: 1,
    lastZoom: 100,
    lastOpened: null,
    inRecent: false,
    inLibrary: true,
    createdAt: 0,
    view: {},
    viewState: {},
    position: 0,
    ...overrides,
  };
}

describe("OutlinePanel", () => {
  beforeEach(() => {
    vi.mocked(openDocument).mockClear();
  });

  it("construye el índice al abrir el documento", async () => {
    const { findByText } = render(OutlinePanel, { props: { doc: makeDoc() } });
    expect(await findByText("Capítulo 1")).toBeInTheDocument();
    expect(vi.mocked(openDocument)).toHaveBeenCalledTimes(1);
  });

  /**
   * Regresión: el store sustituye el objeto del documento en cada zoom o
   * cambio de página, así que la identidad de `doc` cambia aunque sigamos en el
   * mismo documento. Antes eso reconstruía el índice (parpadeo de "cargando").
   */
  it("no reconstruye el índice cuando solo cambia el progreso de lectura", async () => {
    const { rerender, findByText } = render(OutlinePanel, { props: { doc: makeDoc() } });
    expect(await findByText("Capítulo 1")).toBeInTheDocument();
    expect(vi.mocked(openDocument)).toHaveBeenCalledTimes(1);

    // Un zoom guarda el progreso sustituyendo el objeto por uno nuevo.
    await rerender({ doc: makeDoc({ lastPage: 42, lastZoom: 175 }) });

    expect(vi.mocked(openDocument)).toHaveBeenCalledTimes(1);
  });

  it("reconstruye el índice al cambiar de documento", async () => {
    const { rerender, findByText } = render(OutlinePanel, { props: { doc: makeDoc() } });
    expect(await findByText("Capítulo 1")).toBeInTheDocument();

    await rerender({ doc: makeDoc({ id: "doc-2", path: "/tmp/other.pdf" }) });

    expect(vi.mocked(openDocument)).toHaveBeenCalledTimes(2);
  });
});
