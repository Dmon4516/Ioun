<script lang="ts">
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { documents } from "$lib/stores/documents.svelte";
  import { getModule } from "$lib/modules/registry";

  const activeTab = $derived(getActiveTab());
  const doc = $derived(activeTab ? documents.items.get(activeTab.docId) ?? null : null);
  const ModuleView = $derived.by(() => {
    if (!doc) return null;
    return getModule(doc.kind)?.View ?? null;
  });
</script>

{#if doc && activeTab && ModuleView}
  <ModuleView {doc} tab={activeTab}></ModuleView>
{:else}
  <section class="empty-wrap">
    <div class="empty-state">
      {#if doc}
        <h2>Formato no compatible</h2>
        <p>Ioun todavía no tiene un visor para este tipo de documento.</p>
      {:else}
        <h2>Ninguna pestaña activa</h2>
        <p>Abre un documento desde la barra de documentos o usa la búsqueda.</p>
      {/if}
    </div>
  </section>
{/if}

<style>
  .empty-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: var(--bg-content);
  }

  .empty-state {
    width: min(620px, 100%);
    margin: 6vh auto 0;
    border: 1px solid var(--border);
    background: var(--bg-card);
    border-radius: 26px;
    padding: 30px;
    box-shadow: var(--shadow);
  }

  .empty-state h2 { margin: 0 0 8px; font-size: 24px; }
  .empty-state p { margin: 0; color: var(--text-secondary); line-height: 1.6; }
</style>
