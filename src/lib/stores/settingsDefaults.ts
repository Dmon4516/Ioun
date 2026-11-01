import type { Settings, ColorDef } from "$lib/types";

/**
 * Valores por defecto de los ajustes y catálogo de colores.
 *
 * Vive en un módulo hoja a propósito: `settings.svelte.ts` importa
 * `validateSettings` y `settingsValidation.ts` importa `DEFAULT_SETTINGS`, así
 * que dejarlos en `settings.svelte.ts` creaba un ciclo de imports que hoy
 * funciona solo porque los dos usos son diferidos.
 */
export const COLOR_DEFS: ColorDef[] = [
  { key: "accentColor", label: "Acento", cssVar: "--accent", light: "#0a84ff", dark: "#64d2ff", group: "interface" },
  { key: "activeBgColor", label: "Hover / selección", cssVar: null, light: "#0a84ff", dark: "#64d2ff", alpha: 14, group: "interface" },
  { key: "barColor", label: "Barras", cssVar: null, light: "#ffffff", dark: "#1a1d23", group: "interface" },
  { key: "contentColor", label: "Fondo", cssVar: "--bg-content", light: "#dfe3e8", dark: "#0c0e12", group: "interface" },
  { key: "borderColor", label: "Bordes", cssVar: "--border", light: "#d1d5db", dark: "#262b33", group: "interface" },
  { key: "iconColor", label: "Iconos", cssVar: "--icon-color", light: "#656d78", dark: "#99a1ad", group: "interface" },
  { key: "textPrimaryColor", label: "Texto principal", cssVar: "--text-primary", light: "#181b20", dark: "#f3f5f8", group: "text" },
  { key: "textSecondaryColor", label: "Texto secundario", cssVar: "--text-secondary", light: "#656d78", dark: "#99a1ad", group: "text" },
  { key: "stateActiveColor", label: "Activo", cssVar: "--state-active", light: "#0a84ff", dark: "#64d2ff", group: "state" },
  { key: "stateIdleColor", label: "Inactivo", cssVar: "--state-idle", light: "#8a919c", dark: "#9aa1ad", group: "state" },
  { key: "stateHibernatedColor", label: "Hibernado", cssVar: "--state-hibernated", light: "#9a7bff", dark: "#9a7bff", group: "state" },
  { key: "stateRestoringColor", label: "Restaurando", cssVar: "--state-restoring", light: "#ffb340", dark: "#ffb340", group: "state" },
  { key: "selectionColor", label: "Texto seleccionado", cssVar: "--selection-bg", light: "#0a84ff", dark: "#64d2ff", alpha: 34, group: "interface" },
];

export const DEFAULT_SETTINGS: Settings = {
  settingsVersion: 23,
  ramLimitMB: 2048, maxLoadedTabs: 5, maxPagesLoaded: 30, maxActiveFiles: 10,
  maxRecentDocs: 20, maxClosedTabs: 5, inactiveTimeoutSec: 300, libraryRefreshDelayMs: 1500,
  scrollbarMode: "auto", defaultPanel: "thumbs",
  pagesPerRow: 1, continuousScroll: true,
  surfaceStyle: "opaque", bottomBarMode: "auto", pathDepth: 2,
  openLastLocation: true, titlebarVisibility: "auto",
  recenterOnToggle: true, categoriesEnabled: true,
  showTitlebarPath: true, showExtensions: true, showIcons: true, marqueeTitle: true,
  pathDisplay: "expand", thumbSizePx: 100,
  pageIndicatorMode: "change", pageIndicatorFormat: "page",
  deleteFolderContents: false,
  themeMode: "dark", consistentPageSize: true,
  consistentDocZoom: false, globalZoom: 100, fitMode: "spread",
  textLayerEnabled: true, textLayerSelectable: true, ignoreWatermarkOnCopy: false,
  flushOnClose: true, allowReparentSyncedRoot: false, librarySort: "manual",
  colors: { light: {}, dark: {} },
};
