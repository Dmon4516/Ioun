import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/** `ResizeObserver` de mentira: permite disparar entradas a mano. */
class FakeResizeObserver {
  static instances: FakeResizeObserver[] = [];

  observed = new Set<Element>();
  disconnected = false;
  #callback: ResizeObserverCallback;

  constructor(callback: ResizeObserverCallback) {
    this.#callback = callback;
    FakeResizeObserver.instances.push(this);
  }

  observe(element: Element): void {
    this.observed.add(element);
  }

  unobserve(element: Element): void {
    this.observed.delete(element);
  }

  disconnect(): void {
    this.disconnected = true;
    this.observed.clear();
  }

  emit(target: Element, width: number): void {
    this.#callback(
      [{ target, contentRect: { width } } as unknown as ResizeObserverEntry],
      this as unknown as ResizeObserver,
    );
  }
}

function observerAt(index: number): FakeResizeObserver {
  const observer = FakeResizeObserver.instances[index];
  if (!observer) throw new Error(`no se creó el observer ${index}`);
  return observer;
}

/** Frames pendientes, para ejecutarlos sin esperar al navegador. */
const frames: FrameRequestCallback[] = [];

/**
 * El módulo guarda un observer compartido, así que cada test lo carga de nuevo
 * con `resetModules` para no arrastrar suscripciones del anterior.
 */
async function load(): Promise<
  (element: Element, callback: () => void) => () => void
> {
  FakeResizeObserver.instances = [];
  frames.length = 0;
  vi.resetModules();
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    frames.push(callback);
    return frames.length;
  });
  vi.stubGlobal("cancelAnimationFrame", () => {});
  return (await import("./marqueeObserver")).observeMarquee;
}

function runPendingFrames(): void {
  for (const callback of frames.splice(0, frames.length)) callback(0);
}

beforeEach(() => {
  vi.unstubAllGlobals();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("observeMarquee", () => {
  it("comparte un solo observer para todos los títulos", async () => {
    const observeMarquee = await load();
    const first = document.createElement("div");
    const second = document.createElement("div");

    const stopFirst = observeMarquee(first, () => {});
    const stopSecond = observeMarquee(second, () => {});

    expect(FakeResizeObserver.instances).toHaveLength(1);
    expect([...observerAt(0).observed]).toEqual([first, second]);

    stopFirst();
    stopSecond();
  });

  it("avisa a los títulos cuyo ancho cambió, una vez por frame", async () => {
    const observeMarquee = await load();
    const first = document.createElement("div");
    const second = document.createElement("div");
    const onFirst = vi.fn();
    const onSecond = vi.fn();
    const stopFirst = observeMarquee(first, onFirst);
    const stopSecond = observeMarquee(second, onSecond);
    const observer = observerAt(0);

    observer.emit(first, 120);
    observer.emit(second, 200);
    runPendingFrames();
    expect(onFirst).toHaveBeenCalledTimes(1);
    expect(onSecond).toHaveBeenCalledTimes(1);

    // Mismo ancho (solo cambió el alto): no hay recálculo.
    observer.emit(first, 120);
    observer.emit(second, 260);
    runPendingFrames();
    expect(onFirst).toHaveBeenCalledTimes(1);
    expect(onSecond).toHaveBeenCalledTimes(2);

    stopFirst();
    stopSecond();
  });

  it("agrupa el recálculo de muchas filas en un solo frame", async () => {
    const observeMarquee = await load();
    const titles = Array.from({ length: 300 }, () => document.createElement("div"));
    const recalculated: number[] = [];
    const stops = titles.map((title, index) =>
      observeMarquee(title, () => {
        recalculated.push(index);
      }),
    );
    const observer = observerAt(0);

    for (const [index, title] of titles.entries()) observer.emit(title, index);

    expect(frames).toHaveLength(1);
    expect(recalculated).toHaveLength(0);
    runPendingFrames();
    expect(recalculated).toHaveLength(300);

    stops.forEach((stop) => stop());
  });

  it("destruye el observer cuando no queda ninguna suscripción", async () => {
    const observeMarquee = await load();
    const first = document.createElement("div");
    const second = document.createElement("div");

    const stopFirst = observeMarquee(first, () => {});
    const stopSecond = observeMarquee(second, () => {});
    const observer = observerAt(0);

    stopFirst();
    expect(observer.disconnected).toBe(false);
    expect(observer.observed.has(first)).toBe(false);

    stopSecond();
    expect(observer.disconnected).toBe(true);
  });

  it("vuelve a crear el observer si se vuelve a suscribir algo", async () => {
    const observeMarquee = await load();
    const element = document.createElement("div");

    observeMarquee(element, () => {})();
    observeMarquee(element, () => {});

    expect(FakeResizeObserver.instances).toHaveLength(2);
    expect(observerAt(1).observed.has(element)).toBe(true);
  });
});
