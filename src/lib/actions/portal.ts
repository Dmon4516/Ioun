export function portal(node: HTMLElement) {
  const placeholder = document.createComment("portal");
  node.parentNode?.insertBefore(placeholder, node);
  document.body.appendChild(node);

  return {
    destroy() {
      placeholder.parentNode?.insertBefore(node, placeholder);
      placeholder.remove();
    },
  };
}
