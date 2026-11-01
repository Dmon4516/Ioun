import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface FileDropPayload {
  paths: string[];
  position: { x: number; y: number };
}

export async function onFileDrop(handler: (payload: FileDropPayload) => void): Promise<UnlistenFn> {
  return getCurrentWindow().onDragDropEvent((event) => {
    if (event.payload.type === "drop") {
      handler({ paths: event.payload.paths, position: event.payload.position });
    }
  });
}

export async function onLibraryChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("library-changed", handler);
}

export interface SyncProgress {
  phase: "scan" | "done" | "error";
  path?: string;
  message?: string;
  changed?: boolean;
}

/** Progreso del sync de carpetas vinculadas. */
export async function onSyncProgress(
  handler: (progress: SyncProgress) => void,
): Promise<UnlistenFn> {
  return listen<SyncProgress>("sync-progress", (event) => handler(event.payload));
}

export async function onSyncPaused(
  handler: (paused: boolean) => void,
): Promise<UnlistenFn> {
  return listen<boolean>("sync-paused", (event) => handler(event.payload));
}

/** El backend no pudo inicializar la base de datos. */
export async function onDbInitFailed(
  handler: (message: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("db-init-failed", (event) => handler(event.payload));
}

export async function onWatcherInitFailed(
  handler: (message: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("watcher-init-failed", (event) => handler(event.payload));
}

export async function onExternalFileOpen(
  handler: (path: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("open-file-external", (event) => handler(event.payload));
}

/**
 * Avisa del estado de pantalla completa de la ventana.
 * En macOS el semáforo desaparece y su hueco queda libre para el título.
 */
export async function onFullscreenChange(handler: (fullscreen: boolean) => void): Promise<UnlistenFn> {
  const win = getCurrentWindow();
  const sync = async () => {
    try {
      handler(await win.isFullscreen());
    } catch (error) {
      console.warn("[window] No se pudo consultar la pantalla completa", error);
    }
  };
  const unlisten = await win.onResized(() => void sync());
  await sync();
  return unlisten;
}
