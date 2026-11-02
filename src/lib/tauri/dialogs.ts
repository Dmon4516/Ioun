import { confirm as tauriConfirm, message as tauriMessage } from "@tauri-apps/plugin-dialog";
import { isTauri } from "$lib/platform";

export async function confirmDialog(
  message: string,
  options: { title?: string; kind?: "info" | "warning" | "error" } = {},
): Promise<boolean> {
  if (!isTauri()) return window.confirm(message);
  return tauriConfirm(message, {
    title: options.title ?? "Reign",
    kind: options.kind ?? "warning",
  });
}

export async function alertDialog(
  message: string,
  options: { title?: string; kind?: "info" | "warning" | "error" } = {},
): Promise<void> {
  if (!isTauri()) {
    window.alert(message);
    return;
  }
  await tauriMessage(message, {
    title: options.title ?? "Reign",
    kind: options.kind ?? "info",
  });
}
