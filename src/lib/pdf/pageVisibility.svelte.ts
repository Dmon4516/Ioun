type VisibilityCallback = (
  visible: boolean,
  entry: IntersectionObserverEntry,
) => void;

/**
 * Registro compartido de visibilidad para todos los PageSlot.
 * Un único IntersectionObserver por visor en lugar de uno por página.
 */
export class PageVisibilityRegistry {
  #observer: IntersectionObserver | null = null;
  #callbacks = new Map<Element, VisibilityCallback>();
  #root: HTMLElement | null = null;

  attach(root: HTMLElement | null, margin = "600px 0px"): void {
    this.#root = root;
    this.#recreate(margin);
  }

  detach(): void {
    this.#observer?.disconnect();
    this.#observer = null;
    this.#root = null;
  }

  observe(element: Element, callback: VisibilityCallback): () => void {
    this.#callbacks.set(element, callback);
    this.#observer?.observe(element);
    return () => {
      this.#callbacks.delete(element);
      this.#observer?.unobserve(element);
    };
  }

  #recreate(margin: string): void {
    this.#observer?.disconnect();
    this.#observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          this.#callbacks.get(entry.target)?.(entry.isIntersecting, entry);
        }
      },
      { root: this.#root, rootMargin: margin, threshold: 0 },
    );
    for (const element of this.#callbacks.keys()) {
      this.#observer.observe(element);
    }
  }
}
