<script lang="ts">
  import { onDestroy } from "svelte";
  import type { FolderRecord } from "$lib/types";
  import {
    childrenOf,
    folderChildCount,
    hasLoadedChildren,
    loadFolderChildren,
    reloadLibrary,
    findFolder,
  } from "$lib/stores/library.svelte";
  import { setFolderExpanded, renameFolder, deleteFolder } from "$lib/tauri/commands";
  import { dnd, startDrag, endDrag, setDropTarget, clearDropTarget, performDrop } from "$lib/dnd.svelte";
  import { focusOnMount } from "$lib/actions";
  import { showContextMenu } from "$lib/stores/ui.svelte";
  import { showToast } from "$lib/stores/toast.svelte";
  import { confirmDialog } from "$lib/tauri/dialogs";
  import { settings } from "$lib/stores/settings.svelte";
  import { removeDocument } from "$lib/stores/documents.svelte";
  import { forgetDocuments } from "$lib/stores/tabs.svelte";
  import FolderNode from "./FolderNode.svelte";
  import FileRow from "./FileRow.svelte";
  import Icon from "$components/Icon.svelte";

  let {
    folder,
    depth = 0,
  }: { folder: FolderRecord; depth?: number } = $props();

  let editing = $state(false);
  let draftName = $state("");
  let inputEl: HTMLInputElement | null = $state(null);
  let clickTimer = 0;

  const children = $derived(childrenOf(folder.id));
  const isExpanded = $derived(folder.expanded);

  const isDragging = $derived(dnd.active && dnd.kind === "folder" && dnd.id === folder.id);
  const isDropInside = $derived(
    dnd.active &&
    dnd.targetId === folder.id &&
    dnd.targetKind === "folder" &&
    dnd.targetPosition === "inside",
  );
  const isDropBefore = $derived(
    dnd.active &&
    dnd.targetId === folder.id &&
    dnd.targetKind === "folder" &&
    dnd.targetPosition === "before",
  );
  const isDropAfter = $derived(
    dnd.active &&
    dnd.targetId === folder.id &&
    dnd.targetKind === "folder" &&
    dnd.targetPosition === "after",
  );

  $effect(() => {
    // Una carpeta que arranca expandida (las raíces sincronizadas, por ejemplo)
    // necesita pedir sus hijos: con el árbol bajo demanda no vienen solos.
    if (isExpanded && !hasLoadedChildren(folder.id)) void loadFolderChildren(folder.id);
  });

  async function toggle(): Promise<void> {
    // Al expandir se piden frescos: la carpeta pudo cambiar desde la última vez.
    if (!folder.expanded) void loadFolderChildren(folder.id);
    const previous = folder.expanded;
    folder.expanded = !folder.expanded;
    try {
      await setFolderExpanded(folder.id, folder.expanded);
    } catch (error) {
      folder.expanded = previous;
      console.error("[sidebar] No se pudo cambiar el estado de la carpeta", error);
      showToast("No se pudo actualizar la carpeta.");
    }
  }

  function onRowClick(): void {
    if (editing) return;
    clearTimeout(clickTimer);
    clickTimer = window.setTimeout(() => void toggle(), 220);
  }

  function onRowDblClick(): void {
    clearTimeout(clickTimer);
    beginEdit();
  }

  onDestroy(() => clearTimeout(clickTimer));

  function beginEdit() {
    draftName = folder.name;
    editing = true;
  }

  async function commitEdit(): Promise<void> {
    const next = draftName.trim() || "Sin título";
    editing = false;
    if (next !== folder.name) {
      const previous = folder.name;
      folder.name = next;
      try {
        await renameFolder(folder.id, next);
      } catch (error) {
        folder.name = previous;
        console.error("[sidebar] No se pudo renombrar la carpeta", error);
        showToast("No se pudo renombrar la carpeta.");
      }
    }
  }

  function cancelEdit() {
    editing = false;
    draftName = folder.name;
  }

  async function handleDelete() {
    const message = settings.deleteFolderContents
      ? `¿Eliminar "${folder.name}" y quitar sus documentos de Reign? Los PDF se conservarán en el disco.`
      : `¿Eliminar la carpeta "${folder.name}"? Sus subcarpetas desaparecerán de Reign y sus documentos pasarán a la raíz de Archivos.`;
    if (!(await confirmDialog(message))) return;
    try {
      const removedDocumentIds = await deleteFolder(folder.id, settings.deleteFolderContents);
      forgetDocuments(removedDocumentIds);
      removedDocumentIds.forEach(removeDocument);
      await reloadLibrary();
    } catch (err) {
      console.error("[sidebar] No se pudo eliminar la carpeta", err);
      showToast("No se pudo eliminar la carpeta.");
    }
  }

  function onDragStart(e: DragEvent) {
    if (settings.librarySort !== "manual") {
      e.preventDefault();
      return;
    }
    if (editing) {
      e.preventDefault();
      return;
    }
    if (!e.dataTransfer) return;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", folder.id);
    startDrag("folder", folder.id);
  }

  function onDragEnd() {
    endDrag();
  }

  function onDragOver(e: DragEvent) {
    if (!dnd.active) return;
    if (dnd.kind === "folder" && dnd.id === folder.id) return;
    e.preventDefault();
    e.stopPropagation();
    if (!e.dataTransfer) return;
    e.dataTransfer.dropEffect = "move";

    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const y = e.clientY - rect.top;
    const h = rect.height || 1;
    let pos: "before" | "after" | "inside";
    if (y < h * 0.28) pos = "before";
    else if (y > h * 0.72) pos = "after";
    else pos = "inside";

    setDropTarget(folder.id, "folder", pos);
  }

  function onDragLeave(e: DragEvent) {
    if ((e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) return;
    clearDropTarget();
  }

  async function onDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    const ok = await performDrop();
    if (ok) await reloadLibrary();
    else showToast("No se pudo mover el elemento.");
  }

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    showContextMenu(e.clientX, e.clientY, [
      { label: folder.expanded ? "Contraer" : "Expandir", action: toggle },
      { label: "Renombrar", action: beginEdit },
      { label: "Eliminar carpeta", danger: true, action: () => void handleDelete() },
    ]);
  }
