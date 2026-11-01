import {library,
    findFolder,
  } from "$lib/stores/library.svelte";
import { settings } from "$lib/stores/settings.svelte";
import { moveDoc, moveFolder } from "$lib/tauri/commands";
import {
  computeTargetIndex as computeTargetIndexPure,
  isDescendant as isDescendantPure,
  type DragKind,
  type DropPosition,
} from "./dnd.utils";

export interface DragState {
  active: boolean;
  kind: DragKind | null;
  id: string | null;
  targetId: string | null;
  targetKind: DragKind | null;
  targetPosition: DropPosition | null;
}

export type { DragKind, DropPosition } from "./dnd.utils";

export const dnd = $state<DragState>({
  active: false,
  kind: null,
  id: null,
  targetId: null,
  targetKind: null,
  targetPosition: null,
});

let dragLeaveTimer = 0;

export function startDrag(kind: DragKind, id: string): void {
  dnd.active = true;
  dnd.kind = kind;
  dnd.id = id;
  dnd.targetId = null;
  dnd.targetKind = null;
  dnd.targetPosition = null;
}

export function endDrag(): void {
  dnd.active = false;
  dnd.kind = null;
  dnd.id = null;
  dnd.targetId = null;
  dnd.targetKind = null;
  dnd.targetPosition = null;
  clearTimeout(dragLeaveTimer);
}

export function setDropTarget(
  targetId: string,
  targetKind: DragKind,
  position: DropPosition,
): void {
  if (!dnd.active || !dnd.id) return;
  if (dnd.id === targetId && dnd.kind === targetKind) {
    clearDropTarget();
    return;
  }

  // Nunca permitir "inside" sobre un documento.
  if (position === "inside" && targetKind === "doc") {
    clearDropTarget();
    return;
  }

  // Folder no puede anidarse dentro de sí mismo
  if (dnd.kind === "folder" && targetKind === "folder" && position === "inside") {
    if (dnd.id === targetId || isDescendant(targetId, dnd.id)) {
      clearDropTarget();
      return;
    }
  }

  // Cancelar el clear pendiente: el cursor sigue sobre un destino válido.
  clearTimeout(dragLeaveTimer);

  dnd.targetId = targetId;
  dnd.targetKind = targetKind;
  dnd.targetPosition = position;
}

export function clearDropTarget(): void {
  // Delay para evitar parpadeos cuando el cursor pasa por un hueco
  clearTimeout(dragLeaveTimer);
  dragLeaveTimer = window.setTimeout(() => {
    dnd.targetId = null;
    dnd.targetKind = null;
    dnd.targetPosition = null;
  }, 30);
}

function isDescendant(candidateId: string, ancestorId: string): boolean {
  return isDescendantPure(library.folders, candidateId, ancestorId);
}

function computeTargetIndex(
  targetId: string,
  targetKind: DragKind,
  position: DropPosition,
  destFolderId: string | null,
): number {
  return computeTargetIndexPure(
    library,
    dnd.kind as DragKind,
    targetId,
    targetKind,
    position,
    destFolderId,
  );
}

async function targetFolderOf(
  targetId: string,
  targetKind: DragKind,
  position: DropPosition,
): Promise<string | null> {
  if (position === "inside") return targetId;
  if (targetKind === "folder") {
    const f = findFolder(targetId);
    return f?.parentId ?? null;
  }
  // targetKind === "doc": buscamos su carpeta contenedora
  for (const [fid, docs] of Object.entries(library.folderDocs)) {
    if (docs.includes(targetId)) return fid;
  }
  return null;
}

export async function performDrop(): Promise<boolean> {
  if (!dnd.active || !dnd.id || !dnd.kind) return false;
  if (!dnd.targetId || !dnd.targetKind || !dnd.targetPosition) {
    try {
      if (dnd.kind === "doc") {
        await moveDoc(dnd.id, null, library.rootDocIds.length);
      } else {
        const rootFolders = library.folders.filter((folder) => folder.parentId === null).length;
        await moveFolder(dnd.id, null, rootFolders, settings.allowReparentSyncedRoot);
      }
      return true;
    } catch (err) {
      console.error("[dnd] root end drop error", err);
      return false;
    } finally {
      endDrag();
    }
  }

  try {
    const destFolder = await targetFolderOf(dnd.targetId, dnd.targetKind, dnd.targetPosition);
    const index = computeTargetIndex(dnd.targetId, dnd.targetKind, dnd.targetPosition, destFolder);
    if (dnd.kind === "doc") {
      await moveDoc(dnd.id, destFolder, index);
    } else {
      await moveFolder(dnd.id, destFolder, index, settings.allowReparentSyncedRoot);
    }
    return true;
  } catch (err) {
    console.error("[dnd] move error", err);
    return false;
  } finally {
    endDrag();
  }
}
