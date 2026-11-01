<script lang="ts">
  import type { DocumentRecord } from "$lib/types";
  import { ui } from "$lib/stores/ui.svelte";
  import { documents, updateDocView } from "$lib/stores/documents.svelte";
  import { effectivePagesPerRow } from "$lib/utils/documentView";
  import {
    openDocument,
    setActiveTab,
    getTabByDocId,
    getActiveTab,
    updateTabPage,
    restoreLastClosedTab,
  } from "$lib/stores/tabs.svelte";
  import { settings, updateSettings } from "$lib/stores/settings.svelte";
  import { displayTitle, pathSummary } from "$lib/utils/format";
  import { commandZoomToCenter } from "$lib/stores/viewer.svelte";
  import { requestScrollToPage } from "$lib/stores/viewerCommands.svelte";
  import { clamp } from "$lib/utils/colors";
  import { SCROLL_STEP_FINE, SCROLL_STEP_PAGE } from "$lib/constants";
  import { SHORTCUTS } from "$lib/utils/shortcuts";
  import Icon from "$components/Icon.svelte";
  import { focusOnMount, focusTrap, scrollbar } from "$lib/actions";
  import { showToast } from "$lib/stores/toast.svelte";

  interface Item {
    kind: "doc" | "action";
    label: string;
    hint: string;
    command?: string;
    action: () => void;
  }

  let inputEl: HTMLInputElement | null = $state(null);
  let popupEl: HTMLDivElement | null = $state(null);
  let query = $state("");
  let index = $state(0);
  let debounceTimer = 0;

  async function updateActiveDocumentView(
    patch: { pagesPerRow?: 1 | 2 | 3 | 4; continuousScroll?: boolean },
  ): Promise<void> {
    const tab = getActiveTab();
    const doc = tab ? documents.items.get(tab.docId) : undefined;
    if (!doc) {
      if (patch.pagesPerRow !== undefined) updateSettings("pagesPerRow", patch.pagesPerRow);
      if (patch.continuousScroll !== undefined) {
        updateSettings("continuousScroll", patch.continuousScroll);
      }
      return;
    }
    try {
      await updateDocView(doc.id, patch);
    } catch (error) {
      console.error("[search] No se pudo guardar la vista del documento", error);
      showToast("No se pudo guardar la vista del documento.");
    }
  }

  function setPagesPerRow(pagesPerRow: 1 | 2 | 3 | 4): void {
    void updateActiveDocumentView({ pagesPerRow });
  }

  function toggleContinuousScroll(): void {
    const tab = getActiveTab();
    const doc = tab ? documents.items.get(tab.docId) : undefined;
    const current = doc
      ? (doc.view.continuousScroll ?? settings.continuousScroll)
      : settings.continuousScroll;
    void updateActiveDocumentView({ continuousScroll: !current });
  }

  function scrollViewer(direction: 1 | -1, fine: boolean): void {
    const area = document.getElementById("viewerArea");
    if (!area) return;
    const stepConfig = fine ? SCROLL_STEP_FINE : SCROLL_STEP_PAGE;
    const step = Math.max(stepConfig.minPx, area.clientHeight * stepConfig.fraction);
    area.scrollBy({ top: direction * step, behavior: "smooth" });
  }

  function goToPage(target: number | "first" | "last"): void {
    const tab = getActiveTab();
    if (!tab) return;
    const doc = documents.items.get(tab.docId);
    if (!doc) return;
    const page = target === "first" ? 1 : target === "last" ? doc.pages : target;
    const upper = doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
    const clamped = clamp(page, 1, upper);
    updateTabPage(tab.id, clamped, tab.zoom);
    requestScrollToPage(clamped, true);
  }

  function zoomBy(factor: number): void {
    const tab = getActiveTab();
    if (!tab) return;
    commandZoomToCenter(tab.zoom * factor);
  }

  /**
   * Caché del texto buscable de cada documento. Se descarta solo cuando cambian
   * los documentos (ids o títulos), no en cada pulsación: reconstruir el texto en
   * minúsculas de toda la biblioteca por tecla es O(n) con miles de documentos.
   * El progreso (página/zoom) no entra en la huella, así que cambiar de página no
   * la invalida.
   */
  let docIndex: { fingerprint: string; entries: { doc: DocumentRecord; haystack: string }[] } | null =
    null;

  function documentIndex(): { doc: DocumentRecord; haystack: string }[] {
    const docs = [...documents.items.values()];
    let fingerprint = "";
    // La huella incluye la ruta porque es parte de lo que se busca:
    // renombrar una **carpeta** la cambia sin tocar el id ni el título del
    // documento, y el índice servía rutas viejas hasta que cambiara un título.
    for (const doc of docs) fingerprint += `${doc.id}\u0000${doc.title}\u0000${doc.path}\u0001`;
    if (docIndex?.fingerprint === fingerprint) return docIndex.entries;
    const entries = docs.map((doc) => ({
      doc,
      haystack: `${doc.title} ${doc.path}`.toLowerCase(),
    }));
    docIndex = { fingerprint, entries };
    return entries;
  }

  function buildItems(q: string): Item[] {
    const ql = q.trim().toLowerCase();
    const out: Item[] = [];

    // Acciones
    const actions: Item[] = [
      { kind: "action", label: "Ir a la siguiente página", hint: "Navegación", command: SHORTCUTS.nextPage, action: () => {
        const tab = getActiveTab();
        if (tab) {
          const doc = documents.items.get(tab.docId);
          const next = tab.page + effectivePagesPerRow(doc);
          updateTabPage(tab.id, next, tab.zoom);
          requestScrollToPage(next, true);
        }
      }},
      { kind: "action", label: "Ir a la página anterior", hint: "Navegación", command: SHORTCUTS.prevPage, action: () => {
        const tab = getActiveTab();
        if (tab) {
          const doc = documents.items.get(tab.docId);
          const previous = tab.page - effectivePagesPerRow(doc);
          const upper = doc && doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
          const clamped = clamp(previous, 1, upper);
          updateTabPage(tab.id, clamped, tab.zoom);
          requestScrollToPage(clamped, true);
        }
      }},
      { kind: "action", label: "Desplazar arriba", hint: "Navegación", command: SHORTCUTS.scrollUp, action: () => scrollViewer(-1, true) },
      { kind: "action", label: "Desplazar abajo", hint: "Navegación", command: SHORTCUTS.scrollDown, action: () => scrollViewer(1, true) },
      { kind: "action", label: "Desplazar una pantalla arriba", hint: "Navegación", command: SHORTCUTS.scrollUpPage, action: () => scrollViewer(-1, false) },
      { kind: "action", label: "Desplazar una pantalla abajo", hint: "Navegación", command: SHORTCUTS.scrollDownPage, action: () => scrollViewer(1, false) },
      { kind: "action", label: "Ir a la primera página", hint: "Navegación", command: SHORTCUTS.firstPage, action: () => goToPage("first") },
      { kind: "action", label: "Ir a la última página", hint: "Navegación", command: SHORTCUTS.lastPage, action: () => goToPage("last") },
      { kind: "action", label: "Aumentar zoom", hint: "Zoom", command: SHORTCUTS.zoomIn, action: () => zoomBy(1.15) },
      { kind: "action", label: "Reducir zoom", hint: "Zoom", command: SHORTCUTS.zoomOut, action: () => zoomBy(1 / 1.15) },
      { kind: "action", label: "Restaurar última pestaña cerrada", hint: "Sesión", command: SHORTCUTS.restoreTab, action: () => { restoreLastClosedTab(); } },
      { kind: "action", label: "Abrir configuración", hint: "Ajustes", action: () => { ui.settingsOpen = true; } },
      { kind: "action", label: "Alternar barra de documentos", hint: "Apariencia", command: SHORTCUTS.toggleSidebar, action: () => { ui.showSidebar = !ui.showSidebar; } },
      { kind: "action", label: "Alternar modo lectura", hint: "Apariencia", command: SHORTCUTS.readingMode, action: () => { ui.reading = !ui.reading; if (ui.reading) ui.inspector = null; } },
      { kind: "action", label: "Mostrar información", hint: "Panel", command: SHORTCUTS.toggleInspector, action: () => { ui.inspector = "info"; } },
      { kind: "action", label: "Mostrar índice", hint: "Panel", action: () => { ui.inspector = "outline"; } },
      { kind: "action", label: "Mostrar miniaturas", hint: "Panel", action: () => { ui.inspector = "thumbs"; } },
      { kind: "action", label: "Vista: 1 página por fila", hint: "Layout", action: () => setPagesPerRow(1) },
      { kind: "action", label: "Vista: 2 páginas por fila", hint: "Layout", action: () => setPagesPerRow(2) },
      { kind: "action", label: "Vista: 3 páginas por fila", hint: "Layout", action: () => setPagesPerRow(3) },
      { kind: "action", label: "Vista: 4 páginas por fila", hint: "Layout", action: () => setPagesPerRow(4) },
      { kind: "action", label: "Activar/desactivar scroll continuo", hint: "Layout", action: toggleContinuousScroll },
    ];

    for (const a of actions) {
      if (!ql) out.push(a);
      else if (a.label.toLowerCase().includes(ql) || a.hint.toLowerCase().includes(ql)) out.push(a);
    }

    // Documentos
    const docItems: Item[] = [];
    for (const { doc, haystack } of documentIndex()) {
      const title = displayTitle(doc.title, settings.showExtensions);
      if (ql && !haystack.includes(ql)) continue;
      const tab = getTabByDocId(doc.id);
      docItems.push({
        kind: "doc",
        label: tab ? `Cambiar a ${title}` : `Abrir ${title}`,
        hint: pathSummary(doc.path, settings.pathDepth),
        action: () => {
          if (tab) setActiveTab(tab.id);
          else openDocument(doc.id);
        },
      });
    }

    // Si la query es un número, ofrecer "ir a página N"
    if (/^\d+$/.test(ql)) {
      const n = Number(ql);
      out.unshift({
        kind: "action",
        label: `Ir a página ${n}`,
        hint: "Documento actual",
        action: () => requestScrollToPage(n, true),
      });
    }

    return [...out, ...docItems].slice(0, 20);
  }

  let items = $state<Item[]>([]);

  $effect(() => {
    // `void query` fuerza el tracking: sin él, el efecto no se re-ejecutaba al
    // cambiar la búsqueda (la lectura dentro del timeout no se rastrea).
    void query;
    clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(() => {
      items = buildItems(query);
      index = 0;
    }, 60);
    return () => clearTimeout(debounceTimer);
  });

  function close() {
    ui.searchOpen = false;
    query = "";
  }

  function execute(i: number) {
    const item = items[i];
    if (!item) return;
    close();
    item.action();
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      index = clamp(index + 1, 0, items.length - 1);
      scrollToSelected();
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      index = clamp(index - 1, 0, items.length - 1);
      scrollToSelected();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (!query.trim()) {
        close();
        return;
      }
      execute(index);
    }
  }

  function scrollToSelected() {
    requestAnimationFrame(() => {
      const el = popupEl?.querySelector<HTMLElement>(".search-item.selected");
      el?.scrollIntoView({ block: "nearest" });
    });
  }