</script>

<div
  class="folder-block"
  style:--depth={depth}
  class:drop-inside={isDropInside}
  class:drop-before={isDropBefore}
  class:drop-after={isDropAfter}
>
  <article
    class="folder-row"
    class:expanded={isExpanded}
    class:dragging={isDragging}
    draggable={!editing && settings.librarySort === "manual"}
    ondragstart={onDragStart}
    ondragend={onDragEnd}
    ondragover={onDragOver}
    ondragenter={onDragOver}
    ondragleave={onDragLeave}
    ondrop={onDrop}
    oncontextmenu={onContextMenu}
    ondblclick={onRowDblClick}
  >
    <div class="side-text">
      {#if editing}
        <input
          class="folder-name-input"
          type="text"
          bind:value={draftName}
          bind:this={inputEl}
          onblur={commitEdit}
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              commitEdit();
            } else if (e.key === "Escape") {
              e.preventDefault();
              cancelEdit();
            }
          }}
          use:focusOnMount
        />
      {:else}
        <button
          class="row-main"
          type="button"
          onclick={onRowClick}
          aria-expanded={isExpanded}
        >
          <span class="chevron" class:open={isExpanded}>
            <Icon name="chevron-right" size={12} />
          </span>
          {#if settings.showIcons}
            <span class="folder-icon"><Icon name={isExpanded ? "folder-open" : "folder"} size={16} /></span>
          {/if}
          <span class="folder-name">{folder.name || "Sin título"}</span>
          <span class="count">{folderChildCount(folder.id)}</span>
        </button>
      {/if}
    </div>
  </article>

  {#if isExpanded && !editing}
    <div class="children" style:--depth={depth + 1}>
      {#each children.folders as childId (childId)}
        {@const child = findFolder(childId)}
        {#if child}
          <FolderNode folder={child} depth={depth + 1} />
        {/if}
      {/each}
      {#each children.docs as docId (docId)}
        <FileRow {docId} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .folder-block {
    position: relative;
    min-width: 0;
    border-radius: var(--radius-card);
  }

  .folder-block.drop-before::before,
  .folder-block.drop-after::after {
    content: "";
    position: absolute;
    left: 8px;
    right: 8px;
    height: 2px;
    background: var(--accent);
    border-radius: 2px;
    box-shadow: 0 0 6px color-mix(in srgb, var(--accent) 55%, transparent);
    pointer-events: none;
    z-index: 3;
  }

  .folder-block.drop-before::before { top: -1px; }
  .folder-block.drop-after::after { bottom: -1px; }

  .folder-block.drop-inside {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .folder-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-radius: var(--radius-card);
    cursor: pointer;
    transition: background var(--animation-fast);
    margin-bottom: 2px;
  }

  .folder-row:hover { background: var(--bg-active); }
  .folder-row.dragging { opacity: 0.45; }

  .side-text {
    flex: 1;
    min-width: 0;
  }

  .row-main {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 400;
    line-height: 1.3;
    text-align: left;
    cursor: pointer;
  }

  .chevron {
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    color: var(--text-secondary);
    transition: transform var(--animation-fast);
    flex: 0 0 auto;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .folder-icon {
    display: grid;
    place-items: center;
    color: var(--icon-color);
    flex: 0 0 auto;
  }

  .folder-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: 10px;
    color: var(--text-secondary);
    opacity: 0.7;
    flex: 0 0 auto;
    margin-left: 4px;
  }

  .folder-name-input {
    width: 100%;
    height: 28px;
    border-radius: 9px;
    border: 1px solid color-mix(in srgb, var(--accent) 42%, var(--border));
    background: var(--bg-input);
    padding: 0 8px;
    outline: none;
    color: var(--text-primary);
    font-size: 13px;
  }

  .children {
    display: grid;
    gap: 0;
    margin-left: 12px;
    padding-left: 10px;
    border-left: 1px solid var(--border);
    margin-bottom: 4px;
  }
</style>
