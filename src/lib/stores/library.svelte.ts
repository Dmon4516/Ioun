import { SvelteSet } from "svelte/reactivity";
import type { DocumentRecord, FolderChildren, FolderRecord, LibrarySnapshot, LibrarySort } from "$lib/types";
import { listFolderChildren, listLibraryRoot } from "$lib/tauri/commands";
import { settings } from "./settings.svelte";
import { documents } from "./documents.svelte";

export const library = $state<LibrarySnapshot>({
  folders: [],
  rootDocIds: [],
  folderDocs: {},
  folderChildCounts: {},
});

// Índice invertido `docId → folderId|null` para consultas O(1).
const docFolderIndex = $state<{ map: Map<string, string | null> }>({
  map: new Map(),
});

/**
 * Índices derivados de `library.folders`: `id → carpeta` y `parentId → hijos
 * ordenados`. Sin ellos, cada fila del árbol recorría la colección entera
 * (`folders.find`/`filter`) y pintar el árbol costaba O(n²). Son `$derived`, así
 * que se recalculan solos al cambiar la colección y solo si alguien los lee.
 */
const folderIndex = $derived.by(() => {
  const byId = new Map<string, FolderRecord>();
  const children = new Map<string | null, FolderRecord[]>();
  for (const folder of library.folders) {
    byId.set(folder.id, folder);
    const siblings = children.get(folder.parentId);
    if (siblings) siblings.push(folder);
    else children.set(folder.parentId, [folder]);
  }
  for (const siblings of children.values()) {
    siblings.sort((a, b) => a.position - b.position);
  }
  return { byId, children };
});

/** Carpetas cuyos hijos ya están en `library.folders`. */
const loadedFolders = new SvelteSet<string>();
const loadingFolders = new Set<string>();

export function applySnapshot(snap: LibrarySnapshot): void {
  library.folders = snap.folders;
  library.rootDocIds = snap.rootDocIds;
  library.folderDocs = snap.folderDocs;
  library.folderChildCounts = snap.folderChildCounts;
  loadedFolders.clear();
  for (const folder of snap.folders) loadedFolders.add(folder.id);
  rebuildDocIndex(snap);
}

/**
 * Snapshot de raíces: el árbol se carga al expandir, así que hay que conservar
 * las ramas que ya estaban cargadas (si no, cada `library-changed` colapsaría
 * lo que el usuario tenía abierto).
 */
export function mergeRootSnapshot(snap: LibrarySnapshot): void {
  const roots = new Set(snap.folders.map((folder) => folder.id));
  const kept = keepLoadedBranches(snap.folders, library.folders.filter((f) => !roots.has(f.id)));
  library.folders = [...snap.folders, ...kept];
  library.rootDocIds = snap.rootDocIds;
  library.folderDocs = snap.folderDocs;
  library.folderChildCounts = snap.folderChildCounts;

  const present = new Set(library.folders.map((folder) => folder.id));
  for (const id of [...loadedFolders]) {
    if (!present.has(id)) loadedFolders.delete(id);
  }
  rebuildDocIndex(snap);
}

/** Recorre desde las raíces y conserva solo lo que cuelga de ellas. */
function keepLoadedBranches(roots: FolderRecord[], candidates: FolderRecord[]): FolderRecord[] {
  const byParent = new Map<string, FolderRecord[]>();
  for (const folder of candidates) {
    if (!folder.parentId) continue;
    const siblings = byParent.get(folder.parentId) ?? [];
    siblings.push(folder);
    byParent.set(folder.parentId, siblings);
  }

  const kept: FolderRecord[] = [];
  const pending = roots.map((folder) => folder.id);
  while (pending.length) {
    const parentId = pending.pop();
    for (const child of byParent.get(parentId ?? "") ?? []) {
      kept.push(child);
      pending.push(child.id);
    }
  }
  return kept;
}

function rebuildDocIndex(snap: LibrarySnapshot): void {
  const next = new Map<string, string | null>();
  for (const id of snap.rootDocIds) next.set(id, null);
  for (const [folderId, docs] of Object.entries(snap.folderDocs)) {
    for (const docId of docs) next.set(docId, folderId);
  }
  docFolderIndex.map = next;
}

export function mergeFolderChildren(folderId: string, children: FolderChildren): void {
  library.folders = [
    ...library.folders.filter((folder) => folder.parentId !== folderId),
    ...children.folders,
  ];
  loadedFolders.add(folderId);
}

/** Hijos directos de una carpeta; siempre se piden frescos al expandir. */
export async function loadFolderChildren(folderId: string): Promise<void> {
  if (loadingFolders.has(folderId)) return;
  loadingFolders.add(folderId);
  try {
    mergeFolderChildren(folderId, await listFolderChildren(folderId));
  } catch (error) {
    console.error(`[sidebar] No se pudieron cargar los hijos de ${folderId}`, error);
  } finally {
    loadingFolders.delete(folderId);
  }
}

