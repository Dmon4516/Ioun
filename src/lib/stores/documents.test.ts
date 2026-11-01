import { beforeEach, describe, expect, it } from "vitest";
import { documents, reconcileDocuments, upsertDocument } from "./documents.svelte";
import type { DocumentRecord } from "$lib/types";

function makeDoc(id: string, overrides: Partial<DocumentRecord> = {}): DocumentRecord {
  return {
    id,
    path: `/${id}.pdf`,
    title: `${id}.pdf`,
    kind: "pdf",
    pages: 1,
    sizeBytes: 1,
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

describe("reconcileDocuments", () => {
  beforeEach(() => {
    documents.items.clear();
  });

  it("elimina los documentos que ya no vienen en la lista", () => {
    upsertDocument(makeDoc("a"));
    upsertDocument(makeDoc("b"));

    reconcileDocuments([makeDoc("a")]);

    expect(documents.items.has("a")).toBe(true);
    expect(documents.items.has("b")).toBe(false);
  });

  it("añade y actualiza los documentos de la lista", () => {
    upsertDocument(makeDoc("a", { title: "viejo.pdf" }));

    reconcileDocuments([makeDoc("a", { title: "nuevo.pdf" }), makeDoc("c")]);

    expect(documents.items.get("a")?.title).toBe("nuevo.pdf");
    expect(documents.items.has("c")).toBe(true);
  });

  it("conserva la identidad del mapa", () => {
    const map = documents.items;
    reconcileDocuments([makeDoc("a")]);

    expect(documents.items).toBe(map);
  });

  it("no vacía el store ante una lista vacía no autorizada", () => {
    upsertDocument(makeDoc("a"));

    reconcileDocuments([]);

    expect(documents.items.has("a")).toBe(true);
  });

  it("permite vaciar el store explícitamente al reemplazar", () => {
    upsertDocument(makeDoc("a"));

    reconcileDocuments([], true);

    expect(documents.items.size).toBe(0);
  });
});
