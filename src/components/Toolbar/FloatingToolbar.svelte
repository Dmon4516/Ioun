<script lang="ts">
  import { clickOutside } from "$lib/actions";
  import { settings, updateSettings } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { getActiveTab, updateTabPage } from "$lib/stores/tabs.svelte";
  import { documents } from "$lib/stores/documents.svelte";
  import {
    commandFitPreserveCenter,
    commandZoomToCenter,
    scrollViewerBy,
    viewer,
  } from "$lib/stores/viewer.svelte";
  import { requestScrollToPage } from "$lib/stores/viewerCommands.svelte";
  import { clamp } from "$lib/utils/colors";
  import { MAX_ZOOM, MIN_ZOOM, SCROLL_STEP_PAGE } from "$lib/constants";
  import Icon from "$components/Icon.svelte";

  const activeTab = $derived(getActiveTab());
  const doc = $derived(activeTab ? documents.items.get(activeTab.docId) ?? null : null);

  const canScrollUp = $derived(
    !!activeTab && activeTab.status !== "restoring" && !viewer.scrollAtTop,
  );
  const canScrollDown = $derived(
    !!activeTab && activeTab.status !== "restoring" && !viewer.scrollAtBottom,
  );
  const disabled = $derived(!activeTab || activeTab.status === "restoring");

  let pageInputEl: HTMLInputElement | null = $state(null);
  let zoomInputEl: HTMLInputElement | null = $state(null);

  // Reflejar el valor del store en el input cuando cambie externamente
  $effect(() => {
    const p = activeTab?.page ?? 1;
    if (pageInputEl && document.activeElement !== pageInputEl) {
      pageInputEl.value = `${p} / ${doc?.pages ?? 0}`;
    }
  });

  $effect(() => {
    const z = Math.round(activeTab?.zoom ?? 100);
    if (zoomInputEl && document.activeElement !== zoomInputEl) {
      zoomInputEl.value = String(z);
    }
  });

  // ── Navegación ────────────────────────────────────────────────────
  function goToPage(p: number) {
    if (!activeTab || !doc) return;
    const upper = doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
    const target = clamp(Math.round(p) || 1, 1, upper);
    updateTabPage(activeTab.id, target, activeTab.zoom);
    requestScrollToPage(target, true, "top");
  }

  // Paso de pantalla por clic: los botones arriba/abajo acompañan a la
  // navegación del visor; el desplazamiento fino queda en las flechas del
  // teclado (Shift+flecha para el paso de pantalla).
  function scrollStep(direction: 1 | -1) {
    scrollViewerBy(direction, SCROLL_STEP_PAGE.fraction, SCROLL_STEP_PAGE.minPx);
  }

  function commitPageInput() {
    if (!pageInputEl) return;
    const raw = pageInputEl.value.trim();
    const n = Number(raw);
    if (raw !== "" && Number.isFinite(n)) goToPage(n);
    else if (activeTab) pageInputEl.value = `${activeTab.page} / ${doc?.pages ?? 0}`;
    pageInputEl.blur();
  }

  // ── Zoom ──────────────────────────────────────────────────────────
  function zoomBy(factor: number) {
    if (!activeTab) return;
    const next = clamp(activeTab.zoom * factor, MIN_ZOOM, MAX_ZOOM);
    commandZoomToCenter(next);
  }

  function zoomOut() { zoomBy(1 / 1.15); }
  function zoomIn() { zoomBy(1.15); }

  function commitZoomInput() {
    if (!zoomInputEl || !activeTab) return;
    const raw = zoomInputEl.value.trim();
    const n = Number(raw);
    if (raw !== "" && Number.isFinite(n)) {
      const next = clamp(n, MIN_ZOOM, MAX_ZOOM);
      commandZoomToCenter(next);
    } else {
      zoomInputEl.value = String(Math.round(activeTab.zoom));
    }
    zoomInputEl.blur();
  }

  // ── Fit menu ──────────────────────────────────────────────────────
  let fitMenuOpen = $state(false);
  let fitBtnEl: HTMLButtonElement | null = $state(null);
  let fitMenuEl: HTMLDivElement | null = $state(null);
  let fitMenuX = $state(0);
  let fitMenuY = $state(0);
  let holdTimer = 0;
  let holdTriggered = false;
  let suppressFitClick = false;

  function onFitPointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    holdTriggered = false;
    suppressFitClick = false;
    clearTimeout(holdTimer);
    holdTimer = window.setTimeout(() => {
      holdTriggered = true;
      openFitMenu();
    }, 400);
  }

  function onFitPointerUp(): void {
    clearTimeout(holdTimer);
    if (holdTriggered) {
      suppressFitClick = true;
      holdTriggered = false;
      window.setTimeout(() => {
        suppressFitClick = false;
      }, 0);
    }
  }

  function onFitPointerLeave(): void {
    if (!holdTriggered) clearTimeout(holdTimer);
  }

  function onFitClick(): void {
    if (suppressFitClick) return;
    setFitMode("page");
  }

  function openFitMenu() {
    if (!fitBtnEl) return;
    const r = fitBtnEl.getBoundingClientRect();
    // Se calcula en el siguiente frame para tener el ancho real
    fitMenuOpen = true;
    requestAnimationFrame(() => {
      if (!fitMenuEl) return;
      const mr = fitMenuEl.getBoundingClientRect();
      const toolbar = document.getElementById("floatingToolbar");
      const isVertical = toolbar ? getComputedStyle(toolbar).flexDirection === "column" : false;
      const maxX = Math.max(8, window.innerWidth - mr.width - 8);
      const maxY = Math.max(8, window.innerHeight - mr.height - 8);
      if (isVertical && toolbar) {
        const tr = toolbar.getBoundingClientRect();
        fitMenuX = clamp(tr.right + 10, 8, maxX);
        fitMenuY = clamp(r.top + r.height / 2 - mr.height / 2, 8, maxY);
      } else {
        fitMenuX = clamp(r.left + r.width / 2 - mr.width / 2, 8, maxX);
        fitMenuY = clamp(r.top - mr.height - 8, 8, maxY);
      }
      fitMenuEl?.querySelector<HTMLButtonElement>("button")?.focus();
    });
  }

  function setFitMode(mode: "spread" | "page" | "fill"): void {
    const currentZoom = activeTab?.zoom;
    updateSettings("fitMode", mode);
    if (currentZoom !== undefined && Math.abs(currentZoom - 100) < 0.01) {
      commandFitPreserveCenter();
    }
    else commandZoomToCenter(100);
    fitMenuOpen = false;
  }

  // ── Botones de estado ─────────────────────────────────────────────
  function toggleSidebar() {
    ui.showSidebar = !ui.showSidebar;
  }
  function togglePanel() {
    ui.inspector = ui.inspector ? null : settings.defaultPanel;
  }
  function toggleReading() {
    ui.reading = !ui.reading;
    if (ui.reading) ui.inspector = null;
  }
  function openSettings() {
    ui.settingsOpen = true;
  }
  function openSearch() {
    ui.searchOpen = true;
  }
