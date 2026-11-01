<script lang="ts">
  import type { DocumentRecord } from "$lib/types";
  import PageSlot from "./PageSlot.svelte";
  import { viewer } from "$lib/stores/viewer.svelte";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { effectivePagesPerRow, spreadStartPage } from "$lib/utils/documentView";

  let { doc }: { doc: DocumentRecord } = $props();

  const activeTab = $derived(getActiveTab());
  const pagesPerRow = $derived(effectivePagesPerRow(doc));
  const activePage = $derived(activeTab?.page ?? 1);

  const start = $derived(spreadStartPage(activePage, doc.pages, pagesPerRow));

  const visiblePages = $derived.by(() => {
    const out: number[] = [];
    const end = Math.min(doc.pages, start + pagesPerRow - 1);
    for (let p = start; p <= end; p++) out.push(p);
    return out;
  });

</script>

<div
  class="spread-grid"
  style:--gap={`${viewer.dims.gap}px`}
  style:grid-template-columns={`repeat(${pagesPerRow}, ${viewer.dims.pageWidth}px)`}
>
  {#if visiblePages.length === 0}
    <div class="empty-spread" role="status" style:grid-column={`1 / -1`}>No hay páginas disponibles</div>
  {:else}
    {#each visiblePages as p (`${doc.id}:${p}`)}
      <PageSlot page={p} docId={doc.id} current={p === activePage} />
    {/each}
  {/if}
</div>

<style>
  .spread-grid {
    display: grid;
    gap: var(--gap);
    justify-content: center;
    align-items: start;
    width: max-content;
    min-width: 100%;
    margin: 0 auto;
    padding: 20px 0;
  }
  .empty-spread {
    width: min(320px, 80vw);
    padding: 28px 18px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-card);
    color: var(--text-secondary);
    text-align: center;
  }
</style>
