<script lang="ts">
  import type { DocumentRecord } from "$lib/types";
  import PageSlot from "./PageSlot.svelte";
  import { viewer } from "$lib/stores/viewer.svelte";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import {
    PAGE_WINDOW_MARGIN_ROWS,
    pageWindow,
    pageWindowSpacers,
  } from "$lib/utils/pageWindow";

  let { doc }: { doc: DocumentRecord } = $props();

  const activeTab = $derived(getActiveTab());

  /**
   * Solo se montan las páginas de la ventana visible. Montar una por página
   * (900 en un libro) son 900 componentes con sus efectos, dos canvas y su
   * suscripción al registro de visibilidad cada uno.
   */
  const window = $derived.by(() => {
    const prefix = `${doc.id}:`;
    const visible = [...viewer.visiblePages]
      .filter((key) => key.startsWith(prefix))
      .map((key) => Number(key.slice(prefix.length)))
      .filter((page) => Number.isInteger(page) && page > 0);
    const fallback = activeTab?.page ?? 1;
    const first = visible.length ? Math.min(...visible) : fallback;
    const last = visible.length ? Math.max(...visible) : fallback;
    return pageWindow(
      first,
      last,
      viewer.dims.pagesPerRow,
      doc.pages,
      PAGE_WINDOW_MARGIN_ROWS,
    );
  });

  const mountedPages = $derived.by(() => {
    const result: number[] = [];
    for (let page = window.firstPage; page <= window.lastPage; page++) result.push(page);
    return result;
  });

  const spacers = $derived(
    pageWindowSpacers(
      window.firstRow,
      window.lastRow,
      doc.pages,
      viewer.dims.pagesPerRow,
      viewer.dims.pageHeight,
      viewer.dims.gap,
    ),
  );
</script>

<div
  class="continuous-grid"
  style:--gap={`${viewer.dims.gap}px`}
  style:grid-template-columns={`repeat(${viewer.dims.pagesPerRow}, ${viewer.dims.pageWidth}px)`}
>
  {#if spacers.top > 0}
    <div class="row-spacer" style:height={`${spacers.top}px`} aria-hidden="true"></div>
  {/if}
  {#each mountedPages as p (`${doc.id}:${p}`)}
    <PageSlot page={p} docId={doc.id} current={p === activeTab?.page} />
  {/each}
  {#if spacers.bottom > 0}
    <div class="row-spacer" style:height={`${spacers.bottom}px`} aria-hidden="true"></div>
  {/if}
</div>

<style>
  .row-spacer {
    grid-column: 1 / -1;
    pointer-events: none;
  }

  .continuous-grid {
    display: grid;
    gap: var(--gap);
    justify-content: center;
    align-items: start;
    width: max-content;
    min-width: 100%;
    margin: 0 auto;
  }
</style>
