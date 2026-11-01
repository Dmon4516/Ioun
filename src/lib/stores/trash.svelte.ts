import { listTrash, restoreFromTrash, emptyTrash } from "$lib/tauri/commands";
import type { TrashItem } from "$lib/types";
import { showToast } from "./toast.svelte";
import { reloadLibrary } from "./library.svelte";
import { reconcileDocuments } from "./documents.svelte";
import { listDocuments } from "$lib/tauri/commands";

export const trash = $state<{ items: TrashItem[] }>({ items: [] });

const TRASH_ERROR_CODES = {
  TRASH_PATH_COLLISION: "TRASH_PATH_COLLISION",
  TRASH_ID_COLLISION: "TRASH_ID_COLLISION",
  TRASH_MISSING_PARENT: "TRASH_MISSING_PARENT",
  TRASH_MISSING_RELATION: "TRASH_MISSING_RELATION",
  TRASH_NOT_FOUND: "TRASH_NOT_FOUND",
  TRASH_UNKNOWN_KIND: "TRASH_UNKNOWN_KIND",
} as const;

function parseBackendError(raw: unknown): { code: string | null; message: string } {
  const text = String(raw);
  const separator = text.indexOf(":");
  if (separator <= 0) return { code: null, message: text };
  const code = text.slice(0, separator);
  const message = text.slice(separator + 1);
  const known: string[] = Object.values(TRASH_ERROR_CODES);
  if (!known.includes(code)) return { code: null, message: text };
  return { code, message };
}

function formatRestoreWarning(warning: string): string {
  const prefix = `${TRASH_ERROR_CODES.TRASH_MISSING_RELATION}:`;
  if (warning.startsWith(prefix)) {
    const [, relation, id] = warning.split(":");
    return `Se omitió una relación ${relation ?? "desconocida"} inexistente${id ? ` (${id})` : ""}.`;
  }
  return warning;
}

export async function refreshTrash(): Promise<void> {
  try {
    trash.items = await listTrash();
  } catch (error) {
    console.error("[trash] No se pudo cargar la papelera", error);
  }
}

export async function restoreItem(id: string): Promise<void> {
  try {
    const warnings = (await restoreFromTrash(id)) ?? [];
    await Promise.allSettled([
      refreshTrash(),
      reloadLibrary(),
      listDocuments().then((docs) => reconcileDocuments(docs)),
    ]);
    if (warnings.length > 0) {
      showToast(
        `Elemento restaurado con avisos: ${warnings.map(formatRestoreWarning).join(" ")}`,
        6000,
      );
    } else {
      showToast("Elemento restaurado.");
    }
  } catch (error) {
    console.error("[trash] No se pudo restaurar", error);
    const { code, message } = parseBackendError(error);
    const text = code === TRASH_ERROR_CODES.TRASH_PATH_COLLISION ||
      code === TRASH_ERROR_CODES.TRASH_ID_COLLISION ||
      code === TRASH_ERROR_CODES.TRASH_MISSING_PARENT
      ? message
      : code === TRASH_ERROR_CODES.TRASH_NOT_FOUND
      ? "El elemento ya no está en la papelera."
      : "No se pudo restaurar el elemento.";
    showToast(text, code ? 5000 : 2200);
  }
}

export async function emptyAll(): Promise<void> {
  try {
    await emptyTrash();
    await refreshTrash();
    showToast("Papelera vaciada.");
  } catch (error) {
    console.error("[trash] No se pudo vaciar", error);
    showToast("No se pudo vaciar la papelera.");
  }
}
