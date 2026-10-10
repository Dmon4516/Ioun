export function focusOnMount(node: HTMLElement) {
  requestAnimationFrame(() => {
    if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) {
      node.focus();
      node.select?.();
    } else if (typeof (node as HTMLElement).focus === "function") {
      (node as HTMLElement).focus();
    }
  });
}
