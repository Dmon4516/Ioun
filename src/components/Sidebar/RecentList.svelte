<script lang="ts">
  import { documents } from "$lib/stores/documents.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { displayTitle, pathSummary } from "$lib/utils/format";
  import { getTabByDocId } from "$lib/stores/tabs.svelte";
  import { removeRecentDoc } from "$lib/tauri/commands";
  import { showToast } from "$lib/stores/toast.svelte";
  import Icon from "$components/Icon.svelte";

  let { onOpen }: { onOpen: (docId: string) => void } = $props();

  const recentDocs = $derived(
    [...documents.items.values()]
    .filter((d) =>
      d.inRecent &&
      d.lastOpened != null &&
      !d.inLibrary &&
      !getTabByDocId(d.id),
    )
      .sort((a, b) => (b.lastOpened ?? 0) - (a.lastOpened ?? 0))
      .slice(0, settings.maxRecentDocs),
  );

  async function removeRecent(docId: string): Promise<void> {
    try {
      await removeRecentDoc(docId);
      const doc = documents.items.get(docId);
      if (doc) {
        documents.items.set(docId, {
          ...doc,
          inRecent: false,
        });
      }
    } catch (error) {
      console.error("[sidebar] No se pudo quitar el documento de recientes", error);
      showToast("No se pudo quitar el documento de recientes.");
    }
  }
</script>

<div class="recent-list">
  {#if recentDocs.length === 0}
    <div class="empty">No hay elementos recientes.</div>
  {:else}
    {#each recentDocs as doc (doc.id)}
      <div
        class="recent-row"
        role="button"
        tabindex="0"
        onclick={() => onOpen(doc.id)}
        onkeydown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onOpen(doc.id);
          }
        }}
      >
        {#if settings.showIcons}
          <Icon name="file" size={15} />
        {/if}
        <span class="recent-text">
          <span class="recent-title">{displayTitle(doc.title, settings.showExtensions)}</span>
          <span class="recent-meta">{pathSummary(doc.path, settings.pathDepth)}</span>
        </span>
        <button
          class="recent-remove"
          type="button"
          title="Quitar de recientes"
          aria-label={`Quitar ${doc.title} de recientes`}
          onclick={(event) => {
            event.stopPropagation();
            void removeRecent(doc.id);
          }}
          onkeydown={(event) => event.stopPropagation()}
        ><Icon name="trash" size={13} /></button>
      </div>
    {/each}
  {/if}
</div>

<style>
  .recent-list {
    display: grid;
    gap: 2px;
    padding: 0 0 8px;
  }

  .empty {
    padding: 7px 10px 10px;
    color: var(--text-secondary);
    font-size: 12px;
  }

  .recent-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border: none;
    border-radius: var(--radius-card);
    background: transparent;
    color: var(--text-primary);
    text-align: left;
    cursor: pointer;
    transition: background var(--animation-fast);
  }

  .recent-row:hover {
    background: var(--bg-active);
  }

  .recent-text { display: grid; gap: 2px; flex: 1; min-width: 0; }
  .recent-title {
    font-size: 13px;
    font-weight: 400;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .recent-meta {
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.72;
  }

  .recent-remove {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-secondary);
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--animation-fast), background var(--animation-fast);
  }
  .recent-row:hover .recent-remove,
  .recent-row:focus-within .recent-remove {
    opacity: 1;
    pointer-events: auto;
  }
  .recent-remove:hover { background: var(--bg-active); color: var(--text-primary); }
</style>