</script>

<div class="floating-toolbar" id="floatingToolbar">
  <button
    class="tool-button icon-only"
    class:active={ui.searchOpen}
    type="button"
    title="Buscar"
    onclick={openSearch}
  >
    <Icon name="search" size={15} />
  </button>

  <button
    class="tool-button icon-only"
    class:active={ui.showSidebar}
    type="button"
    title="Barra de documentos"
    onclick={toggleSidebar}
  >
    <Icon name="folder" size={15} />
  </button>

  <div class="divider"></div>

  <button
    class="tool-button"
    type="button"
    title="Desplazar arriba"
    disabled={!canScrollUp}
    onclick={() => scrollStep(-1)}
  >
    <Icon name="arrow-up" size={15} />
  </button>

  <input
    class="page-input"
    type="text"
    inputmode="numeric"
    autocomplete="off"
    aria-label="Página actual y total de páginas"
    bind:this={pageInputEl}
    value={`${activeTab?.page ?? 1} / ${doc?.pages ?? 0}`}
    disabled={disabled}
    onfocus={() => {
      if (pageInputEl) pageInputEl.value = String(activeTab?.page ?? 1);
    }}
    onkeydown={(e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        pageInputEl?.blur();
      }
    }}
    onblur={commitPageInput}
  />

  <button
    class="tool-button"
    type="button"
    title="Desplazar abajo"
    disabled={!canScrollDown}
    onclick={() => scrollStep(1)}
  >
    <Icon name="arrow-down" size={15} />
  </button>

  <div class="divider"></div>

  <button class="tool-button" type="button" title="Reducir zoom" disabled={disabled} onclick={zoomOut}>
    <Icon name="minus" size={15} />
  </button>

  <input
    class="zoom-input"
    type="number"
    min="40"
    max="999"
    step="1"
    aria-label="Zoom"
    bind:this={zoomInputEl}
    value={Math.round(activeTab?.zoom ?? 100)}
    disabled={disabled}
    onkeydown={(e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        zoomInputEl?.blur();
      }
    }}
    onblur={commitZoomInput}
  />
  <span class="zoom-percent">%</span>

  <button class="tool-button" type="button" title="Aumentar zoom" disabled={disabled} onclick={zoomIn}>
    <Icon name="plus" size={15} />
  </button>

  <button
    class="tool-button icon-only"
    type="button"
    title="Ajustar a página (mantén para más opciones)"
    disabled={disabled}
    bind:this={fitBtnEl}
    onclick={onFitClick}
    onpointerdown={onFitPointerDown}
    onpointerup={onFitPointerUp}
    onpointerleave={onFitPointerLeave}
    onpointercancel={onFitPointerLeave}
  >
    <Icon name="fit" size={15} />
  </button>

  <div class="divider"></div>

  <button
    class="tool-button icon-only"
    class:active={!!ui.inspector}
    type="button"
    title="Barra de información"
    onclick={togglePanel}
  >
    <Icon name="grid" size={15} />
  </button>

  <div class="divider"></div>

  <button
    class="tool-button icon-only"
    class:active={ui.reading}
    type="button"
    title="Modo lectura"
    onclick={toggleReading}
  >
    <Icon name="book" size={15} />
  </button>

  <button class="tool-button icon-only" type="button" title="Configuración" onclick={openSettings}>
    <Icon name="settings" size={15} />
  </button>
