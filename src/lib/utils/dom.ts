export function rafThrottle<A extends unknown[]>(fn: (...args: A) => void): (...args: A) => void {
  let raf = 0;
  return (...args: A) => {
    if (raf) return;
    raf = requestAnimationFrame(() => {
      raf = 0;
      fn(...args);
    });
  };
}

export function nextTick(): Promise<void> {
  return new Promise((r) => requestAnimationFrame(() => r()));
}