</script>

<div
  class="search-backdrop"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) close();
  }}
>
<div
  class="search-popup show"
  bind:this={popupEl}
  role="dialog"
  aria-modal="true"
  aria-label="Buscar"
  tabindex="-1"
  use:focusTrap
>
  <div class="search-popup-bar">
    <span class="search-popup-icon"><Icon name="search" size={15} /></span>
    <input
      class="search-input"
      type="text"
      placeholder="Buscar, ir a página o comando"
      autocomplete="off"
      spellcheck="false"
      aria-label="Buscar"
      bind:this={inputEl}
      bind:value={query}
      onkeydown={handleKey}
      use:focusOnMount
    />
    <span class="search-kbd">Esc</span>
  </div>

  <div
    class="search-list custom-scroll"
    use:scrollbar={{ always: settings.scrollbarMode === "always" }}
  >
    {#if items.length === 0}
      <div class="search-empty">Sin resultados para esta búsqueda.</div>
    {:else}
      {#each items as item, i (i)}
        <button
          class="search-item"
          class:selected={i === index}
          type="button"
          role="option"
          aria-selected={i === index}
          onclick={() => execute(i)}
          onmouseenter={() => (index = i)}
        >
          <span class="search-label">{item.label}</span>
          <span class="search-right">
            {#if item.command}<kbd class="search-command">{item.command}</kbd>{/if}
            <span class="search-hint">{item.hint}</span>
          </span>
        </button>
      {/each}
    {/if}
  </div>
</div>
</div>

<style>
  .search-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
  }

  .search-popup {
    position: absolute;
    bottom: calc(var(--gap-edge) + 54px);
    left: calc(
      var(--content-left, 0px) +
      (100% - var(--content-left, 0px) - var(--content-right, 0px)) / 2
    );
    transform: translateX(-50%) translateY(8px) scale(0.99);
    z-index: 90;
    border: 1px solid var(--border);
    background: var(--custom-bar-frosted, var(--bg-menu));
    box-shadow: var(--shadow-lg);
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--animation-normal), transform var(--animation-normal);
    backdrop-filter: blur(18px);
    width: min(680px, calc(100% - 32px));
    border-radius: var(--radius-bar);
    overflow: hidden;
  }
  .search-popup.show {
    opacity: 1;
    pointer-events: auto;
    transform: translateX(-50%) translateY(0) scale(1);
  }
  :global(body.opaque-surfaces) .search-popup {
    background: var(--custom-bar-opaque, var(--opaque-panel));
    backdrop-filter: none;
  }

  .search-popup-bar {
    position: relative;
    padding: 10px;
    border-bottom: 1px solid var(--border);
  }
  .search-popup-icon {
    position: absolute;
    left: 22px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-secondary);
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    height: 38px;
    border-radius: var(--radius-card);
    border: 1px solid var(--border);
    background: var(--bg-input);
    padding: 0 58px 0 38px;
    outline: none;
    color: var(--text-primary);
  }
  .search-input:focus { border-color: color-mix(in srgb, var(--accent) 42%, var(--border)); }

  .search-kbd {
    position: absolute;
    right: 20px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 11px;
    color: var(--text-secondary);
    border: 1px solid var(--border);
    background: var(--bg-button);
    border-radius: 7px;
    padding: 3px 6px;
    pointer-events: none;
  }

  .search-list {
    max-height: min(320px, 42vh);
    padding: 8px;
    overflow: auto;
  }

  .search-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    width: 100%;
    padding: 10px 11px;
    border: none;
    border-radius: var(--radius-card);
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
  }
  .search-item.selected, .search-item:hover { background: var(--bg-active); }

  .search-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .search-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
  }

  .search-command {
    font-family: inherit;
    font-size: 10px;
    font-weight: 650;
    padding: 3px 6px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg-button);
    color: var(--text-secondary);
    letter-spacing: 0.04em;
    line-height: 1;
    white-space: nowrap;
  }

  .search-hint {
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
    max-width: 170px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .search-empty {
    padding: 16px 13px;
    color: var(--text-secondary);
    font-size: 13px;
  }
</style>
