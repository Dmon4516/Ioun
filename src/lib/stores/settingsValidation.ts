import type { Settings } from "$lib/types";
import { DEFAULT_SETTINGS } from "./settingsDefaults";
import { clampInt, HEX_RE } from "$lib/utils/colors";
import { MAX_ZOOM, MIN_ZOOM } from "$lib/constants";

const isBool = (v: unknown): v is boolean => typeof v === "boolean";
const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);

function clampFloat(v: unknown, min: number, max: number, fallback: number): number {
  if (!isNum(v)) return fallback;
  return Math.min(max, Math.max(min, v));
}

function oneOf<T extends string>(valid: readonly T[], value: unknown, fallback: T): T {
  return typeof value === "string" && (valid as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}

const COLOR_KEYS = [
  "accentColor", "activeBgColor", "barColor", "contentColor",
  "borderColor", "iconColor", "textPrimaryColor", "textSecondaryColor",
  "stateActiveColor", "stateIdleColor", "stateHibernatedColor", "stateRestoringColor",
  "selectionColor",
] as const;

/** Sanea ajustes leídos de disco: cualquier valor inválido cae al default. */
export function validateSettings(raw: unknown): Settings {
  const base = structuredClone(DEFAULT_SETTINGS);
  if (!raw || typeof raw !== "object") return base;
  const r = raw as Record<string, unknown>;

  base.settingsVersion = clampInt(r.settingsVersion, 1, Number.MAX_SAFE_INTEGER, base.settingsVersion);
  base.ramLimitMB = clampInt(r.ramLimitMB, 128, 65_536, base.ramLimitMB);
  base.maxLoadedTabs = clampInt(r.maxLoadedTabs, 1, 100, base.maxLoadedTabs);
  base.maxPagesLoaded = clampInt(r.maxPagesLoaded, 1, 500, base.maxPagesLoaded);
  base.maxActiveFiles = clampInt(r.maxActiveFiles, 1, 500, base.maxActiveFiles);
  base.maxRecentDocs = clampInt(r.maxRecentDocs, 1, 1000, base.maxRecentDocs);
  base.maxClosedTabs = clampInt(r.maxClosedTabs, 1, 100, base.maxClosedTabs);
  base.inactiveTimeoutSec = clampInt(r.inactiveTimeoutSec, 5, 86_400, base.inactiveTimeoutSec);
  base.libraryRefreshDelayMs = clampInt(r.libraryRefreshDelayMs, 200, 10_000, base.libraryRefreshDelayMs);
  base.scrollbarMode = oneOf(["auto", "always"] as const, r.scrollbarMode, base.scrollbarMode);
  base.defaultPanel = oneOf(["info", "outline", "thumbs"] as const, r.defaultPanel, base.defaultPanel);
  base.pagesPerRow = clampInt(r.pagesPerRow, 1, 4, base.pagesPerRow) as Settings["pagesPerRow"];
  base.continuousScroll = isBool(r.continuousScroll) ? r.continuousScroll : base.continuousScroll;
  base.surfaceStyle = oneOf(["frosted", "opaque"] as const, r.surfaceStyle, base.surfaceStyle);
  base.bottomBarMode = oneOf(["auto", "always"] as const, r.bottomBarMode, base.bottomBarMode);
  base.pathDepth = clampInt(r.pathDepth, 0, 10, base.pathDepth);
  base.openLastLocation = isBool(r.openLastLocation) ? r.openLastLocation : base.openLastLocation;
  base.titlebarVisibility = oneOf(
    ["always", "never", "auto", "fullscreen"] as const,
    r.titlebarVisibility,
    base.titlebarVisibility,
  );
  base.recenterOnToggle = isBool(r.recenterOnToggle) ? r.recenterOnToggle : base.recenterOnToggle;
  base.textLayerEnabled = isBool(r.textLayerEnabled) ? r.textLayerEnabled : base.textLayerEnabled;
  base.textLayerSelectable = isBool(r.textLayerSelectable) ? r.textLayerSelectable : base.textLayerSelectable;
  base.ignoreWatermarkOnCopy = isBool(r.ignoreWatermarkOnCopy) ? r.ignoreWatermarkOnCopy : base.ignoreWatermarkOnCopy;
  base.categoriesEnabled = isBool(r.categoriesEnabled) ? r.categoriesEnabled : base.categoriesEnabled;
  base.showTitlebarPath = isBool(r.showTitlebarPath) ? r.showTitlebarPath : base.showTitlebarPath;
  base.showExtensions = isBool(r.showExtensions) ? r.showExtensions : base.showExtensions;
  base.showIcons = isBool(r.showIcons) ? r.showIcons : base.showIcons;
  base.marqueeTitle = isBool(r.marqueeTitle) ? r.marqueeTitle : base.marqueeTitle;
  base.pathDisplay = oneOf(["show", "hide", "expand"] as const, r.pathDisplay, base.pathDisplay);
  base.thumbSizePx = clampInt(r.thumbSizePx, 40, 400, base.thumbSizePx);
  base.pageIndicatorMode = oneOf(["always", "change", "never"] as const, r.pageIndicatorMode, base.pageIndicatorMode);
  base.pageIndicatorFormat = oneOf(["page", "number"] as const, r.pageIndicatorFormat, base.pageIndicatorFormat);
  base.deleteFolderContents = isBool(r.deleteFolderContents) ? r.deleteFolderContents : base.deleteFolderContents;
  base.themeMode = oneOf(["light", "dark", "system"] as const, r.themeMode, base.themeMode);
  base.consistentPageSize = isBool(r.consistentPageSize) ? r.consistentPageSize : base.consistentPageSize;
  base.consistentDocZoom = isBool(r.consistentDocZoom) ? r.consistentDocZoom : base.consistentDocZoom;
  base.globalZoom = clampFloat(r.globalZoom, MIN_ZOOM, MAX_ZOOM, base.globalZoom);
  base.fitMode = oneOf(["spread", "page", "fill"] as const, r.fitMode, base.fitMode);
  base.flushOnClose = isBool(r.flushOnClose) ? r.flushOnClose : base.flushOnClose;
  base.allowReparentSyncedRoot = isBool(r.allowReparentSyncedRoot) ? r.allowReparentSyncedRoot : base.allowReparentSyncedRoot;
  base.librarySort = oneOf(["manual", "name", "type", "newest", "oldest"] as const, r.librarySort, base.librarySort);

  const colors = r.colors as Record<string, unknown> | undefined;
  for (const theme of ["light", "dark"] as const) {
    const rawTheme = colors?.[theme];
    if (!rawTheme || typeof rawTheme !== "object") continue;
    const themeColors = rawTheme as Record<string, unknown>;
    for (const key of COLOR_KEYS) {
      const value = themeColors[key];
      if (typeof value === "string" && HEX_RE.test(value)) {
        base.colors[theme][key] = value;
      }
    }
  }

  return base;
}
