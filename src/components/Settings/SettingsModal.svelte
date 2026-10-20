<script lang="ts">
  import { ui } from "$lib/stores/ui.svelte";
  import ThemeSection from "./ThemeSection.svelte";
  import LayoutSection from "./LayoutSection.svelte";
  import CategoriesEditor from "./CategoriesEditor.svelte";
  import AdvancedSection from "./AdvancedSection.svelte";
  import Icon from "$components/Icon.svelte";
  import { scrollbar, focusTrap } from "$lib/actions";
  import { settings } from "$lib/stores/settings.svelte";

  function close() {
    ui.settingsOpen = false;
  }

  function onOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) close();
  }

  function onOverlayKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") close();
  }
</script>

{#if ui.settingsOpen}
  <div
    class="overlay"
    role="dialog"
    aria-modal="true"
    aria-label="Configuración"
    tabindex="-1"
    onclick={onOverlayClick}
    onkeydown={onOverlayKeydown}
  >
    <div class="modal" use:focusTrap>
      <div class="modal-header">
        <div class="modal-title">Configuración</div>
        <button class="modal-close" type="button" title="Cerrar configuración" onclick={close}>
          <Icon name="close" size={14} />
        </button>
      </div>
      <div
        class="modal-body custom-scroll"
        use:scrollbar={{ always: settings.scrollbarMode === "always" }}
      >
        <LayoutSection />
        <ThemeSection />
        <CategoriesEditor />
        <AdvancedSection />
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
    padding: 0 18px 18px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
</style>
