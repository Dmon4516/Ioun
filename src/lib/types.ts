/* Tipos compartidos de Lyra Viewer */

export type ThemeMode = "light" | "dark" | "system";
export type SurfaceStyle = "frosted" | "opaque";
export type PathDisplay = "show" | "hide" | "expand";
export type FitMode = "spread" | "page" | "fill";
export type ScrollbarMode = "auto" | "always";
export type BottomBarMode = "auto" | "always";
export type TitlebarVisibility = "always" | "never" | "auto" | "fullscreen";
export type PageIndicatorMode = "always" | "change" | "never";
export type PageIndicatorFormat = "page" | "number";
export type LibrarySort = "manual" | "name" | "type" | "newest" | "oldest";
export type DocumentKind = "pdf" | "markdown" | "typst" | "epub";
export type TabStatus = "active" | "idle" | "hibernated" | "restoring";
export type PanelName = "info" | "outline" | "thumbs";

export interface DocumentView {
  pagesPerRow?: 1 | 2 | 3 | 4;
  continuousScroll?: boolean;
}

export interface DocumentRecord {
  id: string;
  path: string;
  title: string;
  kind: DocumentKind;
  pages: number;
  sizeBytes: number | null;
  hash: string | null;
  categoryIds: string[];
  lastPage: number;
  lastZoom: number;
  lastOpened: number | null;
  inRecent: boolean;
  inLibrary: boolean;
  createdAt: number;
  view: DocumentView;
  viewState: Record<string, unknown>;
  position: number;
}

export interface FolderRecord {
  id: string;
  parentId: string | null;
  name: string;
  kind: "temp" | "synced";
  path: string | null;
  expanded: boolean;
  position: number;
  createdAt: number;
}

export interface LibrarySnapshot {
  folders: FolderRecord[];
  rootDocIds: string[];
  folderDocs: Record<string, string[]>;
  /** Subcarpetas de cada carpeta: el contador debe verse sin cargar el árbol. */
  folderChildCounts: Record<string, number>;
}

/** Hijos directos de una carpeta (el árbol se carga bajo demanda). */
export interface FolderChildren {
  folders: FolderRecord[];
}

export interface Bookmark {
  id: string;
  docId: string;
  page: number;
  name: string;
  createdAt: number;
}

export interface Comment {
  id: string;
  docId: string;
  page: number | null;
  text: string;
  createdAt: number;
}

export interface Category {
  id: string;
  name: string;
  color: string;
  position: number;
}

export interface Tab {
  id: string;
  docId: string;
  page: number;
  zoom: number;
  status: TabStatus;
  lastActive: number;
  _restoreToken: number;
}

export interface ColorOverrides {
  accentColor?: string;
  activeBgColor?: string;
  barColor?: string;
  contentColor?: string;
  borderColor?: string;
  iconColor?: string;
  textPrimaryColor?: string;
  textSecondaryColor?: string;
  stateActiveColor?: string;
  stateIdleColor?: string;
  stateHibernatedColor?: string;
  stateRestoringColor?: string;
  /** Fondo del texto seleccionado en la capa de texto. */
  selectionColor?: string;
}

export interface Settings {
  settingsVersion: number;
  ramLimitMB: number;
  maxLoadedTabs: number;
  maxPagesLoaded: number;
  maxActiveFiles: number;
  maxRecentDocs: number;
  maxClosedTabs: number;
  inactiveTimeoutSec: number;
  libraryRefreshDelayMs: number;
  scrollbarMode: ScrollbarMode;
  defaultPanel: PanelName;
  pagesPerRow: 1 | 2 | 3 | 4;
  continuousScroll: boolean;
  surfaceStyle: SurfaceStyle;
  bottomBarMode: BottomBarMode;
  pathDepth: number;
  openLastLocation: boolean;
  titlebarVisibility: TitlebarVisibility;
  recenterOnToggle: boolean;
  categoriesEnabled: boolean;
  showTitlebarPath: boolean;
  showExtensions: boolean;
  showIcons: boolean;
  marqueeTitle: boolean;
  pathDisplay: PathDisplay;
  thumbSizePx: number;
  pageIndicatorMode: PageIndicatorMode;
  pageIndicatorFormat: PageIndicatorFormat;
  deleteFolderContents: boolean;
  themeMode: ThemeMode;
  consistentPageSize: boolean;
  consistentDocZoom: boolean;
  /** Capa de texto seleccionable sobre cada página (prototipo medido). */
  textLayerEnabled: boolean;
  /** Si la capa captura el ratón para poder seleccionar texto. */
  textLayerSelectable: boolean;
  /** Ignorar las líneas repetidas (marca de agua) al copiar texto. */
  ignoreWatermarkOnCopy: boolean;
  globalZoom: number;
  fitMode: FitMode;
  flushOnClose: boolean;
  allowReparentSyncedRoot: boolean;
  librarySort: LibrarySort;
  colors: { light: ColorOverrides; dark: ColorOverrides };
}

export interface TrashItem {
  id: string;
  kind: string;
  label: string;
  createdAt: number;
}

export interface ImportSummary {
  inserted: number;
  skipped: number;
}

export interface ColorDef {
  key: keyof ColorOverrides;
  label: string;
  cssVar: string | null;
  light: string;
  dark: string;
  alpha?: number;
  group: "interface" | "text" | "state";
}
