<script lang="ts">
  import { syncStatus, syncingFolderName } from "$lib/stores/sync.svelte";
  import { cancelSync, syncSyncedFolders } from "$lib/tauri/commands";

  const visible = $derived(syncStatus.active || syncStatus.paused);
  const label = $derived(
    syncStatus.paused
      ? "Sincronización pausada"
      : `Sincronizando ${syncingFolderName() || "…"}`,
  );

  async function cancel(): Promise<void> {
    try {
      await cancelSync();
    } catch (error) {
      console.error("[sync] No se pudo cancelar el sync", error);
    }
  }

  async function resume(): Promise<void> {
    try {
      await syncSyncedFolders();
    } catch (error) {
      console.error("[sync] No se pudo reanudar el sync", error);
    }
  }
</script>

<div
  class="sync-pill"
  class:visible
  aria-hidden={!visible}
  aria-label={visible ? label : undefined}
>
  <span class="dot" class:paused={syncStatus.paused} aria-hidden="true"></span>
  <span class="sync-text" title={syncStatus.path}>{label}</span>
  {#if syncStatus.paused}
    <button
      class="action"
      type="button"
      onclick={resume}
      disabled={!visible}
      tabindex={visible ? 0 : -1}
    >Reanudar</button>
  {:else}
    <button
      class="action"
      type="button"
      onclick={cancel}
      disabled={!visible}
      tabindex={visible ? 0 : -1}
    >Cancelar</button>
  {/if}
</div>

<style>
  .sync-pill {
    position: fixed;
    left: calc(
      var(--content-left, 0px) +
      (100% - var(--content-left, 0px) - var(--content-right, 0px)) / 2
    );
    bottom: calc(var(--gap-edge) + 132px);
    transform: translateX(-50%) translateY(6px);
    z-index: 91;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(420px, calc(100vw - 24px));
    padding: 6px 6px 6px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-bar);
    background: var(--custom-bar-frosted, var(--bg-toolbar));
    box-shadow: var(--shadow);
    backdrop-filter: blur(18px);
    font-size: 12.5px;
    color: var(--text-primary);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity var(--animation-normal),
      transform var(--animation-normal);
  }

  .sync-pill.visible {
    opacity: 1;
    pointer-events: auto;
    transform: translateX(-50%) translateY(0);
  }

  :global(body.opaque-surfaces) .sync-pill {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: 0 0 auto;
    background: var(--accent);
    animation: syncPulse 1.2s ease-in-out infinite;
  }

  .dot.paused {
    background: var(--state-restoring, #ffb340);
    animation: none;
  }

  @keyframes syncPulse {
    0%, 100% { opacity: 0.45; }
    50% { opacity: 1; }
  }

  .sync-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .action {
    flex: 0 0 auto;
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-button);
    color: var(--text-primary);
    font-size: 11.5px;
    font-weight: 600;
  }

  .action:hover {
    background: var(--bg-active);
  }
</style>
