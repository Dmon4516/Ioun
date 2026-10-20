<script lang="ts">
  import { documents } from "$lib/stores/documents.svelte";
  import { library } from "$lib/stores/library.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { displayTitle } from "$lib/utils/format";
  import { getTabByDocId, setActiveTab, openDocument } from "$lib/stores/tabs.svelte";
  import Icon from "$components/Icon.svelte";

  const docsByCategory = $derived.by(() => {
    const inLib = new Set<string>();
    for (const id of library.rootDocIds) inLib.add(id);
    for (const docs of Object.values(library.folderDocs)) {
      for (const id of docs) inLib.add(id);
    }

    const groups = new Map<string, { name: string; color: string; docs: typeof documents.items extends Map<string, infer V> ? V[] : never[] }>();

    for (const cat of documents.categories) {
      groups.set(cat.id, { name: cat.name, color: cat.color, docs: [] });
    }
    groups.set("__none", { name: "Sin organizar", color: "#8e8e93", docs: [] });

    for (const doc of documents.items.values()) {
      if (!inLib.has(doc.id)) continue;
      const categoryIds = doc.categoryIds.filter((id) => groups.has(id));
      if (categoryIds.length === 0) {
        groups.get("__none")!.docs.push(doc);
      } else {
        for (const categoryId of categoryIds) groups.get(categoryId)!.docs.push(doc);
      }
    }

    return [...groups.entries()]
      .filter(([, g]) => g.docs.length > 0)
      .map(([id, g]) => ({ id, ...g }));
  });

  function openDoc(docId: string) {
    const tab = getTabByDocId(docId);
    if (tab) setActiveTab(tab.id);
    else openDocument(docId);
  }
</script>

<div class="category-view">
  {#if docsByCategory.length === 0}
    <div class="empty">No hay archivos en esta vista.</div>
  {:else}
    {#each docsByCategory as group (group.id)}
      <div class="group">
        <div class="group-header">
          <span class="dot" style:background={group.color}></span>
          <span>{group.name}</span>
          <span class="count">{group.docs.length}</span>
        </div>
        {#each group.docs as doc (doc.id)}
          <button
            class="cat-row"
            type="button"
            onclick={() => openDoc(doc.id)}
            title={doc.path}
          >
            <Icon name="file" size={14} />
            <span class="cat-title">{displayTitle(doc.title, settings.showExtensions)}</span>
          </button>
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  .category-view {
    display: grid;
    gap: 12px;
    padding: 0 4px 8px;
  }

  .empty {
    padding: 7px 8px 10px;
    color: var(--text-secondary);
    font-size: 12px;
  }

  .group {
    display: grid;
    gap: 2px;
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px 4px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: 0 0 auto;
  }

  .count {
    margin-left: auto;
    font-size: 10px;
    opacity: 0.7;
    letter-spacing: 0;
    text-transform: none;
  }

  .cat-row {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border: none;
    border-radius: var(--radius-card);
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }

  .cat-row:hover { background: var(--bg-active); }

  .cat-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
