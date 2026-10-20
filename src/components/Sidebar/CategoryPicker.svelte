<script lang="ts">
  import {
    documents,
    toggleDocCategory,
    upsertDocument,
  } from "$lib/stores/documents.svelte";
  import { clearDocumentCategories } from "$lib/tauri/commands";
  import { showToast } from "$lib/stores/toast.svelte";
  import { clickOutside, focusTrap, portal } from "$lib/actions";

  let {
    docId,
    x,
    y,
    onClose,
  }: { docId: string; x: number; y: number; onClose: () => void } = $props();

  let el: HTMLElement | null = $state(null);
  let posX = $state(0);
  let posY = $state(0);

  const doc = $derived(documents.items.get(docId) ?? null);
  const current = $derived(new Set(doc?.categoryIds ?? []));

  $effect(() => {
    if (!el) return;
    el.focus();
    const rect = el.getBoundingClientRect();
    const maxX = Math.max(8, window.innerWidth - rect.width - 8);
    const maxY = Math.max(8, window.innerHeight - rect.height - 8);
    posX = Math.min(Math.max(8, x), maxX);
    posY = Math.min(Math.max(8, y), maxY);
  });

  async function choose(id: string): Promise<void> {
    if (!doc) {
      onClose();
      return;
    }
    if (id === "none") {
      const previousIds = [...doc.categoryIds];
      upsertDocument({ ...doc, categoryIds: [] });
      try {
        await clearDocumentCategories(doc.id);
        showToast("Documento sin categorías.");
        onClose();
      } catch (error) {
        upsertDocument({ ...doc, categoryIds: previousIds });
        console.error("[sidebar] No se pudieron quitar las categorías", error);
        showToast("No se pudieron quitar las categorías.");
      }
      return;
    }

    try {
      await toggleDocCategory(doc.id, id);
    } catch (error) {
      console.error("[sidebar] No se pudo actualizar la categoría", error);
      showToast("No se pudo actualizar la categoría.");
    }
  }
</script>

<div
  class="picker"
  bind:this={el}
  role="menu"
  tabindex="-1"
  style:left={`${posX}px`}
  style:top={`${posY}px`}
  use:portal
  use:focusTrap
  use:clickOutside={() => onClose()}
  onkeydown={(event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }}
>
  <div class="title">Categoría</div>
  <div class="list">
    {#each documents.categories as category (category.id)}
      <button
        class="item"
        class:selected={current.has(category.id)}
        type="button"
        role="menuitem"
        onclick={() => void choose(category.id)}
      >
        <span class="dot" style:background={category.color}></span>
        <span>{category.name}</span>
      </button>
    {/each}
    <button
      class="item"
      class:selected={current.size === 0}
      type="button"
      role="menuitem"
      onclick={() => void choose("none")}
    >
      <span class="dot" style:background="#8e8e93"></span>
      <span>Sin organizar</span>
    </button>
  </div>
</div>

<style>
  .picker {
    position: fixed;
    z-index: 200;
    min-width: 220px;
    border-radius: var(--radius-bar);
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-menu));
    box-shadow: var(--shadow-lg);
    padding: 6px;
    backdrop-filter: blur(18px);
  }

  :global(body.opaque-surfaces) .picker {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  .title {
    padding: 8px 10px 4px;
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }

  .list { display: grid; gap: 2px; }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }

  .item:hover,
  .item.selected { background: var(--bg-active); }

  .item.selected::after {
    content: "✓";
    margin-left: auto;
    color: var(--accent);
  }

  .dot { width: 8px; height: 8px; border-radius: 50%; }
</style>
