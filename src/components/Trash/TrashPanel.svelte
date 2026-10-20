<script lang="ts">
  import type { TrashItem } from "$lib/types";
  import { ui } from "$lib/stores/ui.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { emptyAll, refreshTrash, restoreItem, trash } from "$lib/stores/trash.svelte";
  import { confirmDialog } from "$lib/tauri/dialogs";
  import { scrollbar, focusTrap } from "$lib/actions";
  import Icon from "$components/Icon.svelte";

  const KIND_LABELS: Record<string, string> = {
    folder: "Carpeta",
    document: "Documento",
    category: "Categoría",
    bookmark: "Marcador",
    comment: "Comentario",
  };

  let busy = $state(false);

  $effect(() => {
    if (ui.trashOpen) void refreshTrash();
  });

  function close(): void {
    ui.trashOpen = false;
  }

  function onOverlayClick(e: MouseEvent): void {
    if (e.target === e.currentTarget) close();
  }

  function onOverlayKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") close();
  }

  function kindLabel(item: TrashItem): string {
    return KIND_LABELS[item.kind] ?? item.kind;
  }

  function formatDate(createdAt: number): string {
    if (!createdAt) return "";
    return new Date(createdAt).toLocaleString("es-ES", {
      day: "2-digit",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  async function handleRestore(item: TrashItem): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      await restoreItem(item.id);
    } finally {
      busy = false;
    }
  }

  async function handleEmpty(): Promise<void> {
    if (busy) return;
    const confirmed = await confirmDialog(
      "¿Vaciar la papelera? Esta acción no se puede deshacer.",
    );
    if (!confirmed) return;
    busy = true;
    try {
      await emptyAll();
    } finally {
      busy = false;
    }
  }
</script>

{#if ui.trashOpen}
  <div
    class="overlay"
    role="dialog"
    aria-modal="true"
    aria-label="Papelera"
    tabindex="-1"
    onclick={onOverlayClick}
    onkeydown={onOverlayKeydown}
  >
    <div class="modal" use:focusTrap>
      <div class="modal-header">
        <div class="modal-title">Papelera</div>
        <button class="modal-close" type="button" title="Cerrar papelera" onclick={close}>
          <Icon name="close" size={14} />
        </button>
      </div>

      <div
        class="modal-body custom-scroll"
        use:scrollbar={{ always: settings.scrollbarMode === "always" }}
      >
        {#if trash.items.length === 0}
          <p class="empty">
            No hay nada en la papelera. Lo que elimines desde el árbol de archivos aparecerá aquí y
            podrás recuperarlo.
          </p>
        {:else}
          <ul class="items">
            {#each trash.items as item (item.id)}
              <li class="item">
                <div class="item-text">
                  <span class="item-label">{item.label}</span>
                  <span class="item-meta">
                    {kindLabel(item)}
                    {#if formatDate(item.createdAt)}· {formatDate(item.createdAt)}{/if}
                  </span>
                </div>
                <button
                  class="restore"
                  type="button"
                  disabled={busy}
                  title="Restaurar"
                  onclick={() => void handleRestore(item)}
                >
                  <Icon name="rotate-ccw" size={14} />
                  Restaurar
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      <div class="modal-footer">
        <span class="count">
          {trash.items.length}
          {trash.items.length === 1 ? "elemento" : "elementos"}
        </span>
        <button
          class="empty-all"
          type="button"
          disabled={busy || trash.items.length === 0}
          onclick={() => void handleEmpty()}
        >
          <Icon name="trash" size={14} />
          Vaciar papelera
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 150;
    background: var(--overlay);
    display: grid;
    place-items: center;
    padding: 18px;
    backdrop-filter: blur(4px);
  }

  .modal {
    width: min(560px, 100%);
    max-height: min(84vh, 820px);
    border-radius: 22px;
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-menu));
    box-shadow: var(--shadow-lg);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    backdrop-filter: blur(20px);
  }
  :global(body.opaque-surfaces) .modal {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 18px 18px 10px;
    flex: 0 0 auto;
  }

  .modal-title { font-size: 19px; font-weight: 750; letter-spacing: -0.02em; }

  .modal-close {
    width: 34px;
    height: 34px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg-button);
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    transition: background var(--animation-fast), color var(--animation-fast);
  }
  .modal-close:hover { background: var(--bg-active); color: var(--text-primary); }

  .modal-body {
    padding: 0 18px 10px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .empty {
    margin: 4px 0 10px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-secondary);
  }

  .items {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 10px;
  }
  .item:hover { background: var(--bg-hover); }

  .item-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .item-label {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-meta {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .restore {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 auto;
    height: 28px;
    padding: 0 10px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-button);
    color: var(--text-secondary);
    font-size: 12px;
    transition: background var(--animation-fast), color var(--animation-fast);
  }
  .restore:hover:not(:disabled) { background: var(--bg-active); color: var(--text-primary); }
  .restore:disabled { opacity: 0.5; }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 18px 18px;
    flex: 0 0 auto;
  }

  .count {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .empty-all {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 12px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--bg-button);
    color: var(--danger, #ff453a);
    font-size: 12.5px;
    font-weight: 600;
    transition: background var(--animation-fast);
  }
  .empty-all:hover:not(:disabled) { background: var(--bg-active); }
  .empty-all:disabled { opacity: 0.5; }
</style>
