import type { FolderRecord } from "$lib/types";

export type DragKind = "doc" | "folder";
export type DropPosition = "before" | "after" | "inside";

export interface DndLibrary {
  folders: FolderRecord[];
  rootDocIds: string[];
  folderDocs: Record<string, string[]>;
}

export function positionWithin(
  lib: DndLibrary,
  folderId: string | null,
  kind: DragKind,
  id: string,
): number {
  if (kind === "folder") {
    const siblings = lib.folders
      .filter((folder) => folder.parentId === folderId)
      .sort((a, b) => a.position - b.position);
    return siblings.findIndex((folder) => folder.id === id);
  }
  const docs = folderId ? (lib.folderDocs[folderId] ?? []) : lib.rootDocIds;
  return docs.indexOf(id);
}

function siblingCount(
  lib: DndLibrary,
  folderId: string | null,
  kind: DragKind,
): number {
  if (kind === "folder") {
    return lib.folders.filter((f) => f.parentId === folderId).length;
  }
  return (folderId ? (lib.folderDocs[folderId] ?? []) : lib.rootDocIds).length;
}

export function computeTargetIndex(
  lib: DndLibrary,
  movedKind: DragKind,
  targetId: string,
  targetKind: DragKind,
  position: DropPosition,
  destFolderId: string | null,
): number {
  if (position === "inside") return 0;
  // El destino y el elemento movido pueden ser de tipos distintos. En ese
  // caso no hay correspondencia 1:1 entre los índices de las dos listas:
  // colocamos el elemento al final de su propia lista.
  if (targetKind !== movedKind) {
    return siblingCount(lib, destFolderId, movedKind);
  }
  const index = positionWithin(lib, destFolderId, targetKind, targetId);
  if (index === -1) return 0;
  return position === "after" ? index + 1 : index;
}

export function isDescendant(
  folders: FolderRecord[],
  candidateId: string,
  ancestorId: string,
): boolean {
  let current: string | null = candidateId;
  const visited = new Set<string>();
  while (current && !visited.has(current)) {
    visited.add(current);
    const folder = folders.find((item) => item.id === current);
    if (!folder) return false;
    if (folder.parentId === ancestorId) return true;
    current = folder.parentId;
  }
  return false;
}
