<script lang="ts">
  import { clickOutside, scrollbar } from "$lib/actions";
  import { ui, sidebarPeek, sidebarView } from "$lib/stores/ui.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import FileTree from "./FileTree.svelte";
  import CategoryView from "./CategoryView.svelte";
  import RecentList from "./RecentList.svelte";
  import Icon from "$components/Icon.svelte";
  import {
    openPdfDialog, openFolderDialog, addFile, addFolder, isSyncPaused,
    restoreDocumentToLibrary, syncSyncedFolders,
  } from "$lib/tauri/commands";
  import { documents, upsertDocument } from "$lib/stores/documents.svelte";
  import { reloadLibrary } from "$lib/stores/library.svelte";
  import { showToast } from "$lib/stores/toast.svelte";
  import { confirmDialog } from "$lib/tauri/dialogs";
  import type { DocumentRecord } from "$lib/types";
  import {
    clearRecentHistory,
    deactivateActiveTab,
    getActiveTab,
    openDocument,
    restoreLastClosedTab,
    enforceLimits,
    tabs,
  } from "$lib/stores/tabs.svelte";

  let libraryCollapsed = $state(false);
  let recentExpanded = $state(false);
  let menuOpen = $state<null | "add">(null);
  let addMenuEl: HTMLElement | null = $state(null);
  let importingFiles = $state(false);
  let importProgress = $state({ complete: 0, total: 0, failed: 0 });

  async function refreshLibrary() {
    await reloadLibrary();
  }

  async function handleAddFile() {
    menuOpen = null;
    try {
      const paths = await openPdfDialog();
      if (!paths?.length) return;
      importingFiles = true;
      importProgress = { complete: 0, total: paths.length, failed: 0 };
      const concurrency = 3;
      let nextIndex = 0;
      const added = new Array<DocumentRecord | null>(paths.length).fill(null);
      const workers = Array.from({ length: Math.min(concurrency, paths.length) }, async () => {
        while (nextIndex < paths.length) {
          const index = nextIndex++;
          const path = paths[index];
          if (!path) continue;
          try {
            const doc = await addFile(path, null);
            upsertDocument(doc);
            added[index] = doc;
          } catch (error) {
            importProgress.failed++;
            console.error(`[sidebar] No se pudo añadir ${path}`, error);
          } finally {
            importProgress.complete++;
          }
        }
      });
      await Promise.all(workers);
      for (const doc of added) {
        if (doc) openDocument(doc.id, false);
      }
      enforceLimits();
      await refreshLibrary();
      showToast(importProgress.failed
        ? `Abiertos ${importProgress.complete - importProgress.failed} de ${paths.length} PDF.`
        : `${paths.length} PDF añadidos y abiertos.`);
    } catch (err) {
      console.error("[sidebar] No se pudo añadir el archivo", err);
      showToast(`Error al añadir: ${String(err)}`);
    } finally {
      importingFiles = false;
      importProgress = { complete: 0, total: 0, failed: 0 };
    }
  }

  async function handleAddFolder(kind: "temp" | "synced") {
    menuOpen = null;
    try {
      if (kind === "synced") {
        const path = await openFolderDialog();
        if (!path) return;
        const name = path.replace(/[\\/]+$/, "").split(/[\\/]/).at(-1) || "Carpeta sincronizada";
        await addFolder(name, kind, null, path);
        await refreshLibrary();
        if (await isSyncPaused()) {
          showToast(
            "Carpeta vinculada. La sincronización sigue pausada; pulsa «Reanudar» cuando quieras.",
          );
        } else {
          showToast("Carpeta vinculada; se sincroniza mientras Ioun esté abierto.");
          void syncSyncedFolders();
        }
        return;
      } else {
        await addFolder("Nueva carpeta", kind, null);
      }
      await refreshLibrary();
      showToast("Carpeta añadida.");
    } catch (err) {
      console.error("[sidebar] No se pudo añadir la carpeta", err);
      showToast(`Error: ${String(err)}`);
    }
  }

  function toggleAddMenu() {
    menuOpen = menuOpen === "add" ? null : "add";
  }

  $effect(() => {
    if (menuOpen === "add") addMenuEl?.focus({ preventScroll: true });
  });

  const collapseChevron = $derived(libraryCollapsed);
  const canRestore = $derived(tabs.recentlyClosed.length > 0);
  const hasActiveTab = $derived(getActiveTab() !== null);

  async function handleClearRecent(): Promise<void> {
    if (!(await confirmDialog("¿Borrar la lista de recientes?"))) return;
    try {
      await clearRecentHistory();
      showToast("Recientes borrados.");
    } catch (error) {
      console.error("[sidebar] No se pudieron borrar los recientes", error);
      showToast("No se pudieron borrar los recientes.");
    }
  }

  async function handleRecentOpen(docId: string): Promise<void> {
    recentExpanded = false;
    libraryCollapsed = false;
    sidebarView.mode = "tabs";
    const doc = documents.items.get(docId);
    try {
      if (doc && !doc.inLibrary) {
        await restoreDocumentToLibrary(docId);
        documents.items.set(docId, { ...doc, inLibrary: true });
        await refreshLibrary();
      }
      openDocument(docId);
    } catch (error) {
      console.error("[sidebar] No se pudo abrir el documento reciente", error);
      showToast("No se pudo abrir el documento.");
    }
  }

  $effect(() => {
    if (!settings.categoriesEnabled && sidebarView.mode === "categories") {
      sidebarView.mode = "tabs";
    }
  });
