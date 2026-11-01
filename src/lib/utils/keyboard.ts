export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad|iPod/.test(navigator.userAgent);
export const MOD = isMac ? "⌘" : "Ctrl+";
export const SHIFT = isMac ? "⇧" : "Shift+";

export function isTypingTarget(el: EventTarget | null): boolean {
  if (typeof HTMLElement === "undefined" || !(el instanceof HTMLElement)) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}
