import { invoke } from "@tauri-apps/api/core";
import type {
  Settings, DocumentRecord, DocumentView, FolderRecord, Bookmark, Comment, Category,
  LibrarySnapshot, FolderChildren, TrashItem, ImportSummary,
} from "$lib/types";

/* ───── Settings ───── */
export function loadSettings(): Promise<Settings | null> {
  return invoke("load_settings");
}
export function saveSettings(s: Settings): Promise<void> {
  return invoke("save_settings", { settings: s });
}

/* ───── Library ───── */
export function listLibrary(): Promise<LibrarySnapshot> {
  return invoke("list_library");
}
/** Raíces del árbol + mapa completo `carpeta → documentos`. */
export function listLibraryRoot(): Promise<LibrarySnapshot> {
  return invoke("list_library_root");
}
export function listFolderChildren(folderId: string): Promise<FolderChildren> {
  return invoke("list_folder_children", { folderId });
}
export function listDocuments(): Promise<DocumentRecord[]> {
  return invoke("list_documents");
}
export function addFile(path: string, folderId?: string | null): Promise<DocumentRecord> {
  return invoke("add_file", { path, folderId: folderId ?? null });
}
export function addFolder(
  name: string,
  kind: "temp" | "synced",
  parentId?: string | null,
  path?: string | null,
): Promise<FolderRecord> {
  return invoke("add_folder", { name, kind, parentId: parentId ?? null, path: path ?? null });
}
export function restoreDocumentToLibrary(docId: string): Promise<void> {
  return invoke("restore_document_to_library", { docId });
}
/** Arranca el sync y vuelve enseguida: el progreso llega por `sync-progress`. */
export function syncSyncedFolders(): Promise<void> {
  return invoke("sync_synced_folders");
}
export function cancelSync(): Promise<void> {
  return invoke("cancel_sync");
}
export function isSyncPaused(): Promise<boolean> {
  return invoke("is_sync_paused");
}
export function renameFolder(id: string, name: string): Promise<void> {
  return invoke("rename_folder", { id, name });
}
export function setFolderExpanded(id: string, expanded: boolean): Promise<void> {
  return invoke("set_folder_expanded", { id, expanded });
}
export function deleteFolder(id: string, deleteContents: boolean): Promise<string[]> {
  return invoke("delete_folder", { id, deleteContents });
}
export function moveDoc(docId: string, folderId: string | null, position: number): Promise<void> {
  return invoke("move_doc", { docId, folderId, position });
}
export function moveFolder(
  folderId: string,
  parentId: string | null,
  position: number,
  allowReparentSynced = false,
): Promise<void> {
  return invoke("move_folder", { folderId, parentId, position, allowReparentSynced });
}
export function removeDoc(docId: string): Promise<void> {
  return invoke("remove_doc", { docId });
}
export function removeFromLibrary(docId: string): Promise<void> {
  return invoke("remove_from_library", { docId });
}
export function assignCategory(docId: string, categoryId: string | null): Promise<void> {
  return invoke("assign_category", { docId, categoryId });
}
export function setDocumentCategory(
  docId: string,
  categoryId: string,
  assigned: boolean,
): Promise<void> {
  return invoke("set_document_category", { docId, categoryId, assigned });
}
export function clearDocumentCategories(docId: string): Promise<void> {
  return invoke("clear_document_categories", { docId });
}
export function clearRecentDocs(): Promise<void> {
  return invoke("clear_recent_docs");
}
export function removeRecentDoc(docId: string): Promise<void> {
  return invoke("remove_recent_doc", { docId });
}
export function setDocView(docId: string, view: DocumentView): Promise<void> {
  return invoke("set_doc_view", { docId, view });
}
export function saveDocumentProgress(
  docId: string,
  page: number,
  zoom: number,
): Promise<void> {
  return invoke("save_document_progress", { docId, page, zoom });
}
/** Longitud real del PDF en disco: pdf.js la necesita antes de pedir rangos. */
export function pdfFileLength(path: string): Promise<number> {
  return invoke("pdf_file_length", { path });
}

/** Bytes `[begin, end)` del PDF. `end` es exclusivo, como en `Range` de HTTP. */
export function readPdfRange(path: string, begin: number, end: number): Promise<ArrayBuffer> {
  return invoke("read_pdf_range", { path, begin, end });
}

export function updateDocumentPages(docId: string, pages: number): Promise<void> {
  return invoke("update_document_pages", { docId, pages });
}

export function drainPendingExternalFiles(): Promise<string[]> {
  return invoke("drain_pending_external_files");
}

/* ───── Trash ───── */
export function listTrash(): Promise<TrashItem[]> {
  return invoke("list_trash");
}
export function restoreFromTrash(id: string): Promise<string[]> {
  return invoke("restore_from_trash", { id });
}
export function emptyTrash(): Promise<void> {
  return invoke("empty_trash");
}

/* ───── Export / Import ───── */
export function exportLibrary(): Promise<string> {
  return invoke("export_library");
}
export function importLibrary(json: string, mode: "merge" | "replace"): Promise<ImportSummary> {
  return invoke("import_library", { json, mode });
}
/** El diálogo de archivo y la escritura ocurren en Rust. */
export function exportLibraryToFile(): Promise<string | null> {
  return invoke("export_library_to_file");
}
export function importLibraryFromFile(
  mode: "merge" | "replace",
): Promise<ImportSummary | null> {
  return invoke("import_library_from_file", { mode });
}

/* ───── Annotations ───── */
export function listBookmarks(docId: string): Promise<Bookmark[]> {
  return invoke("list_bookmarks", { docId });
}
export function addBookmark(docId: string, page: number, name: string): Promise<Bookmark> {
  return invoke("add_bookmark", { docId, page, name });
}
export function deleteBookmark(id: string): Promise<void> {
  return invoke("delete_bookmark", { id });
}

export function listComments(docId: string): Promise<Comment[]> {
  return invoke("list_comments", { docId });
}
export function addComment(docId: string, page: number | null, text: string): Promise<Comment> {
  return invoke("add_comment", { docId, page, text });
}
export function deleteComment(id: string): Promise<void> {
  return invoke("delete_comment", { id });
}

/* ───── Categories ───── */
export function listCategories(): Promise<Category[]> {
  return invoke("list_categories");
}
export function upsertCategory(c: Category): Promise<void> {
  return invoke("upsert_category", { category: c });
}
export function deleteCategory(id: string): Promise<void> {
  return invoke("delete_category", { id });
}

/* ───── Diagnóstico (solo desarrollo) ───── */
export interface ProcessMemory {
  /** RSS del proceso de la app. */
  rssMb: number;
  /** RSS del proceso y sus descendientes (incluye el proceso del webview). */
  treeRssMb: number;
  virtualMb: number;
}

/**
 * Solo disponible en builds de desarrollo: el comando de Rust está detrás de
 * `debug_assertions`.
 */
export function getProcessRss(): Promise<ProcessMemory> {
  return invoke("get_process_rss");
}

/* ───── Dialogs ───── */
export function openPdfDialog(): Promise<string[] | null> {
  return invoke("open_pdf_dialog");
}
export function openFolderDialog(): Promise<string | null> {
  return invoke("open_folder_dialog");
}
