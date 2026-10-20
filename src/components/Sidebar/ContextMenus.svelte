<script lang="ts">
  /**
   * Menú contextual ligero, autocontenido.
   * Se monta en el documento al mostrar y se destruye al ocultar.
   */
  import { clickOutside, focusTrap } from "$lib/actions";

  interface Item {
    label: string;
    danger?: boolean;
    action: () => void;
  }

  let {
    items = [],
    x = 0,
    y = 0,
    onClose,
  }: { items: Item[]; x: number; y: number; onClose: () => void } = $props();

  let el: HTMLElement | null = $state(null);
  let posX = $state(0);
  let posY = $state(0);

  $effect(() => {
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const maxX = Math.max(8, window.innerWidth - rect.width - 8);
    const maxY = Math.max(8, window.innerHeight - rect.height - 8);
    posX = Math.min(Math.max(8, x), maxX);
    posY = Math.min(Math.max(8, y), maxY);
  });
</script>

<div
  class="context-menu"
  bind:this={el}
  role="menu"
  tabindex="-1"
  style:left={`${posX}px`}
  style:top={`${posY}px`}
  use:focusTrap
  use:clickOutside={() => onClose()}
>
  {#each items as item (item.label)}
    <button
      class="context-item"
      class:danger={item.danger}
      type="button"
      role="menuitem"
      onclick={() => {
        item.action();
        onClose();
      }}
    >
      {item.label}
    </button>
  {/each}
</div>

<style>
  .context-menu {
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

  :global(body.opaque-surfaces) .context-menu {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  .context-item {
    display: block;
    width: 100%;
    padding: 9px 10px;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }

  .context-item:hover {
    background: var(--bg-active);
  }

  .context-item.danger {
    color: var(--danger);
  }
</style>
