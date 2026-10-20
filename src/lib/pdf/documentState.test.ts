import { afterEach, describe, expect, it, vi } from "vitest";
import {
  loadedDocuments,
  markDocumentLoaded,
  unmarkDocumentLoaded,
  whenDocumentLoaded,
} from "./documentState.svelte";

describe("whenDocumentLoaded", () => {
  afterEach(() => {
    loadedDocuments.clear();
    vi.useRealTimers();
  });

  it("se resuelve al instante si el documento ya está cargado", async () => {
    markDocumentLoaded("doc-1");

    await expect(whenDocumentLoaded("doc-1", 8000)).resolves.toBeUndefined();
  });

  it("se resuelve cuando el documento se marca como cargado", async () => {
    const pending = whenDocumentLoaded("doc-1", 8000);
    let settled = false;
    void pending.then(() => {
      settled = true;
    });

    await Promise.resolve();
    expect(settled).toBe(false);

    markDocumentLoaded("doc-1");

    await expect(pending).resolves.toBeUndefined();
  });

  it("se resuelve al vencer el plazo si el documento nunca llega", async () => {
    vi.useFakeTimers();

    const pending = whenDocumentLoaded("doc-1", 500);
    vi.advanceTimersByTime(500);

    await expect(pending).resolves.toBeUndefined();
  });

  it("no se queda con esperas colgadas tras vencer el plazo", async () => {
    vi.useFakeTimers();

    const pending = whenDocumentLoaded("doc-1", 500);
    vi.advanceTimersByTime(500);
    await pending;

    // Marcar después del plazo no debe intentar resolver nada ya retirado.
    expect(() => markDocumentLoaded("doc-1")).not.toThrow();
  });

  it("desmarcar un documento no resuelve las esperas pendientes", async () => {
    const pending = whenDocumentLoaded("doc-1", 8000);
    let settled = false;
    void pending.then(() => {
      settled = true;
    });

    unmarkDocumentLoaded("doc-1");
    await Promise.resolve();
    expect(settled).toBe(false);

    // Se limpia la espera para que la prueba no deje un temporizador vivo.
    markDocumentLoaded("doc-1");
    await pending;
  });

  it("resuelve a todas las esperas del mismo documento", async () => {
    const first = whenDocumentLoaded("doc-1", 8000);
    const second = whenDocumentLoaded("doc-1", 8000);

    markDocumentLoaded("doc-1");

    await expect(Promise.all([first, second])).resolves.toEqual([undefined, undefined]);
  });
});
