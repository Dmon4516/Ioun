<script lang="ts">
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { documents } from "$lib/stores/documents.svelte";
  import { settings } from "$lib/stores/settings.svelte";

  let visible = $state(false);
  let indicatorTimer = 0;
  let lastDocId = "";
  let lastPage = 0;

  const activeTab = $derived(getActiveTab());
  const doc = $derived(activeTab ? documents.items.get(activeTab.docId) ?? null : null);

  $effect(() => {
    const docId = activeTab?.docId ?? "";
    const page = activeTab?.page ?? 0;
    const mode = settings.pageIndicatorMode;
    const pageChanged = docId === lastDocId && page !== lastPage;
    lastDocId = docId;
    lastPage = page;

    clearTimeout(indicatorTimer);
    indicatorTimer = 0;
    if (!docId || mode === "never") {
      visible = false;
    } else if (mode === "always") {
      visible = true;
    } else if (pageChanged) {
      visible = true;
      indicatorTimer = window.setTimeout(() => {
        visible = false;
        indicatorTimer = 0;
      }, 1100);
    } else {
      visible = false;
    }

  });

  $effect(() => () => clearTimeout(indicatorTimer));
</script>

{#if visible && activeTab && doc}
  <div
    class="page-indicator"
    aria-live="polite"
    aria-atomic="true"
  >
    {settings.pageIndicatorFormat === "page" ? `Página ${activeTab.page}` : activeTab.page}
  </div>
{/if}

<style>
  .page-indicator {
    position: fixed;
    top: calc(var(--titlebar-offset, 0px) + 12px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 89;
    padding: 7px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-bar);
    background: var(--custom-bar-frosted, var(--bg-titlebar));
    color: var(--text-primary);
    box-shadow: var(--shadow);
    backdrop-filter: blur(16px);
    font-size: 12px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
    user-select: none;
  }

  :global(body.opaque-surfaces) .page-indicator {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }
</style>