</script>

<aside
  class="sidebar"
  onmouseleave={() => {
    if (!ui.showSidebar) sidebarPeek.active = false;
  }}
>
  <div
    class="sidebar-scroll custom-scroll"
    use:scrollbar={{ always: settings.scrollbarMode === "always" }}
  >
    <div class="section-header files-header">
      <span class="section-title">Archivos</span>
      <div class="section-header-actions">
        {#if settings.categoriesEnabled}
          <button
            class="mini-action"
            type="button"
            title={sidebarView.mode === "tabs" ? "Ver por categoría" : "Ver como archivos"}
            aria-label="Cambiar vista"
            onclick={() =>
              (sidebarView.mode = sidebarView.mode === "tabs" ? "categories" : "tabs")}
          >
            <Icon name={sidebarView.mode === "tabs" ? "layers" : "grid"} size={14} />
          </button>
        {/if}
        <button
          class="mini-action"
          type="button"
          title="Papelera"
          aria-label="Abrir papelera"
          onclick={() => (ui.trashOpen = true)}
        >
          <Icon name="trash" size={14} />
        </button>
        <button
          class="mini-action"
          type="button"
          title="Poner documento activo en inactivo"
          aria-label="Poner documento activo en inactivo"
          disabled={!hasActiveTab}
          onclick={deactivateActiveTab}
        >
          <Icon name="refresh" size={14} />
        </button>
        <button
          class="mini-action"
          type="button"
          title="Añadir archivo o carpeta"
          aria-label="Añadir"
          onclick={toggleAddMenu}
        >
          <Icon name="plus" size={14} />
        </button>
        <button
          class="section-chevron"
          class:collapsed={collapseChevron}
          type="button"
          title={libraryCollapsed ? "Expandir archivos" : "Contraer archivos"}
          aria-label="Expandir/contraer archivos"
          onclick={() => (libraryCollapsed = !libraryCollapsed)}
        >
          <Icon name="chevron-down" size={14} />
        </button>
      </div>
    </div>

    {#if menuOpen === "add"}
      <div
        class="add-menu"
        bind:this={addMenuEl}
        role="menu"
        tabindex="-1"
        use:clickOutside={() => (menuOpen = null)}
        onkeydown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            menuOpen = null;
          }
        }}
      >
        <button class="add-menu-item" type="button" disabled={importingFiles} onclick={handleAddFile}>
          <Icon name="file" size={14} /> Abrir archivo
        </button>
        <button class="add-menu-item" type="button" disabled={importingFiles} onclick={() => handleAddFolder("temp")}>
          <Icon name="folder" size={14} /> Carpeta temporal
        </button>
        <button class="add-menu-item" type="button" disabled={importingFiles} onclick={() => handleAddFolder("synced")}>
          <Icon name="folder" size={14} /> Carpeta sincronizada
        </button>
      </div>
    {/if}
    {#if importingFiles}
      <div class="import-progress" role="status" aria-live="polite">
        Procesando PDF: {importProgress.complete}/{importProgress.total}
      </div>
    {/if}

    {#if !libraryCollapsed}
      {#if sidebarView.mode === "categories" && settings.categoriesEnabled}
        <CategoryView />
      {:else}
        <FileTree />
      {/if}
    {/if}

    <div class="section-header">
      <span class="section-title">Recientes</span>
      <div class="section-header-actions">
        {#if recentExpanded}
          <div class="recent-header-actions">
            <button
              class="mini-action"
              type="button"
              title="Restaurar pestaña cerrada"
              aria-label="Restaurar pestaña cerrada"
              disabled={!canRestore}
              onclick={restoreLastClosedTab}
            >
              <Icon name="rotate-ccw" size={14} />
            </button>
            <button
              class="mini-action"
              type="button"
              title="Borrar recientes"
              aria-label="Borrar recientes"
              onclick={() => void handleClearRecent()}
            >
              <Icon name="trash" size={14} />
            </button>
          </div>
        {/if}
        <button
          class="section-chevron"
          class:collapsed={!recentExpanded}
          type="button"
          title={recentExpanded ? "Contraer recientes" : "Expandir recientes"}
          aria-label="Expandir/contraer recientes"
          onclick={() => (recentExpanded = !recentExpanded)}
        >
          <Icon name="chevron-down" size={14} />
        </button>
      </div>
    </div>

    {#if recentExpanded}
      <RecentList
        onOpen={(docId) => void handleRecentOpen(docId)}
      />
    {/if}
  </div>
</aside>

<style>
  .sidebar {
    position: absolute;
    top: max(var(--gap-edge), var(--titlebar-reserve));
    left: var(--gap-edge);
    bottom: var(--gap-edge);
    width: var(--sidebar-width);
    display: flex;
    flex-direction: column;
    background: var(--custom-bar-frosted, var(--bg-sidebar));
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    box-shadow: var(--shadow-lg);
    backdrop-filter: blur(20px);
    overflow: hidden;
    z-index: 84;
    padding: 8px 0;
    transition:
      top var(--animation-normal),
      opacity var(--animation-normal),
      transform var(--animation-normal),
      visibility var(--animation-normal);
  }

  :global(body.no-sidebar:not(.sidebar-peek):not(.reading)) .sidebar {
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transform: translateX(-6px);
  }

  :global(body.no-sidebar.sidebar-peek:not(.reading)) .sidebar {
    opacity: 1;
    visibility: visible;
    pointer-events: auto;
    transform: translateX(0);
  }

  :global(body.opaque-surfaces) .sidebar {
    background: var(--custom-bar-opaque, var(--opaque-sidebar));
    backdrop-filter: none;
  }

  .sidebar-scroll {
    /* `min-height: 0` es el patrón de los demás contenedores con scroll del
       proyecto (`.inspector-content`, `.modal-body`): deja explícito que este
       flex item puede encogerse. Con `overflow: auto` el mínimo automático ya
       es 0, así que hoy no cambia nada, pero evita depender de ese detalle. */
    flex: 1;
    min-height: 0;
    padding: 8px 10px 12px;
    z-index: 30;
    user-select: none;
  }

  .import-progress {
    padding: 8px 10px;
    color: var(--text-secondary);
    font-size: 12px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 12px 8px 6px;
  }

  .section-title {
    font-size: 14px;
    font-weight: 750;
    letter-spacing: -0.01em;
    margin: 0;
  }

  .files-header { margin-top: 4px; }

  .section-header-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .recent-header-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--animation-fast);
  }

  .section-header:hover .recent-header-actions,
  .section-header:focus-within .recent-header-actions {
    opacity: 1;
    pointer-events: auto;
  }

  .mini-action,
  .section-chevron {
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    transition: background var(--animation-fast), color var(--animation-fast);
  }

  .mini-action:hover,
  .section-chevron:hover {
    background: var(--bg-active);
    color: var(--text-primary);
  }

  .mini-action:disabled { opacity: 0.35; pointer-events: none; }

  .section-chevron :global(svg) {
    transition: transform var(--animation-fast);
  }

  .section-chevron.collapsed :global(svg) {
    transform: rotate(-90deg);
  }

  .add-menu {
    margin: 4px 8px 8px;
    display: grid;
    gap: 2px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    background: var(--bg-button);
  }

  .add-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 9px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    text-align: left;
  }

  .add-menu-item:hover {
    background: var(--bg-active);
  }
</style>