/**
 * Recarga de los caminos calientes (arranque, cambios de biblioteca): trae las
 * raíces y refresca solo las carpetas que están expandidas, que son las que se
 * ven. Lo demás se pide al expandirlo.
 */
export async function reloadLibrary(): Promise<void> {
  let rootSnap: LibrarySnapshot;
  try {
    rootSnap = await listLibraryRoot();
  } catch (error) {
    console.error("[library] No se pudo recargar la biblioteca", error);
    return;
  }
  const previouslyLoaded = new Set(loadedFolders);
  const expanded = [
    ...new Set([
      ...rootSnap.folders.filter((folder) => folder.expanded).map((folder) => folder.id),
      ...library.folders.filter((folder) => folder.expanded).map((folder) => folder.id),
    ]),
  ];
  const childrenResults = await Promise.all(
    expanded.map((id) =>
      listFolderChildren(id)
        .then((children) => ({ id, children, ok: true as const }))
        .catch((error) => {
          console.error(`[library] No se pudieron recargar los hijos de ${id}`, error);
          return { id, children: { folders: [] as FolderRecord[] }, ok: false as const };
        }),
    ),
  );

  const roots = new Set(rootSnap.folders.map((folder) => folder.id));
  const preserved = keepLoadedBranches(
    rootSnap.folders,
    library.folders.filter((folder) => !roots.has(folder.id)),
  );
  const refreshedChildren = childrenResults.flatMap(({ id, children, ok }) =>
    ok ? children.folders : library.folders.filter((folder) => folder.parentId === id),
  );
  const byId = new Map<string, FolderRecord>();
  for (const folder of [...rootSnap.folders, ...preserved, ...refreshedChildren]) {
    byId.set(folder.id, folder);
  }
  library.folders = [...byId.values()];
  library.rootDocIds = rootSnap.rootDocIds;
  library.folderDocs = rootSnap.folderDocs;
  library.folderChildCounts = rootSnap.folderChildCounts;
  loadedFolders.clear();
  for (const folder of library.folders) {
    if (previouslyLoaded.has(folder.id)) loadedFolders.add(folder.id);
  }
  for (const { id, ok } of childrenResults) if (ok) loadedFolders.add(id);
  rebuildDocIndex(rootSnap);
}

export function childrenOf(folderId: string | null): {
  folders: string[];
  docs: string[];
  loaded: boolean;
} {
  const folders = sortFolders(folderIndex.children.get(folderId) ?? []).map((f) => f.id);
  const docs = sortDocuments(folderId
    ? (library.folderDocs[folderId] ?? [])
    : library.rootDocIds);
  return { folders, docs, loaded: folderId === null || loadedFolders.has(folderId) };
}

function sortFolders(items: FolderRecord[]): FolderRecord[] {
  const sorted = [...items];
  if (settings.librarySort === "manual") return sorted;
  return sorted.sort((a, b) => compareRecords(a, b, settings.librarySort));
}

function sortDocuments(ids: string[]): string[] {
  const sorted = [...ids];
  if (settings.librarySort === "manual") return sorted;
  return sorted.sort((a, b) => {
    const left = documents.items.get(a);
    const right = documents.items.get(b);
    if (!left || !right) return a.localeCompare(b);
    return compareRecords(left, right, settings.librarySort);
  });
}

function compareRecords(
  left: Pick<FolderRecord, "name" | "createdAt"> | Pick<DocumentRecord, "title" | "createdAt" | "path">,
  right: Pick<FolderRecord, "name" | "createdAt"> | Pick<DocumentRecord, "title" | "createdAt" | "path">,
  mode: LibrarySort,
): number {
  if (mode === "newest" || mode === "oldest") {
    const result = left.createdAt - right.createdAt;
    return mode === "newest" ? -result : result;
  }
  if ("title" in left && "title" in right && mode === "type") {
    const leftType = left.path.split(".").pop()?.toLocaleLowerCase() ?? "";
    const rightType = right.path.split(".").pop()?.toLocaleLowerCase() ?? "";
    const result = leftType.localeCompare(rightType, undefined, { numeric: true });
    if (result !== 0) return result;
    return left.title.localeCompare(right.title, undefined, { numeric: true, sensitivity: "base" });
  }
  const leftName = "title" in left ? left.title : left.name;
  const rightName = "title" in right ? right.title : right.name;
  return leftName.localeCompare(rightName, undefined, { numeric: true, sensitivity: "base" });
}

export function hasLoadedChildren(folderId: string): boolean {
  return loadedFolders.has(folderId);
}

/** Nº de elementos de una carpeta, aunque sus subcarpetas no estén cargadas. */
export function folderChildCount(folderId: string): number {
  return (library.folderDocs[folderId]?.length ?? 0) + (library.folderChildCounts[folderId] ?? 0);
}

export function findFolder(id: string) {
  return folderIndex.byId.get(id) ?? null;
}

export function folderContaining(docId: string): string | null {
  return docFolderIndex.map.get(docId) ?? null;
}

export function isDocInLibrary(docId: string): boolean {
  return docFolderIndex.map.has(docId);
}
