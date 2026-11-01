<script lang="ts">
  import { onMount } from "svelte";
  import Titlebar from "./Titlebar/Titlebar.svelte";
  import Sidebar from "./Sidebar/Sidebar.svelte";
  import Viewer from "./Viewer/Viewer.svelte";
  import Inspector from "./Inspector/Inspector.svelte";
  import FloatingToolbar from "./Toolbar/FloatingToolbar.svelte";
  import SyncPill from "./Toolbar/SyncPill.svelte";
  import SearchPopup from "./Search/SearchPopup.svelte";
  import SettingsModal from "./Settings/SettingsModal.svelte";
  import ContextMenus from "./Sidebar/ContextMenus.svelte";
  import TrashPanel from "./Trash/TrashPanel.svelte";
  import PageIndicator from "./Viewer/PageIndicator.svelte";
  import { installPdfCopyNormalization } from "$lib/pdf/copySelection";
  import {
    ui,
    contextMenu,
    hideContextMenu,
    markActivity,
    sidebarPeek,
  } from "$lib/stores/ui.svelte";
  import { settings, applyAppearance, hydrateSettings } from "$lib/stores/settings.svelte";
  import {
    tabs, closeTab, setActiveTab, restoreLastClosedTab, enforceLimits,
    openDocument, enforceMemoryLimit, hibernateInactiveTabs, flushPendingProgress,
  } from "$lib/stores/tabs.svelte";
  import { isTypingTarget } from "$lib/utils/keyboard";
  import { SCROLL_STEP_FINE, SCROLL_STEP_PAGE } from "$lib/constants";
  import {
    commandZoomToCenter,
    scrollViewerBy,
    viewer,
  } from "$lib/stores/viewer.svelte";
  import { requestScrollToPage } from "$lib/stores/viewerCommands.svelte";
  import {
    onDbInitFailed,
    onWatcherInitFailed,
    onFileDrop,
    onFullscreenChange,
    onLibraryChanged,
    onExternalFileOpen,
    onSyncPaused,
    onSyncProgress,
  } from "$lib/tauri/events";
  import { applySyncPaused, applySyncProgress } from "$lib/stores/sync.svelte";
  import {
    addFile,
    drainPendingExternalFiles,
    isSyncPaused,
    listCategories,
    listDocuments,
  } from "$lib/tauri/commands";
  import { reloadLibrary } from "$lib/stores/library.svelte";
  import { documents, reconcileDocuments, setCategories, upsertDocument } from "$lib/stores/documents.svelte";
  import { showToast } from "$lib/stores/toast.svelte";
  import { isSupportedDocument } from "$lib/utils/fileExtensions";
  import { flushSettings } from "$lib/stores/settings.svelte";
  import { handleCloseRequested } from "$lib/window/closeHandler";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Toast from "./Toast.svelte";

  let errorMessage = $state("");

  /** Holgura horizontal de la franja del título, como en la barra flotante. */
  const TITLEBAR_HOVER_SLACK = 40;
  /** Margen extra antes de ocultar el título (evita parpadeo en el borde). */
  const TITLEBAR_HOVER_HYSTERESIS = 16;

  // ── Sistema de auto-ocultado de la barra ──────────────────────────
  let toolbarHover = false;
  let nearToolbarZone = false;
  let toolbarTimer = 0;
  let lastActivityWrite = 0;
  let toolbarVertical: boolean | null = null;

  function scheduleToolbarHide() {
    clearTimeout(toolbarTimer);
    if (settings.bottomBarMode === "always") {
      ui.toolbarHidden = false;
      return;
    }
    toolbarTimer = window.setTimeout(() => {
      if (toolbarHover || nearToolbarZone) return;
      if (ui.settingsOpen || ui.searchOpen) return;
      if (isTypingTarget(document.activeElement)) return;
      ui.toolbarHidden = true;
    }, 2400);
  }

  function showToolbar() {
    ui.toolbarHidden = false;
    scheduleToolbarHide();
  }

  /**
   * Zona en la que se revela el título oculto. El título es una píldora de
   * ancho ajustado al contenido, así que la zona es su franja real (no toda la
   * banda superior de la ventana). Se usan `offset*` porque con el título oculto
   * su `transform` lo saca de pantalla y su rect ya no sirve.
   */
  function titlebarBand(): { top: number; bottom: number; left: number; right: number } | null {
    const titlebar = document.getElementById("titlebarEl");
    if (!titlebar) return null;
    return {
      top: 0,
      bottom: titlebar.offsetTop + titlebar.offsetHeight,
      left: Math.max(0, titlebar.offsetLeft - TITLEBAR_HOVER_SLACK),
      right: Math.min(window.innerWidth, titlebar.offsetLeft + titlebar.offsetWidth + TITLEBAR_HOVER_SLACK),
    };
  }

  function pointerInTitlebarBand(
    e: PointerEvent,
    band: { top: number; bottom: number; left: number; right: number },
    horizontalSlack: number,
  ): boolean {
    const toolbar = document.getElementById("floatingToolbar");
    if (toolbar) {
      const toolbarRect = toolbar.getBoundingClientRect();
      if (
        e.clientX >= toolbarRect.left &&
        e.clientX <= toolbarRect.right &&
        e.clientY >= toolbarRect.top &&
        e.clientY <= toolbarRect.bottom
      ) {
        return false;
      }
    }

    return (
      e.clientY >= band.top &&
      e.clientY <= band.bottom &&
      e.clientX >= band.left - horizontalSlack &&
      e.clientX <= band.right + horizontalSlack
    );
  }

  // El handler se ejecuta como mucho una vez por fotograma: `pointermove` dispara
  // a más frecuencia que los refrescos y cada llamada hace `getComputedStyle` y
  // lecturas de layout (que fuerzan recálculo síncrono).
  let pendingPointerEvent: PointerEvent | null = null;
  let pointerFrame = 0;

  function onPointerMove(e: PointerEvent): void {
    pendingPointerEvent = e;
    if (pointerFrame) return;
    pointerFrame = requestAnimationFrame(() => {
      pointerFrame = 0;
      const event = pendingPointerEvent;
      pendingPointerEvent = null;
      if (event) applyPointerMove(event);
    });
  }

  function applyPointerMove(e: PointerEvent) {
    const fullscreen = document.documentElement.dataset.fullscreen === "true";
    const titlebarAutoHidden =
      settings.titlebarVisibility === "always" ||
      (settings.titlebarVisibility === "auto" && ui.reading) ||
      (settings.titlebarVisibility === "fullscreen" && fullscreen);
    if (titlebarAutoHidden) {
      // Histéresis: se revela al entrar en la franja y se oculta al salir con
      // holgura, para que no parpadee en el borde. La franja se calcula una vez
      // por fotograma, no una vez por consulta.
      const band = titlebarBand();
      if (band && pointerInTitlebarBand(e, band, 0)) {
        document.documentElement.dataset.titlebarHover = "true";
      } else if (!band || !pointerInTitlebarBand(e, band, TITLEBAR_HOVER_HYSTERESIS)) {
        delete document.documentElement.dataset.titlebarHover;
      }
    } else {
      delete document.documentElement.dataset.titlebarHover;
    }

    if (settings.bottomBarMode === "always") return;
    const toolbar = document.getElementById("floatingToolbar");
    if (!toolbar) return;
    const rect = toolbar.getBoundingClientRect();
    if (toolbarVertical === null) {
      toolbarVertical = getComputedStyle(toolbar).flexDirection === "column";
    }
    const vertical = toolbarVertical;
    if (vertical) {
      const leftBound = rect.left - 40;
      const rightBound = rect.right + 40;
      if (e.clientX >= leftBound && e.clientX <= rightBound) {
        nearToolbarZone = true;
        showToolbar();
      } else if (nearToolbarZone) {
        nearToolbarZone = false;
        scheduleToolbarHide();
      }
    } else {
      if (e.clientY >= window.innerHeight - 90) {
        nearToolbarZone = true;
        showToolbar();
      } else if (nearToolbarZone) {
        nearToolbarZone = false;
        scheduleToolbarHide();
      }
    }
  }

  function onPointerMoveActivity() {
    onActivityThrottled();
  }

  function onActivityThrottled() {
    const now = performance.now();
    if (now - lastActivityWrite < 1000) return;
    lastActivityWrite = now;
    markActivity();
  }

  function onSidebarEdgeLeave(e: MouseEvent): void {
    const sidebar = document.querySelector(".sidebar");
    if (!(e.relatedTarget instanceof Node) || !sidebar?.contains(e.relatedTarget)) {
      sidebarPeek.active = false;
    }
  }

  // ── Actividad global ──────────────────────────────────────────────
  function onActivity() {
    markActivity();
    showToolbar();
  }

  // ── Atajos de teclado globales ────────────────────────────────────
  function onKeyDown(e: KeyboardEvent) {
    onActivityThrottled();
    const mod = e.metaKey || e.ctrlKey;
    const key = e.key.toLowerCase();
    const typing = isTypingTarget(e.target);

    // Escape: cerrar capas
    if (e.key === "Escape") {
      if (contextMenu.open) { hideContextMenu(); return; }
      if (ui.trashOpen) { ui.trashOpen = false; return; }
      if (ui.settingsOpen) { ui.settingsOpen = false; return; }
      if (ui.searchOpen) { ui.searchOpen = false; return; }
      if (ui.reading) { ui.reading = false; return; }
      return;
    }

    // Buscar
    if (mod && key === "k") {
      e.preventDefault();
      ui.searchOpen = true;
      return;
    }
    if (!typing && !mod && e.key === "/") {
      e.preventDefault();
      ui.searchOpen = true;
      return;
    }

    if (ui.settingsOpen) return;

    if (!typing && !mod && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      const tab = tabs.items.find((item) => item.id === tabs.activeId);
      const doc = tab ? documents.items.get(tab.docId) : null;
      if (!tab || !doc || tab.status === "restoring") return;
      e.preventDefault();
      const upper = doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
      const nextPage = Math.max(1, Math.min(upper, tab.page + (e.key === "ArrowRight" ? 1 : -1)));
      if (nextPage !== tab.page) {
        // El cambio de página siempre empieza en el borde superior: así nunca
        // queda visible el final de la página anterior, también con zoom alto.
        requestScrollToPage(nextPage, true, "top");
      }
      return;
    }

    // Desplazamiento del visor: las flechas solas dan un paso fino; con Shift,
    // una pantalla. (Los botones arriba/abajo de la barra usan el paso de
    // pantalla.)
    if (
      !typing &&
      !mod &&
      !viewer.zooming &&
      (e.key === "ArrowUp" || e.key === "ArrowDown")
    ) {
      const area = document.getElementById("viewerArea");
      if (area) {
        e.preventDefault();
        const step = e.shiftKey ? SCROLL_STEP_PAGE : SCROLL_STEP_FINE;
        scrollViewerBy(e.key === "ArrowDown" ? 1 : -1, step.fraction, step.minPx);
      }
      return;
    }

    if (!typing && !mod) {
      const tab = tabs.items.find((item) => item.id === tabs.activeId);
      const doc = tab ? documents.items.get(tab.docId) : null;
      if (tab && doc && tab.status !== "restoring") {
        const perRow = doc.view.pagesPerRow ?? settings.pagesPerRow;
        let target: number | null = null;
        if (e.key === "PageDown") target = tab.page + perRow;
        else if (e.key === "PageUp") target = tab.page - perRow;
        else if (e.key === "Home") target = 1;
        else if (e.key === "End") target = doc.pages;
        if (target !== null) {
          e.preventDefault();
          const upper = doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
          const page = Math.max(1, Math.min(upper, target));
          if (page !== tab.page) {
            requestScrollToPage(page, true);
          }
          return;
        }
      }
    }

    if (mod && ["=", "+", "-", "_"].includes(e.key)) {
      e.preventDefault();
      const tab = tabs.items.find((item) => item.id === tabs.activeId);
      if (tab) {
        commandZoomToCenter(
          tab.zoom * (e.key === "-" || e.key === "_" ? 1 / 1.15 : 1.15),
        );
      }
      return;
    }

    // Cerrar pestaña
    if (mod && key === "w") {
      e.preventDefault();
      if (tabs.activeId) closeTab(tabs.activeId);
      return;
    }
    // Inspector info
    if (mod && key === "i") {
      e.preventDefault();
      ui.inspector = ui.inspector === "info" ? null : "info";
      return;
    }
    // Sidebar
    if (mod && key === "b") {
      e.preventDefault();
      ui.showSidebar = !ui.showSidebar;
      return;
    }
    // Modo lectura
    if (mod && e.shiftKey && key === "l") {
      e.preventDefault();
      ui.reading = !ui.reading;
      if (ui.reading) ui.inspector = null;
      return;
    }
    // Restaurar cerrada
    if (mod && e.shiftKey && key === "t") {
      e.preventDefault();
      restoreLastClosedTab();
      return;
    }
    // Ctrl+1..9
    if (mod && /^[1-9]$/.test(key)) {
      e.preventDefault();
      const idx = Number(key) - 1;
      const tab = tabs.items[idx];
      if (tab) setActiveTab(tab.id);
      return;
    }
  }

  async function importExternalFiles(paths: string[]): Promise<void> {
    const failures: string[] = [];
    const openedIds: string[] = [];
    for (const path of paths) {
      if (!isSupportedDocument(path)) continue;
      try {
        const doc = await addFile(path, null);
        upsertDocument(doc);
        openedIds.push(doc.id);
      } catch (error) {
        failures.push(`${path}: ${String(error)}`);
      }
    }
    for (const id of openedIds) openDocument(id, false);
    enforceLimits();
    if (openedIds.length > 0 || failures.length > 0) {
      try {
        const [, nextDocuments] = await Promise.all([reloadLibrary(), listDocuments()]);
        reconcileDocuments(nextDocuments);
      } catch (error) {
        failures.push(String(error));
      }
    }
    if (failures.length > 0) {
      console.error("[external-open] Fallos al abrir archivos externos:", failures);
      showToast(
        failures.length === 1
          ? "No se pudo abrir el archivo externo."
          : `No se pudieron abrir ${failures.length} archivos externos.`,
      );
    }
    if (failures.length === 0 && openedIds.length > 0) {
      showToast(
          openedIds.length === 1
            ? "Archivo añadido y abierto."
            : `${openedIds.length} PDF añadidos y abiertos.`,
      );
    }
  }


  // ── Init + listeners ──────────────────────────────────────────────
  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenFullscreen: (() => void) | undefined;
    let unlistenLibrary: (() => void) | undefined;
    let unlistenSync: (() => void) | undefined;
    let unlistenSyncPaused: (() => void) | undefined;
    let unlistenDb: (() => void) | undefined;
    let unlistenWatcher: (() => void) | undefined;
    let unlistenExternal: (() => void) | undefined;
    let unlistenClose: (() => void) | undefined;
    let libraryChangedTimer = 0;
    let libraryRefreshing = false;
    let libraryDirty = false;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onSystemThemeChange = () => {
      if (settings.themeMode === "system") applyAppearance();
    };
    media.addEventListener?.("change", onSystemThemeChange);

    /**
     * Una sola cola de refresco para `library-changed` y para el final del sync:
     * el watcher emite en ráfaga y varios `done` seguidos refrescarían de más.
     */
    function scheduleLibraryRefresh(delay = settings.libraryRefreshDelayMs): void {
      clearTimeout(libraryChangedTimer);
      if (libraryRefreshing) {
        libraryDirty = true;
        return;
      }
      libraryChangedTimer = window.setTimeout(runLibraryRefresh, delay);
    }

    async function runLibraryRefresh(): Promise<void> {
      if (libraryRefreshing) return;
      libraryRefreshing = true;
      try {
        const storedDocuments = await listDocuments();
        reconcileDocuments(storedDocuments);
        await reloadLibrary();
      } catch (error) {
        console.error("[library] No se pudo actualizar la biblioteca", error);
      } finally {
        libraryRefreshing = false;
        if (libraryDirty) {
          libraryDirty = false;
          scheduleLibraryRefresh();
        }
      }
    }
    const uninstallCopyNormalization = installPdfCopyNormalization();
    const onActivityWrap = () => onActivity();
    window.addEventListener("pointerdown", onActivityWrap, { passive: true });
    window.addEventListener("pointermove", onPointerMoveActivity, { passive: true });
    window.addEventListener("pointermove", onPointerMove, { passive: true });
    const invalidateToolbarLayout = () => { toolbarVertical = null; };
    window.addEventListener("resize", invalidateToolbarLayout);
    window.addEventListener("keydown", onKeyDown);
    scheduleToolbarHide();

    void onDbInitFailed((message) => {
      errorMessage = `No se pudo iniciar la base de datos: ${message}. La aplicación funcionará en modo limitado hasta reiniciar.`;
    })
      .then((fn) => {
        if (disposed) fn();
        else unlistenDb = fn;
      })
      .catch((error: unknown) => {
        console.warn("[lyra] No se pudo escuchar el estado de la base de datos", error);
      });

    void onWatcherInitFailed((message) => {
      errorMessage = `No se pudo iniciar la vigilancia de archivos: ${message}. La biblioteca seguirá disponible, pero los cambios externos requerirán sincronización manual.`;
    })
      .then((fn) => {
        if (disposed) fn();
        else unlistenWatcher = fn;
      })
      .catch((error: unknown) => {
        console.warn("[lyra] No se pudo escuchar el estado del watcher", error);
      });

    // `getCurrentWindow()` lanza de forma síncrona fuera de Tauri (modo web):
    // sin el try/catch, la excepción abortaría el resto del arranque.
    try {
      const appWindow = getCurrentWindow();
      void appWindow
        .onCloseRequested(async (event) => {
          await handleCloseRequested(
            event,
            settings.flushOnClose,
            flushSettings,
            flushPendingProgress,
            () => appWindow.destroy(),
          );
        })
        .then((fn) => {
          if (disposed) fn();
          else unlistenClose = fn;
        })
        .catch((error: unknown) => {
          console.warn("[lyra] No se pudo interceptar el cierre de la ventana", error);
        });
    } catch (error) {
      console.warn("[lyra] No se pudo interceptar el cierre de la ventana", error);
    }

    void onFullscreenChange((fullscreen) => {
      document.documentElement.dataset.fullscreen = String(fullscreen);
    })
      .then((fn) => {
        if (disposed) fn();
        else unlistenFullscreen = fn;
      })
      .catch((error: unknown) => {
        console.warn("[window] No se pudo seguir el estado de pantalla completa", error);
      });
    void onSyncProgress((progress) => {
      applySyncProgress(progress);
      if (progress.phase === "done") scheduleLibraryRefresh();
      else if (progress.phase === "error") {
        showToast(`No se pudo sincronizar: ${progress.message ?? "error desconocido"}`);
      }
    })
      .then((fn) => {
        if (disposed) fn();
        else unlistenSync = fn;
      })
      .catch((error: unknown) => {
        console.warn("[lyra] No se pudo escuchar el progreso del sync", error);
      });
    void onSyncPaused((paused) => {
      applySyncPaused(paused);
    })
      .then((fn) => {
        if (disposed) fn();
        else unlistenSyncPaused = fn;
      })
      .catch((error: unknown) => {
        console.warn("[lyra] No se pudo escuchar el estado pausado del sync", error);
      });
    void isSyncPaused()
      .then(applySyncPaused)
      .catch((error: unknown) => {
        console.warn("[lyra] No se pudo consultar el estado pausado del sync", error);
      });

    void onLibraryChanged(() => {
      // El watcher puede emitir en ráfaga: agrupamos y evitamos solaparnos.
      scheduleLibraryRefresh();
    })
      .then((fn) => {
        if (disposed) fn();
        else unlistenLibrary = fn;
      })
      .catch((error: unknown) => {
        console.warn("[library] No se pudo escuchar la sincronización", error);
      });

    void (async () => {
      try {
        await hydrateSettings();
        const [storedDocuments, categories] = await Promise.all([
          listDocuments(),
          listCategories(),
        ]);
        if (disposed) return;

        reconcileDocuments(storedDocuments);
        setCategories(categories);
        await reloadLibrary();

        try {
          const externalListener = await onExternalFileOpen((path) => {
            void importExternalFiles([path]);
          });
          if (disposed) externalListener();
          else unlistenExternal = externalListener;

          const pendingExternal = await drainPendingExternalFiles();
          if (pendingExternal.length > 0) {
            await importExternalFiles(pendingExternal);
          }
        } catch (error) {
          console.warn("[lyra] No se pudo configurar la apertura de archivos externos", error);
        }

        unlisten = await onFileDrop(async ({ paths }) => {
          await importExternalFiles(paths);
        });
        if (disposed) unlisten();
      } catch (error) {
        if (!disposed) {
          errorMessage = `No se pudo iniciar la aplicación: ${String(error)}`;
          console.error("[bootstrap]", error);
        }
      }
    })();

    return () => {
      disposed = true;
      window.removeEventListener("pointerdown", onActivityWrap);
      window.removeEventListener("pointermove", onPointerMoveActivity);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("resize", invalidateToolbarLayout);
      if (pointerFrame) cancelAnimationFrame(pointerFrame);
      window.removeEventListener("keydown", onKeyDown);
      media.removeEventListener?.("change", onSystemThemeChange);
      uninstallCopyNormalization();
      clearTimeout(toolbarTimer);
      clearTimeout(libraryChangedTimer);
      unlisten?.();
      unlistenFullscreen?.();
      unlistenLibrary?.();
      unlistenSync?.();
      unlistenSyncPaused?.();
      unlistenDb?.();
      unlistenWatcher?.();
      unlistenExternal?.();
      unlistenClose?.();
    };
  });

  $effect(() => {
    void settings.themeMode;
    void settings.surfaceStyle;
    void settings.pathDisplay;
    void settings.showIcons;
    void settings.showTitlebarPath;
    void settings.titlebarVisibility;
    void settings.colors.light;
    void settings.colors.dark;
    applyAppearance();
  });

  $effect(() => {
    void settings.titlebarVisibility;
    void ui.reading;
    const titlebar = document.getElementById("titlebarEl");
    if (!titlebar) return;

    const updateOffset = () => {
      const fullscreen = document.documentElement.dataset.fullscreen === "true";
      const gap =
        Number.parseFloat(
          getComputedStyle(document.documentElement).getPropertyValue("--gap-edge"),
        ) || 10;
      const autoHidden =
        settings.titlebarVisibility === "always" ||
        (settings.titlebarVisibility === "auto" && ui.reading) ||
        (settings.titlebarVisibility === "fullscreen" && fullscreen);
      const revealed = document.documentElement.dataset.titlebarHover === "true";
      // Geometría sin `transform`: el título oculto se desplaza con translateY,
      // así que su rect no sirve para medir la franja.
      const height = titlebar.offsetHeight
        ? Math.ceil(titlebar.offsetTop + titlebar.offsetHeight + gap)
        : 0;
      // Los paneles laterales dejan libre la franja mientras el título está a la
      // vista (visible o revelado con el puntero), y la recuperan cuando se oculta.
      const reserve = height && (!autoHidden || revealed) ? `${height}px` : "0px";
      document.documentElement.style.setProperty("--titlebar-reserve", reserve);
      // El área del documento solo reserva cuando el título está visible de forma
      // estable. Si contara el revelado con el puntero, el PDF se desplazaría.
      const offset = height && !autoHidden ? `${height}px` : "0px";
      document.documentElement.style.setProperty("--titlebar-offset", offset);
    };

    updateOffset();
    const observer = new ResizeObserver(updateOffset);
    observer.observe(titlebar);
    const visibilityObserver = new MutationObserver(updateOffset);
    visibilityObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-fullscreen", "data-titlebar-hover"] });
    window.addEventListener("resize", updateOffset);
    return () => {
      observer.disconnect();
      visibilityObserver.disconnect();
      window.removeEventListener("resize", updateOffset);
    };
  });

  // Aplicar clases al body según el estado UI
  $effect(() => {
    document.body.classList.toggle("no-sidebar", !ui.showSidebar);
    document.body.classList.toggle("sidebar-peek", sidebarPeek.active);
    document.body.classList.toggle("reading", ui.reading);
    document.body.classList.toggle("inspector-open", ui.inspector !== null);
    document.body.classList.toggle("inspector-outline-open", ui.inspector === "outline");
    document.body.classList.toggle("toolbar-hidden", ui.toolbarHidden);
    document.body.classList.toggle("search-open", ui.searchOpen);
  });

  $effect(() => {
    if (ui.showSidebar || ui.reading) sidebarPeek.active = false;
  });

  // Auto-hide de la toolbar ante cambios de settings
  $effect(() => {
    void settings.bottomBarMode;
    if (settings.bottomBarMode === "always") {
      ui.toolbarHidden = false;
      clearTimeout(toolbarTimer);
    } else {
      scheduleToolbarHide();
    }
  });

  // Housekeeping: expulsar tabs excedentes cuando cambian límites
  $effect(() => {
    void settings.maxActiveFiles;
    void settings.maxLoadedTabs;
    void settings.ramLimitMB;
    enforceLimits();
  });

  // `--content-left`/`--content-right` se definen en `base.css` a partir de
  // `--rail-width`/`--sidebar-width`/`--inspector-*-width` y de las clases de
  // `body` (sin barra, modo lectura, panel abierto…). Eso es la única fuente de
  // verdad: aquí había además una medición con `ResizeObserver` que escribía esas
  // variables en `:root`, donde no las ve nadie (para el subárbol de `body` gana
  // la declaración de `body`), y cuyo valor además contradiría al CSS en el estado
  // de «asomo» de la barra (que se superpone al contenido sin desplazarlo).

  $effect(() => {
    void settings.inactiveTimeoutSec;
    void settings.ramLimitMB;
    const interval = window.setInterval(() => {
      // Sin pestañas cargadas no hay nada que hibernar: mejor no recorrer los
      // límites cada 15 s en una app abierta sin documentos.
      if (!tabs.items.some((tab) => tab.status !== "hibernated")) return;
      hibernateInactiveTabs();
      enforceMemoryLimit();
    }, 15_000);
    return () => clearInterval(interval);
  });
