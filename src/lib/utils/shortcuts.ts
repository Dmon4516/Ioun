import { isMac } from "./keyboard";

export const SHORTCUTS = {
  search: isMac ? "⌘K" : "Ctrl+K",
  searchAlt: "/",
  close: "Esc",

  prevPage: "←",
  nextPage: "→",
  scrollUp: "↑",
  scrollDown: "↓",
  scrollUpPage: isMac ? "⇧↑" : "Shift+↑",
  scrollDownPage: isMac ? "⇧↓" : "Shift+↓",
  prevView: "PageUp",
  nextView: "PageDown",
  firstPage: "Home",
  lastPage: "End",

  zoomIn: isMac ? "⌘+" : "Ctrl++",
  zoomOut: isMac ? "⌘−" : "Ctrl+−",

  closeTab: isMac ? "⌘W" : "Ctrl+W",
  restoreTab: isMac ? "⌘⇧T" : "Ctrl+Shift+T",
  switchTab: isMac ? "⌘1–9" : "Ctrl+1–9",

  toggleSidebar: isMac ? "⌘B" : "Ctrl+B",
  toggleInspector: isMac ? "⌘I" : "Ctrl+I",
  readingMode: isMac ? "⌘⇧L" : "Ctrl+Shift+L",
} as const;
