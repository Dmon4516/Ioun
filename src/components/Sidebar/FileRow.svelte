<script lang="ts">
  import { documents } from "$lib/stores/documents.svelte";
  import {
    tabs,
    setActiveTab,
    getTabByDocId,
    openDocument,
    closeTab,
    hibernateTab,
  } from "$lib/stores/tabs.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { displayTitle, pathSummary } from "$lib/utils/format";
  import { computeMarquee } from "$lib/utils/marquee";
  import { observeMarquee } from "$lib/utils/marqueeObserver";
  import { dnd, startDrag, endDrag, setDropTarget, clearDropTarget, performDrop } from "$lib/dnd.svelte";
  import { removeDoc, removeFromLibrary as removeFromLibraryCommand } from "$lib/tauri/commands";
  import { isDocInLibrary, reloadLibrary } from "$lib/stores/library.svelte";
  import { ui, showContextMenu } from "$lib/stores/ui.svelte";
  import { showToast } from "$lib/stores/toast.svelte";
  import { confirmDialog } from "$lib/tauri/dialogs";
  import CategoryPicker from "./CategoryPicker.svelte";
  import Icon from "$components/Icon.svelte";

  let { docId }: { docId: string } = $props();
  let pickerPos = $state<{ x: number; y: number } | null>(null);
  let titleEl: HTMLElement | null = $state(null);
  let titleTextEl: HTMLElement | null = $state(null);
  let marqueeActive = $state(false);
  let marqueeDistance = $state(0);
  let marqueeDuration = $state(12);

  const doc = $derived(documents.items.get(docId) ?? null);
  const tab = $derived(getTabByDocId(docId));
  const isActive = $derived(tab?.id === tabs.activeId);
  const category = $derived.by(() =>
    doc?.categoryIds
      .map((id) => documents.categories.find((candidate) => candidate.id === id))
      .find((candidate) => candidate !== undefined) ?? null,
  );

  $effect(() => {
    void doc?.title;
    void settings.marqueeTitle;
    marqueeActive = false;
    marqueeDistance = 0;
    if (!settings.marqueeTitle || !titleEl || !titleTextEl) return;
    requestAnimationFrame(() => {
      if (!titleEl || !titleTextEl) return;
      const marquee = computeMarquee(titleTextEl, titleEl);
      if (!marquee) return;
      marqueeDistance = marquee.distance;
      marqueeDuration = marquee.duration;
      marqueeActive = true;
    });
  });

  $effect(() => {
    if (!titleEl || !titleTextEl) return;
    return observeMarquee(titleEl, () => {
      marqueeActive = false;
      requestAnimationFrame(() => {
        if (!titleEl || !titleTextEl || !settings.marqueeTitle) return;
        const marquee = computeMarquee(titleTextEl, titleEl);
        if (!marquee) return;
        marqueeDistance = marquee.distance;
        marqueeDuration = marquee.duration;
        marqueeActive = true;
      });
    });
  });

  const isDragging = $derived(dnd.active && dnd.kind === "doc" && dnd.id === docId);
  const isDropBefore = $derived(
    dnd.active && dnd.targetId === docId && dnd.targetKind === "doc" && dnd.targetPosition === "before",
  );
  const isDropAfter = $derived(
    dnd.active && dnd.targetId === docId && dnd.targetKind === "doc" && dnd.targetPosition === "after",
  );

  function handleClick() {
    if (!doc) return;
    if (tab) setActiveTab(tab.id);
    else openDocument(doc.id);
  }

  function onDragStart(e: DragEvent) {
    if (settings.librarySort !== "manual") {
      e.preventDefault();
      return;
    }
    if (!e.dataTransfer) return;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", docId);
    startDrag("doc", docId);
  }
  function onDragEnd() {
    endDrag();
  }
  function onDragOver(e: DragEvent) {
    if (!dnd.active || (dnd.kind === "doc" && dnd.id === docId)) return;
    e.preventDefault();
    e.stopPropagation();
    if (!e.dataTransfer) return;
    e.dataTransfer.dropEffect = "move";
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos = e.clientY - rect.top < rect.height / 2 ? "before" : "after";
    setDropTarget(docId, "doc", pos);
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
    else showToast("No se pudo mover el documento.");
  }

  async function removeFromLibrary() {
    if (!doc) return;
    try {
      await removeFromLibraryCommand(doc.id);
      documents.items.set(doc.id, {
        ...doc,
        inLibrary: false,
      });
      await reloadLibrary();
    } catch (err) {
      console.error("[sidebar] No se pudo quitar el documento de archivos", err);
      showToast("No se pudo quitar el documento de archivos.");
    }
  }

  async function deleteDocumentData() {
    if (!doc) return;
    try {
      await removeDoc(doc.id);
      documents.items.delete(doc.id);
      await reloadLibrary();
      if (tab) closeTab(tab.id);
    } catch (err) {
      console.error("[sidebar] No se pudieron eliminar los datos del documento", err);
      showToast("No se pudieron eliminar los datos del documento.");
    }
  }

  function openContextMenu(e: MouseEvent) {
    if (!doc) return;
    e.preventDefault();
    e.stopPropagation();
    const items = [] as Parameters<typeof showContextMenu>[2];

    if (tab) {
      if (tab.status === "hibernated") {
        items.push({
          label: "Restaurar pestaña",
          action: () => setActiveTab(tab.id),
        });
      } else if (tab.id !== tabs.activeId) {
        items.push({
          label: "Hibernar pestaña",
          action: () => hibernateTab(tab.id),
        });
      }
    }

    items.push({
      label: "Mostrar información",
      action: () => {
        if (tab) setActiveTab(tab.id);
        else openDocument(doc.id);
        ui.inspector = "info";
      },
    });

    if (settings.categoriesEnabled && isDocInLibrary(doc.id)) {
      items.push({
        label: "Asignar categoría…",
        action: () => {
          window.setTimeout(() => {
            pickerPos = { x: e.clientX, y: e.clientY };
          }, 10);
        },
      });
    }

    if (isDocInLibrary(doc.id)) {
      items.push({
        label: "Quitar de archivos",
        action: () => void removeFromLibrary(),
      });
    }

    items.push({
      label: "Eliminar datos de la app en este documento",
      danger: true,
      action: () => {
        void (async () => {
          if (!(await confirmDialog(`¿Eliminar los datos de "${doc.title}"?`))) return;
          void deleteDocumentData();
        })();
      },
    });

    if (tab) {
      items.push({
        label: "Cerrar pestaña",
        action: () => closeTab(tab.id),
      });
    }

    showContextMenu(e.clientX, e.clientY, items);
  }
