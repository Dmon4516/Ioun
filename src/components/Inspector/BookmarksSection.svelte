<script lang="ts">
  import type { Bookmark } from "$lib/types";
  import { listBookmarks, addBookmark, deleteBookmark } from "$lib/tauri/commands";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { requestScrollToPage } from "$lib/stores/viewerCommands.svelte";
  import { sectionTitle } from "$lib/utils/format";
  import Icon from "$components/Icon.svelte";
    import { showToast } from "$lib/stores/toast.svelte";

  let { docId }: { docId: string } = $props();

  let items = $state<Bookmark[]>([]);
  let loading = $state(true);
  let collapsed = $state(true);
  let addOpen = $state(false);
  let draftPage = $state("");
  let busy = $state(false);

  const activeTab = $derived(getActiveTab());

  let refreshGeneration = 0;

  async function refresh(id: string) {
    const token = ++refreshGeneration;
    loading = true;
    try {
      const list = await listBookmarks(id);
      if (token !== refreshGeneration) return;
      items = list;
    } catch (err) {
      if (token !== refreshGeneration) return;
      console.error("[bookmarks] No se pudieron cargar los marcadores", err);
      items = [];
    } finally {
      if (token === refreshGeneration) loading = false;
    }
  }

  $effect(() => {
    const id = docId;
    items = [];
    loading = true;
    void refresh(id);
  });

  async function save() {
    const pageRaw = draftPage.trim();
    const page = Number(pageRaw);
    if (!Number.isFinite(page) || page < 1) return;
    busy = true;
    try {
      await addBookmark(docId, Math.round(page), "");
      draftPage = "";
      addOpen = false;
      await refresh(docId);
    } catch (error) {
      console.error("[bookmarks] No se pudo guardar el marcador", error);
      showToast("No se pudo guardar el marcador.");
    } finally {
      busy = false;
    }
  }

  async function addCurrent() {
    const page = activeTab?.page ?? 1;
    busy = true;
    try {
      await addBookmark(docId, page, "");
      await refresh(docId);
    } catch (error) {
      console.error("[bookmarks] No se pudo guardar el marcador", error);
      showToast("No se pudo guardar el marcador.");
    } finally {
      busy = false;
    }
  }

  async function remove(id: string) {
    try {
      await deleteBookmark(id);
      await refresh(docId);
    } catch (err) {
      console.error(err);
    }
  }

  function goTo(page: number) {
    requestScrollToPage(page, true);
  }

  function placeholder(page: number): string {
    return sectionTitle(Math.ceil(page / 5));
  }
</script>

<div class="section">
  <div class="section-head">
    <div class="section-title">Marcadores</div>
    <div class="actions">
      <button
        class="mini-action"
        type="button"
        title={addOpen ? "Cerrar" : "Añadir marcador"}
        aria-label="Añadir marcador"
        onclick={() => (addOpen = !addOpen)}
      >
        <Icon name="plus" size={14} />
      </button>
      <button
        class="collapse-arrow"
        class:collapsed
        type="button"
        title={collapsed ? "Expandir" : "Contraer"}
        onclick={() => (collapsed = !collapsed)}
      >
        <Icon name="chevron-down" size={14} />
      </button>
    </div>
  </div>

  {#if !collapsed}
    {#if addOpen}
      <div class="add-panel">
        <button class="btn-subtle" type="button" disabled={busy} onclick={() => void addCurrent()}>
          Añadir página actual ({activeTab?.page ?? 1})
        </button>
        <div class="add-row">
          <input
            class="page-input"
            type="number"
            min="1"
            placeholder="Otra página"
            bind:value={draftPage}
            onkeydown={(e) => {
              if (e.key === "Enter") { e.preventDefault(); void save(); }
            }}
          />
          <button class="btn-primary small" type="button" disabled={busy} onclick={() => void save()}>
            Añadir
          </button>
        </div>
      </div>
    {/if}

    {#if loading}
      <div class="hint">Cargando…</div>
    {:else if items.length === 0}
      <div class="hint">Todavía no hay marcadores.</div>
    {:else}
      <ul class="list">
        {#each items as b (b.id)}
          <li class="bookmark">
            <button
              class="bm-main"
              type="button"
              onclick={() => goTo(b.page)}
              title={`Ir a página ${b.page}`}
            >
              <span class="bm-name">{b.name || placeholder(b.page)}</span>
              <span class="bm-page">pág. {b.page}</span>
            </button>
            <button
              class="mini-action delete"
              type="button"
              title="Eliminar"
              onclick={() => void remove(b.id)}
            >
              <Icon name="trash" size={12} />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .section { display: grid; gap: 6px; }

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin: 14px 0 6px;
  }

  .section-title {
    font-size: 12px;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }

  .actions { display: flex; align-items: center; gap: 4px; }

  .mini-action,
  .collapse-arrow {
    width: 26px; height: 26px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    transition: background var(--animation-fast), color var(--animation-fast), transform var(--animation-fast);
  }
  .mini-action:hover, .collapse-arrow:hover { background: var(--bg-active); color: var(--text-primary); }
  .collapse-arrow.collapsed { transform: rotate(-90deg); }

  .add-panel {
    display: grid;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    background: color-mix(in srgb, var(--bg-button) 42%, transparent);
    margin-bottom: 8px;
  }

  .add-row { display: grid; grid-template-columns: 1fr auto; gap: 6px; }

  .page-input {
    height: 32px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--bg-input);
    padding: 0 10px;
    outline: none;
    color: var(--text-primary);
    font-size: 12px;
    appearance: textfield;
    -moz-appearance: textfield;
  }
  .page-input::-webkit-outer-spin-button,
  .page-input::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .btn-primary, .btn-subtle {
    border-radius: 9px;
    font-size: 12px;
    font-weight: 650;
    padding: 0 12px;
    height: 32px;
    border: 1px solid var(--border);
    white-space: nowrap;
  }
  .btn-primary {
    background: var(--accent);
    color: #071018;
    border-color: transparent;
  }
  .btn-primary:disabled { opacity: 0.5; }
  .btn-subtle {
    background: var(--bg-button);
    color: var(--text-primary);
    width: 100%;
  }
  .btn-subtle:hover { background: var(--bg-active); }

  .hint {
    color: var(--text-secondary);
    font-size: 12px;
    padding: 4px 2px 8px;
  }

  .list { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }

  .bookmark {
    position: relative;
    display: flex;
    align-items: center;
  }

  .bm-main {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 7px 10px;
    border: none;
    border-radius: var(--radius-card);
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition: background var(--animation-fast);
  }
  .bm-main:hover { background: var(--bg-active); }

  .bm-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bm-page { color: var(--text-secondary); font-size: 12px; flex: 0 0 auto; }

  .delete { opacity: 0; }
  .bookmark:hover .delete,
  .delete:focus-visible { opacity: 1; }
</style>
