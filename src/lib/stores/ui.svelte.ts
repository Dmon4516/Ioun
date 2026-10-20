import type { PanelName } from "$lib/types";

export interface ContextItem {
  label: string;
  danger?: boolean;
  action: () => void;
}

export const ui = $state({
  inspector: null as PanelName | null,
  showSidebar: true,
  reading: false,
  toolbarHidden: false,
  searchOpen: false,
  settingsOpen: false,
  trashOpen: false,
  lastActivity: Date.now(),
});

export const sidebarView = $state<{ mode: "tabs" | "categories" }>({
  mode: "tabs",
});

export const sidebarPeek = $state<{ active: boolean }>({
  active: false,
});

export const contextMenu = $state<{
  open: boolean;
  x: number;
  y: number;
  items: ContextItem[];
}>({
  open: false,
  x: 0,
  y: 0,
  items: [],
});

export function showContextMenu(x: number, y: number, items: ContextItem[]): void {
  contextMenu.x = x;
  contextMenu.y = y;
  contextMenu.items = items;
  contextMenu.open = true;
}

export function hideContextMenu(): void {
  contextMenu.open = false;
  contextMenu.items = [];
}

export function toggleInspector(panel: PanelName): void {
  ui.inspector = ui.inspector === panel ? null : panel;
}

export function openInspector(panel: PanelName): void {
  ui.inspector = panel;
}

export function closeInspector(): void {
  ui.inspector = null;
}

export function toggleSidebar(): void {
  ui.showSidebar = !ui.showSidebar;
}

export function toggleReading(): void {
  ui.reading = !ui.reading;
  if (ui.reading) ui.inspector = null;
}

export function markActivity(): void {
  ui.lastActivity = Date.now();
}
