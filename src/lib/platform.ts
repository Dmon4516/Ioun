const TAURI_INTERNALS = "__TAURI_INTERNALS__";

export function isTauri(): boolean {
  return typeof window !== "undefined" && TAURI_INTERNALS in window;
}

export function detectPlatformSync(): string {
  if (!isTauri()) return "web";
  const userAgent = navigator.userAgent.toLowerCase();
  if (userAgent.includes("mac os") || userAgent.includes("macintosh") || userAgent.includes("darwin")) return "macos";
  if (userAgent.includes("windows")) return "windows";
  if (userAgent.includes("android")) return "android";
  if (userAgent.includes("iphone") || userAgent.includes("ipad") || userAgent.includes("ipod")) return "ios";
  if (userAgent.includes("linux") || userAgent.includes("x11")) return "linux";
  return "unknown";
}

export async function detectPlatform(): Promise<string> {
  let detected = document.documentElement.dataset.platform || detectPlatformSync();
  if (isTauri()) {
    try {
      const { platform } = await import("@tauri-apps/plugin-os");
      detected = await platform();
    } catch (error) {
      console.warn("[platform] No se pudo confirmar el sistema operativo con Tauri", error);
    }
  }
  document.documentElement.dataset.platform = detected;
  return detected;
}

export function currentPlatform(): string {
  return typeof document === "undefined" ? "web" : document.documentElement.dataset.platform ?? "web";
}

export function isMac(): boolean {
  return currentPlatform() === "macos";
}
