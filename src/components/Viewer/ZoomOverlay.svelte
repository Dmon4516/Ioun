<script lang="ts">
  import { getActiveTab } from "$lib/stores/tabs.svelte";

  const activeTab = $derived(getActiveTab());
  const zoom = $derived(activeTab?.zoom ?? 100);

  let visible = $state(false);
  let lastZoom = 100;
  let timer = 0;

  $effect(() => {
    const z = zoom;
    if (Math.abs(z - lastZoom) < 0.5) return;
    lastZoom = z;
    visible = true;
    clearTimeout(timer);
    timer = window.setTimeout(() => {
      visible = false;
    }, 900);
  });

  $effect(() => () => clearTimeout(timer));
</script>

<div class="zoom-overlay" class:show={visible} aria-live="polite">
  {Math.round(zoom)}%
</div>

<style>
  .zoom-overlay {
    position: fixed;
    top: max(12px, calc(var(--titlebar-offset) + 6px));
    left: 50%;
    transform: translateX(-50%) translateY(-8px);
    z-index: 89;
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-menu));
    color: var(--text-primary);
    border-radius: var(--radius-bar);
    padding: 8px 12px;
    box-shadow: var(--shadow);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity var(--animation-normal),
      transform var(--animation-normal);
    font-size: 13px;
    font-weight: 600;
    backdrop-filter: blur(16px);
  }

  :global(body.opaque-surfaces) .zoom-overlay {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  .zoom-overlay.show {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
</style>