</div>

{#if fitMenuOpen}
  <div
    class="fit-menu"
    bind:this={fitMenuEl}
    role="menu"
    tabindex="-1"
    use:clickOutside={() => (fitMenuOpen = false)}
    onkeydown={(event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        fitMenuOpen = false;
      }
    }}
    style:left={`${fitMenuX}px`}
    style:top={`${fitMenuY}px`}
  >
    <div class="fit-menu-title">Modo de ajuste</div>
    <button
      class="fit-menu-item"
      class:active={settings.fitMode === "page"}
      type="button"
      onclick={() => setFitMode("page")}
    >Página completa</button>
    <button
      class="fit-menu-item"
      class:active={settings.fitMode === "spread"}
      type="button"
      onclick={() => setFitMode("spread")}
    >Ajustar al ancho</button>
    <button
      class="fit-menu-item"
      class:active={settings.fitMode === "fill"}
      type="button"
      onclick={() => setFitMode("fill")}
    >Rellenar bordes</button>
  </div>
{/if}

<style>
  .floating-toolbar {
    position: absolute;
    left: calc(
      var(--content-left, 0px) +
      (100% - var(--content-left, 0px) - var(--content-right, 0px)) / 2
    );
    transform: translateX(-50%);
    bottom: var(--gap-edge);
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px;
    border-radius: var(--radius-bar);
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-toolbar));
    box-shadow: var(--shadow);
    backdrop-filter: blur(18px);
    z-index: 89;
    max-width: calc(100% - 24px);
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    transition:
      top var(--animation-normal),
      left var(--animation-normal),
      opacity var(--animation-normal),
      transform var(--animation-normal);
  }
  .floating-toolbar::-webkit-scrollbar { display: none; }

  :global(body.opaque-surfaces) .floating-toolbar {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  :global(body.toolbar-hidden) .floating-toolbar {
    opacity: 0;
    pointer-events: none;
    transform: translateX(-50%) translateY(10px);
  }

  :global(body.reading) :global(#sidebarToggleButton) { display: none; }

  .tool-button {
    min-width: 35px;
    height: 35px;
    padding: 0 8px;
    border: none;
    border-radius: 10px;
    background: transparent;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    white-space: nowrap;
    transition: background var(--animation-fast), color var(--animation-fast);
  }
  .tool-button.icon-only { padding: 0; }
  .tool-button:hover:not(:disabled) { background: var(--bg-active); }
  .tool-button.active { background: var(--bg-active); color: var(--accent); }
  .tool-button:disabled { opacity: 0.38; cursor: default; }

  .divider {
    width: 1px;
    height: 23px;
    background: var(--border);
    margin: 0 3px;
    flex: 0 0 auto;
  }

  .page-input,
  .zoom-input {
    height: 35px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg-input);
    text-align: center;
    outline: none;
    color: var(--text-primary);
    appearance: textfield;
    -moz-appearance: textfield;
  }
  .page-input { width: 88px; }
  .zoom-input { width: 64px; }

  .page-input::-webkit-outer-spin-button,
  .page-input::-webkit-inner-spin-button,
  .zoom-input::-webkit-outer-spin-button,
  .zoom-input::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .page-input:focus,
  .zoom-input:focus { border-color: color-mix(in srgb, var(--accent) 42%, var(--border)); }

  .zoom-percent {
    font-size: 12px;
    color: var(--text-secondary);
    margin-left: 2px;
  }

  .fit-menu {
    position: fixed;
    z-index: 130;
    min-width: 230px;
    border-radius: var(--radius-bar);
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-menu));
    box-shadow: var(--shadow-lg);
    padding: 6px;
    backdrop-filter: blur(18px);
  }
  :global(body.opaque-surfaces) .fit-menu {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  .fit-menu-title {
    padding: 8px 10px 4px;
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }

  .fit-menu-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 9px 10px;
    border: none;
    background: transparent;
    color: var(--text-primary);
    text-align: left;
    border-radius: 9px;
    font-size: 13px;
    cursor: pointer;
  }
  .fit-menu-item:hover { background: var(--bg-active); }
  .fit-menu-item.active::after { content: "✓"; color: var(--accent); margin-left: 8px; }

  @media (max-width: 920px) {
    .floating-toolbar {
      left: calc(var(--sidebar-width) + var(--gap-edge) + var(--gap-edge));
      bottom: auto;
      top: max(var(--gap-edge), var(--titlebar-reserve));
      transform: none;
      flex-direction: column;
      align-items: center;
      width: 100px;
      max-width: 100px;
      max-height: calc(100% - max(var(--gap-edge), var(--titlebar-reserve)) - var(--gap-edge));
      overflow-y: auto;
      overflow-x: hidden;
      padding: 8px 6px;
      gap: 7px;
    }
    :global(body.no-sidebar:not(.reading)) .floating-toolbar,
    :global(body.reading) .floating-toolbar {
      left: 18px;
    }
    :global(body.toolbar-hidden) .floating-toolbar {
      transform: translateX(-10px);
    }
    .divider {
      width: 26px;
      height: 1px;
      margin: 2px 0;
    }
    .page-input { width: 88px; height: 34px; }
    .zoom-input { width: 64px; height: 34px; }
    .zoom-percent { display: none; }
    .tool-button { width: 38px; min-width: 38px; height: 38px; }
  }
</style>
