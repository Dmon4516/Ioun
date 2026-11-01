import type { DocumentRecord, DocumentView, Bookmark, Comment, Category } from "$lib/types";
import { SvelteMap } from "svelte/reactivity";
import { setDocView, setDocumentCategory } from "$lib/tauri/commands";

export const documents = $state<{
  items: SvelteMap<string, DocumentRecord>;
  bookmarks: SvelteMap<string, Bookmark[]>;
  comments: SvelteMap<string, Comment[]>;
  categories: Category[];
}>({
  items: new SvelteMap(),
  bookmarks: new SvelteMap(),
  comments: new SvelteMap(),
  categories: [],
});

const documentViewWrites = new Map<string, Promise<void>>();
const categoryWrites = new Map<string, Promise<void>>();

async function persistDocumentView(
  docId: string,
  update: (current: DocumentView) => DocumentView,
): Promise<void> {
  const doc = documents.items.get(docId);
  if (!doc) throw new Error(`No existe el documento ${docId}`);

  const previousView = doc.view;
  const nextView = update(previousView);
  documents.items.set(docId, {
    ...doc,
    view: nextView,
  });

  const previous = documentViewWrites.get(docId) ?? Promise.resolve();
  const write = previous.catch(() => undefined).then(async () => {
    if (!documents.items.has(docId)) throw new Error(`No existe el documento ${docId}`);
    await setDocView(docId, nextView);
  });
  documentViewWrites.set(docId, write);

  try {
    await write;
  } catch (error) {
    const current = documents.items.get(docId);
    if (current?.view === nextView) {
      documents.items.set(docId, {
        ...current,
        view: previousView,
      });
    }
    throw error;
  } finally {
    if (documentViewWrites.get(docId) === write) documentViewWrites.delete(docId);
  }
}

export function getDocument(id: string): DocumentRecord | undefined {
  return documents.items.get(id);
}

export function upsertDocument(doc: DocumentRecord): void {
  documents.items.set(doc.id, doc);
}

export function reconcileDocuments(list: DocumentRecord[], allowEmpty = false): void {
  if (list.length === 0 && !allowEmpty) return;
  const present = new Set(list.map((doc) => doc.id));
  for (const id of [...documents.items.keys()]) {
    if (!present.has(id)) documents.items.delete(id);
  }
  for (const doc of list) {
    const current = documents.items.get(doc.id);
    documents.items.set(
      doc.id,
      current && documentViewWrites.has(doc.id)
        ? { ...doc, view: current.view }
        : doc,
    );
  }
}

export function updateDocView(docId: string, patch: DocumentView): Promise<void> {
  return persistDocumentView(docId, (current) => ({ ...current, ...patch }));
}

export function clearDocView(docId: string): Promise<void> {
  return persistDocumentView(docId, () => ({}));
}

export function removeDocument(id: string): void {
  documents.items.delete(id);
  documents.bookmarks.delete(id);
  documents.comments.delete(id);
}

export function setBookmarks(docId: string, list: Bookmark[]): void {
  documents.bookmarks.set(docId, list);
}

export function setComments(docId: string, list: Comment[]): void {
  documents.comments.set(docId, list);
}

export function setCategories(list: Category[]): void {
  documents.categories = list;
}

export async function toggleDocCategory(docId: string, categoryId: string): Promise<void> {
  const previous = categoryWrites.get(docId) ?? Promise.resolve();
  const write = previous.catch(() => undefined).then(async () => {
    const doc = documents.items.get(docId);
    if (!doc) throw new Error(`No existe el documento ${docId}`);
    const previousIds = [...doc.categoryIds];
    const assigned = !previousIds.includes(categoryId);
    const nextIds = assigned
      ? [...previousIds, categoryId]
      : previousIds.filter((id) => id !== categoryId);
    documents.items.set(docId, { ...doc, categoryIds: nextIds });
    try {
      await setDocumentCategory(docId, categoryId, assigned);
    } catch (error) {
      const current = documents.items.get(docId);
      if (current) documents.items.set(docId, { ...current, categoryIds: previousIds });
      throw error;
    }
  });
  categoryWrites.set(docId, write);
  try {
    await write;
  } finally {
    if (categoryWrites.get(docId) === write) categoryWrites.delete(docId);
  }
}
