type VisibilityCallback = (visible: boolean) => void;
type LayoutCallback = () => void;

export class ThumbVisibility {
  #intersect: IntersectionObserver | null = null;
  #grid: ResizeObserver | null = null;
  #visibility = new Map<Element, VisibilityCallback>();
  #layout = new Set<LayoutCallback>();

  attach(scrollRoot: HTMLElement | null, grid: HTMLElement | null): void {
    this.detach();
    this.#intersect = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          this.#visibility.get(entry.target)?.(entry.isIntersecting);
        }
      },
      {
        root: scrollRoot ?? undefined,
        rootMargin: "400px 0px",
        threshold: 0.01,
      },
    );
    for (const element of this.#visibility.keys()) {
      this.#intersect.observe(element);
    }

    if (grid) {
      this.#grid = new ResizeObserver(() => this.notifyLayoutChange());
      this.#grid.observe(grid);
    }
  }

  detach(): void {
    this.#intersect?.disconnect();
    this.#grid?.disconnect();
    this.#intersect = null;
    this.#grid = null;
  }

  observe(element: Element, onVisible: VisibilityCallback): () => void {
    this.#visibility.set(element, onVisible);
    this.#intersect?.observe(element);
    return () => {
      this.#visibility.delete(element);
      this.#intersect?.unobserve(element);
    };
  }

  onLayoutChange(callback: LayoutCallback): () => void {
    this.#layout.add(callback);
    return () => {
      this.#layout.delete(callback);
    };
  }

  notifyLayoutChange(): void {
    for (const callback of this.#layout) callback();
  }
}
