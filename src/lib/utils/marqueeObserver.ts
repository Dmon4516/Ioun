/*
 * Un único `ResizeObserver` para todos los títulos del sidebar. Con 1.000
 * archivos, uno por fila eran 1.000 observers nativos; aquí hay uno solo y los
 * recálculos se agrupan en un `requestAnimationFrame` común.
 */

type MarqueeCallback = () => void;

interface Subscription {
  callback: MarqueeCallback;
  /** Último ancho observado, para no recalcular si solo cambió el alto. */
  width: number;
}

const subscriptions = new Map<Element, Subscription>();
const dirty = new Set<Element>();
let observer: ResizeObserver | null = null;
let frame = 0;

function flush(): void {
  frame = 0;
  const pending = [...dirty];
  dirty.clear();
  for (const element of pending) {
    subscriptions.get(element)?.callback();
  }
}

function onResize(entries: ResizeObserverEntry[]): void {
  for (const entry of entries) {
    const subscription = subscriptions.get(entry.target);
    if (!subscription) continue;
    // El marquee depende del ancho disponible del título, no de su alto.
    const width = entry.contentRect.width;
    if (width === subscription.width) continue;
    subscription.width = width;
    dirty.add(entry.target);
  }
  if (dirty.size > 0 && frame === 0) {
    frame = requestAnimationFrame(flush);
  }
}

/**
 * Avisa cuando cambia el ancho de `element`. Devuelve la función para darse de
 * baja; el observer se destruye cuando no queda ninguna suscripción.
 */
export function observeMarquee(element: Element, callback: MarqueeCallback): () => void {
  observer ??= new ResizeObserver(onResize);
  subscriptions.set(element, { callback, width: -1 });
  observer.observe(element);

  return () => {
    subscriptions.delete(element);
    dirty.delete(element);
    observer?.unobserve(element);
    if (subscriptions.size === 0) {
      observer?.disconnect();
      observer = null;
      if (frame !== 0) {
        cancelAnimationFrame(frame);
        frame = 0;
      }
    }
  };
}
