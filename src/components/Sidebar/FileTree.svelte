<script lang="ts">
  import {
    childrenOf,
    folderContaining,
    isDocInLibrary,
    reloadLibrary,
    findFolder,
  } from "$lib/stores/library.svelte";
  import { dnd, clearDropTarget, performDrop } from "$lib/dnd.svelte";
  import { showToast } from "$lib/stores/toast.svelte";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import FolderNode from "./FolderNode.svelte";
  import FileRow from "./FileRow.svelte";

  const root = $derived(childrenOf(null));
  const activeDocInCollapsedFolder = $derived.by(() => {
    const docId = getActiveTab()?.docId;
    if (!docId || !isDocInLibrary(docId)) return null;

    let folderId = folderContaining(docId);
    const visited = new Set<string>();
    while (folderId && !visited.has(folderId)) {
      visited.add(folderId);
      const folder = findFolder(folderId);
      // Con el árbol bajo demanda, un ancestro que no está en memoria significa
      // que cuelga de una carpeta contraída: el documento está oculto.
      if (!folder) return docId;
      if (!folder.expanded) return docId;
      folderId = folder.parentId;
    }
    return null;
  });

  function onDragOver(e: DragEvent) {
    if (!dnd.active) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    if (e.target === e.currentTarget || (e.target instanceof Element && e.target.classList.contains("empty"))) {
      clearDropTarget();
    }
  }

  async function onDrop(e: DragEvent) {
    if (!dnd.active) return;
    e.preventDefault();
    try {
      if (!(await performDrop())) {
        showToast("No se pudo mover el elemento.");
        return;
      }
      await reloadLibrary();
    } catch (error) {
      console.error("[dnd] Could not refresh the library after dropping", error);
      showToast(`No se pudo actualizar la biblioteca: ${String(error)}`);
    }
  }
</script>

<div class="tree" role="region" aria-label="Archivos y carpetas" ondragover={onDragOver} ondrop={onDrop}>
  {#if root.folders.length === 0 && root.docs.length === 0}
    <div class="empty">No hay archivos ni carpetas.</div>
  {:else}
    {#each root.folders as folderId (folderId)}
      {@const folder = findFolder(folderId)}
      {#if folder}
        <FolderNode {folder} depth={0} />
      {/if}
    {/each}

    {#each root.docs as docId (docId)}
      <FileRow {docId} />
    {/each}

    {#if activeDocInCollapsedFolder}
      <div class="active-document" aria-label="Documento activo en una carpeta contraída">
        <FileRow docId={activeDocInCollapsedFolder} />
      </div>
    {/if}
  {/if}
</div>

<style>
  .tree {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 0;
    padding-bottom: 8px;
    min-height: 60px;
    min-width: 0;
  }

  .empty {
    padding: 7px 8px 10px;
    color: var(--text-secondary);
    font-size: 12px;
  }

  .active-document {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    margin-top: 8px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }
</style>
