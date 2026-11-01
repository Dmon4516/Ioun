<script lang="ts">
  import type { Comment } from "$lib/types";
  import {
    listComments, addComment, deleteComment,
  } from "$lib/tauri/commands";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { requestScrollToPage } from "$lib/stores/viewerCommands.svelte";
  import { showToast } from "$lib/stores/toast.svelte";
  import { confirmDialog } from "$lib/tauri/dialogs";
  import Icon from "$components/Icon.svelte";

  let { docId }: { docId: string } = $props();

  let items = $state<Comment[]>([]);
  let loading = $state(true);
  let collapsed = $state(true);
  let addOpen = $state(false);
  let draftText = $state("");
  let draftPage = $state("");
  let busy = $state(false);

  const activeTab = $derived(getActiveTab());

  let refreshGeneration = 0;

  async function refresh(id: string) {
    const token = ++refreshGeneration;
    loading = true;
    try {
      const list = await listComments(id);
      if (token !== refreshGeneration) return;
      items = list;
    } catch (err) {
      if (token !== refreshGeneration) return;
      console.error("[comments] No se pudieron cargar los comentarios", err);
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
    const text = draftText.trim();
    if (!text) return;
    const pageRaw = draftPage.trim();
    let page: number | null = null;
    if (pageRaw) {
      const parsed = Number(pageRaw);
      if (!Number.isFinite(parsed) || parsed < 1) {
        showToast("La página debe ser un número mayor o igual que 1.");
        return;
      }
      page = Math.round(parsed);
    }
    busy = true;
    try {
      await addComment(docId, page, text);
      draftText = "";
      draftPage = "";
      addOpen = false;
      await refresh(docId);
    } catch (error) {
      console.error("[comments] No se pudo guardar el comentario", error);
      showToast("No se pudo guardar el comentario.");
    } finally {
      busy = false;
    }
  }

  async function remove(id: string) {
    if (!(await confirmDialog("¿Eliminar este comentario?"))) return;
    try {
      await deleteComment(id);
      await refresh(docId);
    } catch (err) {
      console.error(err);
    }
  }

  function goTo(page: number | null) {
    if (page) requestScrollToPage(page, true);
  }

  function addCurrentPage() {
    draftPage = String(activeTab?.page ?? 1);
  }

  function formatDate(ms: number): string {
    return new Date(ms).toLocaleDateString();
  }
</script>

<div class="section">
  <div class="section-head">
    <div class="section-title">Comentarios</div>
    <div class="actions">
      <button
        class="mini-action"
        type="button"
        title={addOpen ? "Cerrar" : "Añadir comentario"}
        aria-label="Añadir comentario"
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
        <textarea
          class="textarea"
          rows="3"
          placeholder="Escribe un comentario…"
          bind:value={draftText}
          onkeydown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              void save();
            }
          }}
        ></textarea>
        <div class="add-row">
          <input
            class="page-input"
            type="number"
            min="1"
            placeholder="Página (opcional)"
            bind:value={draftPage}
          />
          <button class="btn-subtle small" type="button" onclick={addCurrentPage}>
            Actual
          </button>
          <button class="btn-primary small" type="button" disabled={busy} onclick={() => void save()}>
            Guardar
          </button>
        </div>
      </div>
    {/if}

    {#if loading}
      <div class="hint">Cargando…</div>
    {:else if items.length === 0}
      <div class="hint">Todavía no hay comentarios.</div>
    {:else}
      <ul class="list">
        {#each items as c (c.id)}
          <li class="comment">
            <button
              class="comment-text"
              type="button"
              disabled={!c.page}
              onclick={() => goTo(c.page)}
              title={c.page ? `Ir a página ${c.page}` : ""}
            >
              {c.text}
            </button>
            <div class="meta">
              {#if c.page}<span class="page-tag">pág. {c.page}</span>{/if}
              <span>{formatDate(c.createdAt)}</span>
            </div>
            <button
              class="mini-action delete"
              type="button"
              title="Eliminar"
              onclick={() => void remove(c.id)}
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
  .section { display: grid; gap: 6px; margin-top: 4px; }

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

  .textarea {
    width: 100%;
    min-height: 60px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--bg-input);
    padding: 8px 10px;
    outline: none;
    color: var(--text-primary);
    resize: vertical;
    font-family: inherit;
    font-size: 13px;
    line-height: 1.5;
  }
  .textarea:focus { border-color: color-mix(in srgb, var(--accent) 38%, var(--border)); }

  .add-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 6px;
  }

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

  .btn-primary,
  .btn-subtle {
    border-radius: 9px;
    font-size: 12px;
    font-weight: 650;
    padding: 0 10px;
    height: 32px;
    border: 1px solid var(--border);
    transition: background var(--animation-fast), border-color var(--animation-fast);
    white-space: nowrap;
  }
  .btn-primary {
    background: var(--accent);
    color: #071018;
    border-color: transparent;
  }
  .btn-primary:hover:not(:disabled) { filter: brightness(1.05); }
  .btn-primary:disabled { opacity: 0.5; }
  .btn-subtle { background: var(--bg-button); color: var(--text-primary); }
  .btn-subtle:hover { background: var(--bg-active); }

  .hint {
    color: var(--text-secondary);
    font-size: 12px;
    padding: 4px 2px 8px;
  }

  .list { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }

  .comment {
    position: relative;
    padding: 8px 30px 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    background: color-mix(in srgb, var(--bg-card) 40%, transparent);
  }

  .comment-text {
    display: block;
    width: 100%;
    text-align: left;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    cursor: default;
  }
  .comment-text:not(:disabled) { cursor: pointer; }
  .comment-text:not(:disabled):hover { color: var(--accent); }

  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
    font-size: 11px;
    color: var(--text-secondary);
  }
  .page-tag {
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 1px 6px;
  }

  .delete {
    position: absolute;
    top: 6px;
    right: 6px;
    opacity: 0;
  }
  .comment:hover .delete,
  .delete:focus-visible { opacity: 1; }
</style>
