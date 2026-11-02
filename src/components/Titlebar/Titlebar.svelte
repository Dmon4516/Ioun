<script lang="ts">
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { documents } from "$lib/stores/documents.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { computeMarquee } from "$lib/utils/marquee";
  import { displayTitle, pathSummary } from "$lib/utils/format";
  import Icon from "$components/Icon.svelte";

  let titleEl: HTMLElement | null = $state(null);
  let textEl: HTMLElement | null = $state(null);
  let infoWrapHovered = $state(false);

  const activeTab = $derived(getActiveTab());
  const doc = $derived(activeTab ? documents.items.get(activeTab.docId) ?? null : null);

  const title = $derived(doc ? displayTitle(doc.title, settings.showExtensions) : "Reign");
  const path = $derived(doc ? doc.path : "");
  const displayPath = $derived(pathSummary(path, settings.pathDepth));

  let marqueeDistance = $state(0);
  let marqueeDuration = $state(12);
  let marqueeActive = $state(false);

  function updateMarquee() {
    marqueeActive = false;
    marqueeDistance = 0;
    if (!settings.marqueeTitle) return;
    if (!titleEl || !textEl) return;

    // Defer a siguiente frame para que el DOM esté medido
    requestAnimationFrame(() => {
      if (!titleEl || !textEl) return;
      const cfg = computeMarquee(textEl, titleEl);
      if (!cfg) return;
      marqueeDistance = cfg.distance;
      marqueeDuration = cfg.duration;
      marqueeActive = true;
    });
  }

  $effect(() => {
    // Re-ejecuta cuando cambia el título o la ruta
    void title;
    void path;
    void settings.marqueeTitle;
    updateMarquee();
  });

  function toggleInfo() {
    ui.inspector = ui.inspector === "info" ? null : "info";
  }

  const infoActive = $derived(ui.inspector === "info");

</script>

<header
  class="titlebar"
  class:no-path={!settings.showTitlebarPath}
  class:info-hovered={infoWrapHovered}
  id="titlebarEl"
  data-tauri-drag-region="deep"
>
  <div class="titlebar-text">
    <div
      class="titlebar-info-wrap"
      role="presentation"
      onmouseenter={() => (infoWrapHovered = true)}
      onmouseleave={() => (infoWrapHovered = false)}
    >
      <div
        class="titlebar-title"
        class:marquee={marqueeActive}
        bind:this={titleEl}
        style:--marquee-distance={`${marqueeDistance}px`}
        style:--marquee-duration={`${marqueeDuration}s`}
      >
        <span class="titlebar-title-text" bind:this={textEl}>{title}</span>
      </div>
      {#if settings.showTitlebarPath && path}
        <div class="titlebar-path" title={path}>{displayPath}</div>
      {/if}
    </div>

    {#if doc}
      <button
        class="titlebar-info-toggle"
        class:active={infoActive}
        type="button"
        title="Información del documento"
        aria-label="Información del documento"
        aria-pressed={infoActive}
        onclick={toggleInfo}
      >
        <Icon name="info" size={14} />
      </button>
    {/if}
  </div>
</header>

<style>
  .titlebar {
    position: absolute;
    top: var(--gap-edge);
    left: var(--gap-edge);
    z-index: 80;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    padding-left: max(18px, calc(var(--titlebar-traffic-slot) + 8px));
    min-height: var(--titlebar-min-height, 0px);
    border-radius: var(--radius-bar);
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-titlebar));
    box-shadow: var(--shadow);
    backdrop-filter: blur(16px);
    width: fit-content;
    max-width: min(560px, calc(100% - var(--gap-edge) - var(--gap-edge) - var(--content-right, 0px)));
    /* Misma aparición que la barra flotante: fundido y un desplazamiento corto. */
    transition:
      max-width var(--animation-normal),
      opacity var(--animation-normal),
      transform var(--animation-normal);
    user-select: none;
  }

  :global(body.opaque-surfaces) .titlebar {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  :global(body.hide-titlebar-always) .titlebar,
  :global(body.reading.hide-titlebar-auto) .titlebar,
  :global(html[data-fullscreen="true"] body.hide-titlebar-fullscreen) .titlebar {
    opacity: 0;
    pointer-events: none;
    transform: translateY(-10px);
  }

  :global(html[data-titlebar-hover="true"] body.hide-titlebar-always) .titlebar,
  :global(html[data-titlebar-hover="true"] body.reading.hide-titlebar-auto) .titlebar,
  :global(html[data-titlebar-hover="true"][data-fullscreen="true"] body.hide-titlebar-fullscreen) .titlebar,
  .titlebar:focus-within {
    opacity: 1;
    pointer-events: auto;
    transform: translateY(0);
  }

  .titlebar-text {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0;
    flex: 0 1 auto;
  }


  .titlebar-info-wrap {
    min-width: 0;
    display: grid;
    gap: 2px;
    flex: 1;
    overflow: hidden;
    padding: 3px 8px;
    margin: -3px -8px;
    border-radius: 9px;
  }

  .titlebar-title {
    position: relative;
    padding-left: 8px;
    font-size: 13.5px;
    font-weight: 750;
    opacity: 0.96;
    letter-spacing: -0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .no-path .titlebar-title {
    font-size: 15px;
    font-weight: 780;
  }

  .titlebar-title-text {
    display: inline-block;
    vertical-align: top;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .titlebar-title.marquee {
    text-overflow: clip;
  }

  .titlebar-title.marquee .titlebar-title-text {
    max-width: none;
    text-overflow: clip;
    will-change: transform;
    animation: titleMarquee var(--marquee-duration, 12s) linear infinite;
  }

  .titlebar:hover .titlebar-title.marquee .titlebar-title-text,
  .titlebar.info-hovered .titlebar-title.marquee .titlebar-title-text {
    animation-play-state: paused;
  }

  @keyframes titleMarquee {
    0%, 6% { transform: translateX(0); }
    47%, 53% { transform: translateX(calc(-1 * var(--marquee-distance, 0px))); }
    94%, 100% { transform: translateX(0); }
  }

  .titlebar-path {
    font-size: 10px;
    color: var(--text-secondary);
    opacity: 0.72;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .titlebar-info-toggle {
    width: 0;
    height: 22px;
    border: none;
    padding: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    opacity: 0;
    pointer-events: none;
    overflow: hidden;
    margin-left: 0;
    transition:
      width var(--animation-normal),
      opacity var(--animation-normal),
      margin var(--animation-normal),
      background var(--animation-fast),
      color var(--animation-fast);
  }

  .titlebar:hover .titlebar-info-toggle,
  .titlebar.info-hovered .titlebar-info-toggle,
  .titlebar-info-toggle:focus-visible {
    width: 22px;
    margin-left: 6px;
    opacity: 1;
    pointer-events: auto;
  }

  .titlebar-info-toggle:hover {
    background: var(--bg-active);
    color: var(--text-primary);
  }

  .titlebar-info-toggle.active {
    color: var(--accent);
  }

  @media (prefers-reduced-motion: reduce) {
    .titlebar-title.marquee .titlebar-title-text {
      animation: none !important;
    }
  }
</style>
