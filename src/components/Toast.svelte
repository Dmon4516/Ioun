<script lang="ts">
  import { toast, hideToast } from "$lib/stores/toast.svelte";
</script>

{#if toast.visible}
  <button class="toast" type="button" aria-live="polite" onclick={hideToast}>
    {toast.message}
  </button>
{/if}

<style>
  .toast {
    position: fixed;
    left: 50%;
    bottom: 92px;
    transform: translateX(-50%);
    z-index: 220;
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-menu));
    color: var(--text-primary);
    border-radius: var(--radius-bar);
    padding: 10px 14px;
    box-shadow: var(--shadow);
    backdrop-filter: blur(16px);
    font-size: 13px;
    animation: toastIn 200ms ease;
  }

  :global(body.opaque-surfaces) .toast {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  @keyframes toastIn {
    from { opacity: 0; transform: translate(-50%, 8px); }
    to { opacity: 1; transform: translate(-50%, 0); }
  }
</style>
