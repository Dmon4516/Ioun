<script lang="ts">
  import { onMount } from "svelte";
  import { documents, setCategories } from "$lib/stores/documents.svelte";
  import { settings, updateSettings } from "$lib/stores/settings.svelte";
  import { listCategories, upsertCategory, deleteCategory } from "$lib/tauri/commands";
  import type { Category } from "$lib/types";
  import { uid } from "$lib/utils/id";
  import { confirmDialog, alertDialog } from "$lib/tauri/dialogs";
  import Icon from "$components/Icon.svelte";

  let busy = $state(false);
  onMount(() => {
    void refresh();
  });

  async function refresh() {
    try {
      const cats = await listCategories();
      setCategories(cats);
    } catch (err) {
      console.error(err);
    }
  }

  async function addOne() {
    const position = documents.categories.reduce(
      (max, category) => Math.max(max, Number.isFinite(category.position) ? category.position : -1),
      -1,
    ) + 1;
    const cat: Category = {
      id: uid("cat"),
      name: "Nueva categoría",
      color: "#ff6b6b",
      position,
    };
    busy = true;
    try {
      await upsertCategory(cat);
      await refresh();
    } finally {
      busy = false;
    }
  }

  async function rename(cat: Category, name: string) {
    const next = name.trim() || "Sin nombre";
    if (next === cat.name) return;
    busy = true;
    try {
      await upsertCategory({ ...cat, name: next });
      await refresh();
    } finally {
      busy = false;
    }
  }

  async function recolor(cat: Category, color: string) {
    if (color === cat.color) return;
    busy = true;
    try {
      await upsertCategory({ ...cat, color });
      await refresh();
    } finally {
      busy = false;
    }
  }

  async function remove(cat: Category) {
    if (documents.categories.length <= 1) {
      await alertDialog("Debe quedar al menos una categoría.");
      return;
    }
    if (!(await confirmDialog(`¿Eliminar la categoría "${cat.name}"?`))) return;
    busy = true;
    try {
      await deleteCategory(cat.id);
      await refresh();
    } finally {
      busy = false;
    }
  }
</script>

<div class="settings-group">
  <div class="settings-group-title">Categorías</div>

  <div class="row">
    <div>
      <div class="label">Activar categorías</div>
      <div class="desc">Permite organizar y filtrar archivos por categoría.</div>
    </div>
    <button
      class="switch"
      class:active={settings.categoriesEnabled}
      type="button"
      role="switch"
      aria-label="Activar categorías"
      aria-checked={settings.categoriesEnabled}
      onclick={() => updateSettings("categoriesEnabled", !settings.categoriesEnabled)}
    ></button>
  </div>

  {#if settings.categoriesEnabled}
    {#if documents.categories.length === 0}
      <div class="hint">No hay categorías. Añade una para empezar.</div>
    {:else}
      <div class="editor">
        {#each documents.categories as cat (cat.id)}
          <div class="editor-row">
            <label class="editor-color" style:background={cat.color} title="Color">
              <input
                type="color"
                value={cat.color}
                onchange={(e) => void recolor(cat, e.currentTarget.value)}
              />
            </label>
            <input
              class="editor-name"
              type="text"
              value={cat.name}
              disabled={busy}
              onchange={(e) => void rename(cat, e.currentTarget.value)}
            />
            <button
              class="mini-action"
              type="button"
              title="Eliminar categoría"
              disabled={busy}
              onclick={() => void remove(cat)}
            >
              <Icon name="trash" size={12} />
            </button>
          </div>
        {/each}
      </div>
    {/if}

    <button class="subtle-button" type="button" disabled={busy} onclick={() => void addOne()}>
      Añadir categoría
    </button>
  {/if}
</div>

<style>
  .settings-group {
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg-button) 36%, transparent);
    border-radius: var(--radius-panel);
    padding: 13px;
    margin-top: 12px;
  }
  .settings-group-title {
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-secondary);
    margin-bottom: 10px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 6px 0 10px;
  }

  .label { font-size: 13px; font-weight: 630; }
  .desc { margin-top: 3px; font-size: 12px; color: var(--text-secondary); line-height: 1.45; max-width: 280px; }

  .switch {
    width: 44px;
    height: 27px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--bg-button);
    position: relative;
    flex: 0 0 auto;
  }
  .switch::after {
    content: "";
    position: absolute;
    width: 21px;
    height: 21px;
    border-radius: 50%;
    background: #fff;
    top: 2px;
    left: 2px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.22);
    transition: transform var(--animation-fast);
  }
  .switch.active {
    background: color-mix(in srgb, var(--accent) 24%, var(--bg-button));
    border-color: color-mix(in srgb, var(--accent) 42%, var(--border));
  }
  .switch.active::after { transform: translateX(17px); }

  .hint { color: var(--text-secondary); font-size: 12px; padding: 6px 0; }

  .editor { display: grid; gap: 8px; margin-bottom: 10px; }

  .editor-row {
    display: grid;
    grid-template-columns: 32px 1fr auto;
    gap: 8px;
    align-items: center;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    background: color-mix(in srgb, var(--bg-button) 42%, transparent);
  }

  .editor-color {
    width: 32px;
    height: 32px;
    border-radius: 9px;
    border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
    position: relative;
    overflow: hidden;
    cursor: pointer;
  }
  .editor-color input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
  }

  .editor-name {
    height: 32px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--bg-input);
    padding: 0 10px;
    outline: none;
    width: 100%;
    font-size: 13px;
    color: var(--text-primary);
  }

  .mini-action {
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
  }
  .mini-action:hover { background: var(--bg-active); color: var(--text-primary); }
  .mini-action:disabled { opacity: 0.4; pointer-events: none; }

  .subtle-button {
    width: 100%;
    border: 1px solid var(--border);
    background: var(--bg-button);
    color: var(--text-primary);
    border-radius: var(--radius-card);
    padding: 11px 12px;
    font-size: 13px;
    font-weight: 650;
  }
  .subtle-button:hover:not(:disabled) { background: var(--bg-active); }
</style>
