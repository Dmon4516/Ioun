<script lang="ts">
  import InfoPanel from "./InfoPanel.svelte";
  import OutlinePanel from "./OutlinePanel.svelte";
  import ThumbPanel from "./ThumbPanel.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { documents } from "$lib/stores/documents.svelte";
  import { scrollbar } from "$lib/actions";
  import { nextTick } from "$lib/utils/dom";
  import Icon from "$components/Icon.svelte";
  import { settings } from "$lib/stores/settings.svelte";

  const activeTab = $derived(getActiveTab());
  const doc = $derived(activeTab ? documents.items.get(activeTab.docId) ?? null : null);

  const panel = $derived(ui.inspector);

  // Mide el panel para ajustar `--inspector-top` según la colisión con
  // la barra de título (se aplica desde App.svelte globalmente).
  //
  // El rect del título cambia sin que cambie `panel`: al abrir un documento con un
  // título largo, al redimensionar la ventana o al aparecer/desaparecer la barra
  // lateral. Observando los dos rectángulos (y la ventana) el valor deja de
  // quedarse viejo y el inspector no acaba tapado por el título.
  $effect(() => {
    const el = document.getElementById("inspectorEl");
    if (!el || panel === null) return;
    const titlebar = document.getElementById("titlebarEl");

    let frame = 0;
    const schedule = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => updateInspectorTop());
    };

    const observer = new ResizeObserver(schedule);
    observer.observe(el);
    if (titlebar) observer.observe(titlebar);
    window.addEventListener("resize", schedule);
    void nextTick().then(schedule);

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      window.removeEventListener("resize", schedule);
    };
  });

  function updateInspectorTop() {
    const root = document.documentElement;
    const inspector = document.getElementById("inspectorEl");
    const titlebar = document.getElementById("titlebarEl");
    if (!inspector || !titlebar) return;
    const styles = getComputedStyle(root);
    const gapEdge = parseFloat(styles.getPropertyValue("--gap-edge")) || 10;
    const gapPanel = parseFloat(styles.getPropertyValue("--gap-panel")) || 10;
    const tb = titlebar.getBoundingClientRect();
    const insp = inspector.getBoundingClientRect();
    if (tb.right + gapPanel >= insp.left) {
      root.style.setProperty("--inspector-top", `${Math.round(tb.bottom + gapEdge)}px`);
    } else {
      root.style.setProperty("--inspector-top", `${gapEdge}px`);
    }
  }

  function close() {
    ui.inspector = null;
  }
</script>

<aside class="inspector" id="inspectorEl">
  <div class="panel-header">
    <div class="panel-tabs" role="tablist">
      <button
        class="panel-tab-button"
        class:active={panel === "info"}
        type="button"
        role="tab"
        aria-selected={panel === "info"}
        onclick={() => (ui.inspector = "info")}
      >Información</button>
      <button
        class="panel-tab-button"
        class:active={panel === "outline"}
        type="button"
        role="tab"
        aria-selected={panel === "outline"}
        onclick={() => (ui.inspector = "outline")}
      >Índice</button>
      <button
        class="panel-tab-button"
        class:active={panel === "thumbs"}
        type="button"
        role="tab"
        aria-selected={panel === "thumbs"}
        onclick={() => (ui.inspector = "thumbs")}
      >Miniaturas</button>
    </div>
    <button class="panel-close" type="button" title="Cerrar panel" onclick={close}>
      <Icon name="close" size={14} />
    </button>
  </div>

  <div
    class="inspector-content custom-scroll"
    id="inspectorContent"
    data-thin-scroll={panel === "thumbs" ? "" : undefined}
    use:scrollbar={{
      thin: panel === "thumbs",
      always: settings.scrollbarMode === "always",
    }}
  >
    {#if !doc}
      <div class="empty">
        <div class="panel-title">Sin documento</div>
        <p>Abre una pestaña para ver información, índice o miniaturas.</p>
      </div>
    {:else if panel === "info"}
      <InfoPanel {doc} />
    {:else if panel === "outline"}
      <OutlinePanel {doc} />
    {:else if panel === "thumbs"}
      <ThumbPanel {doc} />
    {/if}
  </div>
</aside>

<style>
  .inspector {
    position: absolute;
    top: var(--inspector-top, var(--gap-edge));
    right: var(--gap-edge);
    bottom: var(--gap-edge);
    width: var(--inspector-width);
    border-radius: var(--radius-panel);
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-sidebar));
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    z-index: 82;
    backdrop-filter: blur(18px);
    transition: top var(--animation-normal), width var(--animation-normal);
  }

  :global(body.inspector-outline-open) .inspector {
    width: var(--inspector-outline-width, min(480px, 90vw));
  }

  :global(body.opaque-surfaces) .inspector {
    background: var(--custom-bar-opaque, var(--opaque-sidebar));
    backdrop-filter: none;
  }

  :global(body.reading) .inspector {
    display: none;
  }

  .panel-header {
    position: relative;
    display: grid;
    /* Columnas simétricas: las pestañas quedan centradas en el panel y la X
       reserva su hueco, así su resaltado no se solapa con ellas. */
    grid-template-columns: 26px minmax(0, 1fr) 26px;
    align-items: center;
    gap: 4px;
    padding: 12px 8px;
    border-bottom: 1px solid var(--border);
  }

  .panel-tabs {
    grid-column: 2;
    display: flex;
    justify-content: center;
    gap: 4px;
    min-width: 0;
  }

  .panel-tab-button {
    flex: 0 1 auto;
    min-width: 0;
    padding: 0 8px;
    height: 34px;
    border-radius: 10px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background var(--animation-fast), color var(--animation-fast);
  }
  .panel-tab-button:hover { background: var(--bg-active); color: var(--text-primary); }
  .panel-tab-button.active { background: var(--bg-active); color: var(--accent); }

  .panel-close {
    grid-column: 3;
    width: 26px;
    height: 26px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    border-radius: 8px;
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    justify-self: end;
  }
  .panel-close:hover { background: var(--bg-active); color: var(--text-primary); }

  .inspector-content {
    box-sizing: border-box;
    flex: 1 1 auto;
    min-height: 0;
    padding: 14px;
    padding-bottom: 24px;
    overflow: auto;
    scroll-padding-bottom: 24px;
  }

  .empty { padding: 4px 0; }
  .empty .panel-title { margin: 0 0 8px; }
  .empty p { margin: 0; color: var(--text-secondary); font-size: 13px; line-height: 1.5; }

  .panel-title {
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
</style>
