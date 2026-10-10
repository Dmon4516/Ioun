export function clickOutside(
  node: HTMLElement,
  handler: (event: MouseEvent) => void,
) {
  function listener(event: MouseEvent) {
    const target = event.target as Node | null;
    if (!target) return;
    if (!node.contains(target)) handler(event);
  }
  document.addEventListener("mousedown", listener, true);
  return {
    destroy() {
      document.removeEventListener("mousedown", listener, true);
    },
  };
}
