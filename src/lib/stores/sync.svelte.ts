import type { SyncProgress } from "$lib/tauri/events";

/** Estado visible del sync de carpetas vinculadas. */
export const syncStatus = $state<{ active: boolean; paused: boolean; path: string }>({
  active: false,
  paused: false,
  path: "",
});

export function applySyncProgress(progress: SyncProgress): void {
  syncStatus.active = progress.phase === "scan";
  if (progress.path) syncStatus.path = progress.path;
}

export function applySyncPaused(paused: boolean): void {
  syncStatus.paused = paused;
  if (paused) syncStatus.active = false;
}

/** Nombre de la carpeta que se está sincronizando (el path completo no cabe). */
export function syncingFolderName(): string {
  return syncStatus.path.replace(/[\\/]+$/, "").split(/[\\/]/).at(-1) ?? "";
}