</script>

<div class="app-shell">
  <Titlebar />
  <PageIndicator />

  <div class="main">
    {#if !ui.reading}
      <Sidebar />
    {/if}

    <Viewer />

    {#if ui.inspector && !ui.reading}
      <Inspector />
    {/if}
  </div>

  <div
    class="sidebar-edge"
    aria-hidden="true"
    onmouseenter={() => {
      if (!ui.showSidebar && !ui.reading) sidebarPeek.active = true;
    }}
    onmouseleave={onSidebarEdgeLeave}
  ></div>

  <div
    role="presentation"
    onmouseenter={() => { toolbarHover = true; showToolbar(); }}
    onmouseleave={() => { toolbarHover = false; nearToolbarZone = false; scheduleToolbarHide(); }}
  >
    <FloatingToolbar />
  </div>

  <SyncPill />

  {#if ui.searchOpen}
    <SearchPopup />
  {/if}

  <SettingsModal />

  {#if errorMessage}
    <div class="app-error" role="alert">
      <span>{errorMessage}</span>
      <button type="button" aria-label="Cerrar aviso" onclick={() => (errorMessage = "")}>×</button>
    </div>
  {/if}

  {#if contextMenu.open}
    <ContextMenus
      items={contextMenu.items}
      x={contextMenu.x}
      y={contextMenu.y}
      onClose={hideContextMenu}
    />
  {/if}

  <TrashPanel />

  <Toast />
</div>

<style>
  .app-shell {
    position: relative;
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-app);
  }

  .main {
    flex: 1;
    display: flex;
    min-height: 0;
    position: relative;
  }

  .sidebar-edge {
    position: absolute;
    top: max(var(--gap-edge), var(--titlebar-reserve));
    left: 0;
    bottom: 0;
    width: 16px;
    z-index: 86;
    display: none;
  }

  :global(body.no-sidebar:not(.reading)) .sidebar-edge {
    display: block;
  }

  .app-error {
    position: absolute;
    z-index: 120;
    right: 16px;
    bottom: 16px;
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: min(560px, calc(100vw - 32px));
    padding: 10px 12px;
    border: 1px solid var(--danger);
    border-radius: var(--radius-card);
    background: var(--bg-menu);
    color: var(--text-primary);
    box-shadow: var(--shadow);
    font-size: 13px;
  }

  .app-error button {
    border: 0;
    background: transparent;
    color: var(--text-secondary);
    font-size: 18px;
  }
</style>
