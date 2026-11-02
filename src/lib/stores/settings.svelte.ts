import type { Settings, ColorOverrides } from "$lib/types";
import { saveSettings as persist, loadSettings } from "$lib/tauri/commands";
import { hexWithAlpha, safeColor } from "$lib/utils/colors";
import { validateSettings } from "./settingsValidation";
import { COLOR_DEFS, DEFAULT_SETTINGS } from "./settingsDefaults";

export { COLOR_DEFS, DEFAULT_SETTINGS } from "./settingsDefaults";

export const settings = $state<Settings>(structuredClone(DEFAULT_SETTINGS));

let saveTimer: ReturnType<typeof setTimeout> | null = null;

function saveSoon(): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void persist($state.snapshot(settings)).catch((error: unknown) => {
      console.error("[settings] No se pudieron guardar los ajustes", error);
    });
  }, 300);
}

export function updateSettings<K extends keyof Settings>(key: K, value: Settings[K]): void {
  settings[key] = value;
  saveSoon();
}

/** Cancela el guardado diferido y persiste ya. Usado al cerrar la ventana. */
export async function flushSettings(): Promise<void> {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  await persist($state.snapshot(settings)).catch((error: unknown) => {
    console.error("[settings] No se pudieron guardar los ajustes al cerrar", error);
  });
}

export function resetSettingsToDefaults(): void {
  Object.assign(settings, structuredClone(DEFAULT_SETTINGS));
  applyAppearance();
  saveSoon();
}

export function setColorOverride(theme: "light" | "dark", key: keyof ColorOverrides, value: string): void {
  settings.colors[theme][key] = value;
  saveSoon();
}

export function resetColors(theme: "light" | "dark"): void {
  settings.colors[theme] = {};
  saveSoon();
}

export function effectiveTheme(): "light" | "dark" {
  const mode = settings.themeMode;
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return mode === "light" ? "light" : "dark";
}

function systemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function applyAppearance(): void {
  const root = document.documentElement;
  const theme = effectiveTheme();
  root.dataset.theme = theme;
  try {
    // Espejo del tema para el script inline de `index.html`, que lo aplica antes
    // del primer pintado. La clave está duplicada allí a propósito: ese script
    // no puede importar nada.
    localStorage.setItem("ioun-theme", theme);
  } catch {
    // Sin localStorage el tema se aplica igual, solo se pierde el anti-FOUC.
  }
  const overrides = settings.colors[theme] ?? {};

  for (const def of COLOR_DEFS) {
    const value = overrides[def.key];
    if (def.key === "barColor") {
      if (value) {
        root.style.setProperty("--custom-bar-frosted", hexWithAlpha(value, "E6"));
        root.style.setProperty("--custom-bar-opaque", value);
      } else {
        root.style.removeProperty("--custom-bar-frosted");
        root.style.removeProperty("--custom-bar-opaque");
      }
      continue;
    }
    if (def.key === "selectionColor") {
      // Necesita transparencia para que se lea el texto que hay debajo.
      if (value) {
        root.style.setProperty("--selection-bg", `color-mix(in srgb, ${safeColor(value)} ${def.alpha ?? 34}%, transparent)`);
      } else {
        root.style.removeProperty("--selection-bg");
      }
      continue;
    }
    if (def.key === "activeBgColor") {
      if (value) {
        root.style.setProperty("--bg-active", `color-mix(in srgb, ${safeColor(value)} ${def.alpha ?? 14}%, transparent)`);
      } else {
        root.style.removeProperty("--bg-active");
      }
      continue;
    }
    if (value && def.cssVar) root.style.setProperty(def.cssVar, value);
    else if (def.cssVar) root.style.removeProperty(def.cssVar);
  }

  document.body.classList.toggle("opaque-surfaces", settings.surfaceStyle === "opaque");
  document.body.classList.toggle("hide-titlebar-always", settings.titlebarVisibility === "always");
  document.body.classList.toggle("hide-titlebar-auto", settings.titlebarVisibility === "auto");
  document.body.classList.toggle("hide-titlebar-fullscreen", settings.titlebarVisibility === "fullscreen");
  document.body.classList.toggle("no-titlebar-path", !settings.showTitlebarPath);
  document.body.classList.toggle("hide-icons", !settings.showIcons);
  document.body.dataset.pathsMode = settings.pathDisplay;
}

function pickColorTheme(): "light" | "dark" {
  const mode = settings.themeMode;
  if (mode === "system") return systemPrefersDark() ? "dark" : "light";
  return mode === "light" ? "light" : "dark";
}

export function currentEditTheme(): "light" | "dark" {
  return pickColorTheme();
}

export function hasCustomColors(theme: "light" | "dark"): boolean {
  const c = settings.colors[theme];
  return Object.values(c).some((v) => typeof v === "string" && v.length > 0);
}

export async function hydrateSettings(): Promise<void> {
  let stored: Awaited<ReturnType<typeof loadSettings>> = null;
  try {
    stored = await loadSettings();
  } catch (error) {
    console.error("[settings] No se pudieron leer los ajustes", error);
  }
  // `loadSettings` reads JSON from the backend; old/corrupt versions may
  // return primitives or arrays, which must not reach the migration below.
  if (stored && typeof stored === "object" && !Array.isArray(stored)) {
    const restored = validateSettings(stored);
    const legacyReading = Reflect.get(stored, "hideTitlebarInReading");
    const legacyFullscreen = Reflect.get(stored, "hideTitlebarInFullscreen");
    if (Reflect.get(stored, "titlebarVisibility") === undefined) {
      if (legacyReading === true && legacyFullscreen === true) {
        restored.titlebarVisibility = "always";
      } else if (legacyReading === true) {
        restored.titlebarVisibility = "auto";
      } else if (legacyFullscreen === true || Reflect.get(stored, "trafficHideFullscreen") === true) {
        restored.titlebarVisibility = "fullscreen";
      } else if (legacyReading === false || legacyFullscreen === false) {
        restored.titlebarVisibility = "never";
      }
    }
    Object.assign(settings, restored);
  }
  applyAppearance();
}