</script>

{#if doc}
  <div
    class="file-block"
    class:drop-before={isDropBefore}
    class:drop-after={isDropAfter}
    class:dragging={isDragging}
  >
    <div
      class="file-row"
      class:active={isActive}
      draggable={settings.librarySort === "manual"}
      ondragstart={onDragStart}
      ondragend={onDragEnd}
      ondragover={onDragOver}
      ondragenter={onDragOver}
      ondragleave={onDragLeave}
      ondrop={onDrop}
      onclick={handleClick}
      oncontextmenu={openContextMenu}
      onkeydown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          handleClick();
        }
      }}
      role="button"
      tabindex="0"
      title={doc.path}
    >
      {#if settings.showIcons}
        <span class="file-icon"><Icon name="file" size={16} /></span>
      {/if}

      {#if tab}
        <span
          class="status-dot"
          class:active={tab.status === "active"}
          class:idle={tab.status === "idle"}
          class:hibernated={tab.status === "hibernated"}
          class:restoring={tab.status === "restoring"}
        ></span>
      {/if}

      {#if category}
        <span
          class="category-dot"
          style:background={category.color}
          title={`Categoría: ${category.name}`}
        ></span>
      {/if}

      <div class="side-text">
        <div class="side-title">
          <span
            class="title"
            class:marquee={marqueeActive}
            bind:this={titleEl}
            style:--marquee-distance={`${marqueeDistance}px`}
            style:--marquee-duration={`${marqueeDuration}s`}
          ><span class="title-text" bind:this={titleTextEl}>{displayTitle(doc.title, settings.showExtensions)}</span></span>
        </div>
        {#if settings.pathDisplay !== "hide"}
          <div class="side-meta">{pathSummary(doc.path, settings.pathDepth)}</div>
        {/if}
      </div>

      <div class="side-actions">
        {#if tab}
          <button
            class="mini-action"
            type="button"
            title="Cerrar pestaña"
            onclick={(e) => {
              e.stopPropagation();
              closeTab(tab.id);
            }}
          >
            <Icon name="close" size={12} />
          </button>
        {:else}
          <button
            class="mini-action"
            type="button"
            title="Quitar de archivos"
            onclick={(e) => {
              e.stopPropagation();
              void removeFromLibrary();
            }}
          >
            <Icon name="trash" size={12} />
          </button>
        {/if}
      </div>
    </div>
  </div>
  {#if pickerPos}
    <CategoryPicker
      docId={docId}
      x={pickerPos.x}
      y={pickerPos.y}
      onClose={() => (pickerPos = null)}
    />
  {/if}
{:else}
  <div class="file-row skeleton" aria-hidden="true">
    <span class="status-dot"></span>
    <div class="side-text">
      <div class="side-title">
        <span class="skeleton-bar"></span>
      </div>
    </div>
  </div>
{/if}

<style>
  .file-block {
    position: relative;
    min-width: 0;
  }

  .file-block.drop-before::before,
  .file-block.drop-after::after {
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

  .file-block.drop-before::before { top: -1px; }
  .file-block.drop-after::after { bottom: -1px; }

  .file-row {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    gap: 8px;
    padding: 7px 10px;
    border-radius: var(--radius-card);
    cursor: pointer;
    transition: background var(--animation-fast);
    margin-bottom: 2px;
    outline: none;
  }

  .file-row:hover { background: var(--bg-active); }

  .file-row.active {
    background: var(--bg-active);
  }

  .file-row.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    border-radius: var(--radius-card) 0 0 var(--radius-card);
    background: var(--state-active);
    pointer-events: none;
  }

  .file-row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .file-row.skeleton {
    pointer-events: none;
  }

  .file-row.skeleton .status-dot {
    background: var(--bg-button);
  }

  .skeleton-bar {
    display: block;
    height: 10px;
    width: 60%;
    border-radius: 4px;
    background: linear-gradient(
      90deg,
      var(--bg-button),
      var(--bg-active),
      var(--bg-button)
    );
    background-size: 220% 100%;
    animation: skeletonPulse 1.2s infinite linear;
  }

  @keyframes skeletonPulse {
    0% { background-position: 100% 0; }
    100% { background-position: -100% 0; }
  }

  .file-block.dragging {
    opacity: 0.45;
  }

  .file-icon {
    display: grid;
    place-items: center;
    color: var(--icon-color);
    flex: 0 0 auto;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: 0 0 auto;
  }

  .status-dot.active { background: var(--state-active); }
  .status-dot.idle { background: var(--state-idle); }
  .status-dot.hibernated { background: var(--state-hibernated); }
  .status-dot.restoring { background: var(--state-restoring); }

  .category-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: 0 0 auto;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.18);
  }

  .side-text {
    flex: 1;
    min-width: 0;
    transition: padding-right var(--animation-fast);
  }

  .side-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 400;
    line-height: 1.3;
  }

  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title-text {
    display: inline-block;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: top;
    font-weight: 400;
  }

  /* El nombre se trunca; al pasar el ratón (o enfocar) se desplaza, como el título de la barra. */
  .file-row:hover .title.marquee,
  .file-row:focus-visible .title.marquee {
    text-overflow: clip;
  }

  .file-row:hover .title.marquee .title-text,
  .file-row:focus-visible .title.marquee .title-text {
    max-width: none;
    text-overflow: clip;
    will-change: transform;
    animation: titleMarquee var(--marquee-duration, 24s) linear infinite;
  }

  @keyframes titleMarquee {
    0% { transform: translateX(0); }
    45%, 55% { transform: translateX(calc(-1 * var(--marquee-distance, 0px))); }
    100% { transform: translateX(0); }
  }

  @media (prefers-reduced-motion: reduce) {
    .file-row:hover .title.marquee,
    .file-row:focus-visible .title.marquee {
      text-overflow: ellipsis;
    }

    .file-row:hover .title.marquee .title-text,
    .file-row:focus-visible .title.marquee .title-text {
      max-width: 100%;
      text-overflow: ellipsis;
      animation: none;
    }
  }

  .side-meta {
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(body[data-paths-mode="expand"]) .side-meta {
    max-height: 16px;
    opacity: 0.72;
    font-size: 10px;
    transition:
      max-height var(--animation-normal),
      opacity var(--animation-normal),
      font-size var(--animation-normal);
  }

  :global(body[data-paths-mode="expand"]) .file-row:hover .side-meta {
    max-height: 72px;
    white-space: normal;
    opacity: 1;
    font-size: 11px;
    overflow-wrap: anywhere;
  }

  .side-actions {
    position: absolute;
    right: 10px;
    top: 50%;
    display: flex;
    gap: 4px;
    opacity: 0;
    pointer-events: none;
    transform: translateY(-50%);
    transition: opacity var(--animation-fast);
  }

  .file-row:hover .side-actions,
  .file-row:focus-within .side-actions {
    opacity: 1;
    pointer-events: auto;
  }

  .file-row:hover .side-text,
  .file-row:focus-within .side-text {
    padding-right: 48px;
  }

  .mini-action {
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
  }

  .mini-action:hover {
    background: var(--bg-active);
    color: var(--text-primary);
  }
</style>
