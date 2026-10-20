import { describe, expect, it } from "vitest";
import { scrollViewerBy } from "./viewer.svelte";

function mountViewer(height: number): { area: HTMLElement; calls: ScrollToOptions[] } {
  const area = document.createElement("div");
  area.id = "viewerArea";
  Object.defineProperty(area, "clientHeight", { value: height, configurable: true });
  const calls: ScrollToOptions[] = [];
  area.scrollBy = ((options: ScrollToOptions) => {
    calls.push(options);
  }) as never;
  document.body.append(area);
  return { area, calls };
}

describe("scrollViewerBy", () => {
  it("avanza algo menos de una pantalla por paso", () => {
    const { area, calls } = mountViewer(800);

    scrollViewerBy(1);
    scrollViewerBy(-1);

    expect(calls).toEqual([
      { top: 680, behavior: "smooth" },
      { top: -680, behavior: "smooth" },
    ]);
    area.remove();
  });

  it("mantiene un mínimo en visores muy bajos", () => {
    const { area, calls } = mountViewer(50);

    scrollViewerBy(1);

    expect(calls[0]?.top).toBe(80);
    area.remove();
  });

  it("permite pasos de una fracción distinta (Shift + flecha)", () => {
    const { area, calls } = mountViewer(800);

    scrollViewerBy(1, 0.15, 40);

    expect(calls[0]?.top).toBe(120);
    area.remove();
  });

  it("no hace nada si el visor no está montado", () => {
    expect(() => scrollViewerBy(1)).not.toThrow();
  });
});
