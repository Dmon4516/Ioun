<script lang="ts">
  import { onDestroy, setContext, untrack } from "svelte";
  import { ZoomController } from "$lib/pdf/zoomController.svelte";
  import { PageVisibilityRegistry } from "$lib/pdf/pageVisibility.svelte";
  import ContinuousGrid from "./ContinuousGrid.svelte";
  import SpreadView from "./SpreadView.svelte";
  import ZoomOverlay from "./ZoomOverlay.svelte";
  import {
    viewer,
    computeDims,
    setVisiblePages,
    viewerCommands,
  } from "$lib/stores/viewer.svelte";
  import type { ScrollAlign } from "$lib/stores/viewer.svelte";
  import { getDocumentAspect, hasDocumentAspect } from "$lib/pdf/documentAspect.svelte";
  import { DEFAULT_ASPECT_RATIO } from "$lib/constants";
  import { getActiveTab, updateTabPage } from "$lib/stores/tabs.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { documents } from "$lib/stores/documents.svelte";
  import { openDocument as loadPdfDocument } from "$lib/pdf/loader";
  import { getPageCached } from "$lib/pdf/renderPage";
  import {
    effectiveContinuousScroll,
    effectivePagesPerRow,
  } from "$lib/utils/documentView";
  import { scrollbar } from "$lib/actions";
  import { clamp } from "$lib/utils/colors";

  let areaEl: HTMLDivElement | null = $state(null);
  let scrollLocked = false;
  let scrollLockTimer = 0;
  let scrollRaf = 0;
  let initialScrollDoneFor = "";
  let centerAnchorTimer = 0;
  let lastZoomCommand = 0;
  let lastFitCommand = 0;
  let pageAspectRatio = $state(DEFAULT_ASPECT_RATIO);
  let aspectDocumentId = "";

  const activeTab = $derived(getActiveTab());
  const doc = $derived(activeTab ? documents.items.get(activeTab.docId) ?? null : null);
  const zoom = $derived(activeTab?.zoom ?? 100);
  const pagesPerRow = $derived(effectivePagesPerRow(doc));
  const continuousLayout = $derived(effectiveContinuousScroll(doc));
  const singleColumnContinuous = $derived(continuousLayout && pagesPerRow === 1);
  const zoomCtl = new ZoomController({ area: () => areaEl });

  const pageVisibility = new PageVisibilityRegistry();
  setContext<PageVisibilityRegistry>("pageVisibility", pageVisibility);

  $effect(() => {
    const area = areaEl;
    if (!area) return;
    pageVisibility.attach(area);
    return () => pageVisibility.detach();
  });

  $effect(() => {
    let cancelled = false;
    const currentDoc = doc;
    const pageNumber = settings.consistentPageSize ? 1 : (activeTab?.page ?? 1);
    if (!currentDoc) {
      pageAspectRatio = DEFAULT_ASPECT_RATIO;
      aspectDocumentId = "";
      return;
    }
    // Proporción medida al abrir el documento: se aplica al instante, sin
    // esperar a cargar una página, y se refresca sola cuando llega la medida.
    const preloaded = getDocumentAspect(currentDoc.id);
    if (aspectDocumentId !== currentDoc.id) {
      aspectDocumentId = currentDoc.id;
      pageAspectRatio = preloaded;
    } else if (pageNumber === 1) {
      pageAspectRatio = preloaded;
    }
    // La página 1 ya viene medida por el cargador: no hay nada que abrir.
    if (pageNumber === 1 && hasDocumentAspect(currentDoc.id)) return;
    void (async () => {
      try {
        const pdf = await loadPdfDocument(currentDoc.id, currentDoc.path);
        if (cancelled) return;
        // `getPageCached` y no `pdf.getPage`: el proxy queda en el caché
        // compartido (con su límite y su liberación) en vez de crear uno nuevo
        // por cada cambio de página que nadie vuelve a soltar.
        const page = await getPageCached(pdf, pageNumber, currentDoc.id);
        if (cancelled) return;
        const viewport = page.getViewport({ scale: 1 });
        pageAspectRatio = viewport.width / viewport.height;
      } catch (error) {
        if (!cancelled) console.error("[viewer] No se pudo medir el formato del documento", error);
      }
    })();
    return () => { cancelled = true; };
  });

  /**
   * Padding real del área de contenido, medido en el `ResizeObserver` que ya
   * existe. Se guarda aquí para que el manejador de scroll no tenga que llamar a
   * `getComputedStyle` en cada evento (el observador se dispara también cuando
   * cambia el padding porque observa la caja de contenido).
   */
  let areaPadding = { top: 0, bottom: 0, left: 0, right: 0 };

  // ── Medir el área de contenido ────────────────────────────────────
  $effect(() => {
    if (!areaEl) return;
    const el = areaEl;

    let previousWidth = 0;
    let previousHeight = 0;
    let firstMeasure = true;

    function measure() {
      const styles = getComputedStyle(el);
      const pl = parseFloat(styles.paddingLeft) || 0;
      const pr = parseFloat(styles.paddingRight) || 0;
      const pt = parseFloat(styles.paddingTop) || 0;
      const pb = parseFloat(styles.paddingBottom) || 0;
      areaPadding = { top: pt, bottom: pb, left: pl, right: pr };
      const contentWidth = Math.max(240, el.clientWidth - pl - pr);
      const contentHeight = Math.max(200, el.clientHeight - pt - pb);
      const sizeChanged = !firstMeasure &&
        (contentWidth !== previousWidth || contentHeight !== previousHeight);
      previousWidth = contentWidth;
      previousHeight = contentHeight;
      firstMeasure = false;

      viewer.contentWidth = contentWidth;
      viewer.contentHeight = contentHeight;
      viewer.viewportPaddingTop = pt;

      if (!sizeChanged) return;
      if (!settings.recenterOnToggle) return;
      clearTimeout(centerAnchorTimer);
      centerAnchorTimer = window.setTimeout(() => {
        untrack(() => {
          const tab = getActiveTab();
          if (
            !areaEl ||
            !tab ||
            zoomCtl.isBusy ||
            viewer.zooming ||
            scrollLocked
          ) return;
          doScrollToPage(tab.page, false, "center");
        });
      }, 120);
    }

    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  });

  // ── Empujar dims al store cuando cambia algo relevante ────────────
  $effect(() => {
    const dims = computeDims(
      zoom,
      pagesPerRow,
      settings.fitMode,
      viewer.contentWidth,
      viewer.contentHeight,
      pageAspectRatio,
    );
    if (
      dims.pageWidth !== viewer.dims.pageWidth ||
      dims.pageHeight !== viewer.dims.pageHeight ||
      dims.pagesPerRow !== viewer.dims.pagesPerRow
    ) {
      viewer.dims = dims;
    }
  });

  $effect(() => {
    const command = viewerCommands.zoomToCenter;
    if (!command || command.token === lastZoomCommand || !activeTab) return;
    lastZoomCommand = command.token;
    zoomCtl.command(command.zoom);
  });

  $effect(() => {
    const command = viewerCommands.fitPreserveCenter;
    if (!command || command.token === lastFitCommand) return;
    lastFitCommand = command.token;
    zoomCtl.preserveLayout();
  });

  $effect(() => {
    void activeTab?.id;
    zoomCtl.cancel();
    initialScrollDoneFor = "";
  });

  // ── Scroll inicial al cambiar de pestaña ──────────────────────────
  $effect(() => {
    // Deps
    void viewer.dims.pageHeight;
    void viewer.dims.pageWidth;
    if (!activeTab || !areaEl) return;
    if (viewer.contentWidth <= 0) return;
    if (initialScrollDoneFor === activeTab.id) return;

    initialScrollDoneFor = activeTab.id;
    // Doble rAF: garantiza que el ResizeObserver ya disparó y los slots
    // tienen su tamaño final antes de posicionar el scroll.
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (!areaEl || !activeTab) return;
        doScrollToPage(activeTab.page, false);
      }),
    );
  });

  // ── Scroll programático solicitado por otros componentes ──────────
  $effect(() => {
    const target = viewer.scrollTarget;
    if (!target || !areaEl) return;
    viewer.scrollTarget = null;
    doScrollToPage(target.page, target.smooth, target.align);
  });

  function doScrollToPage(page: number, smooth: boolean, align: ScrollAlign = "top") {
    if (!areaEl || !doc) return;
    if (zoomCtl.isBusy) zoomCtl.settle(true);
    const upper = doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
    const target = clamp(page, 1, upper);
    // SpreadView renders exactly one horizontal set at a time. It follows
    // `activeTab.page` directly, so applying the continuous-grid row
    // calculation here would move the empty scroll container to the first row
    // and make a page change appear to jump backwards.
    if (!continuousLayout) {
      scrollLocked = false;
      return;
    }
    const pageSlot = areaEl.querySelector<HTMLElement>(`[data-page="${target}"]`);
    const areaRect = areaEl.getBoundingClientRect();
    const { top: paddingTop, bottom: paddingBottom, left: paddingLeft, right: paddingRight } =
      areaPadding;
    const contentWidth = Math.max(0, areaEl.clientWidth - paddingLeft - paddingRight);
    const contentHeight = Math.max(0, areaEl.clientHeight - paddingTop - paddingBottom);
    const slotRect = pageSlot?.getBoundingClientRect() ?? null;
    // Con `content-visibility: auto` el slot puede no tener caja todavía:
    // en ese caso calculamos la posición a partir de la rejilla.
    const usableRect =
      slotRect && slotRect.height > 0 && slotRect.width > 0 ? slotRect : null;
    const oversizedPage = usableRect
      ? usableRect.height > contentHeight
      : viewer.dims.pageHeight > contentHeight;
    const topBoundary = align === "top" && oversizedPage ? 0 : paddingTop;
    const centeredTop =
      align === "center" && usableRect ? (contentHeight - usableRect.height) / 2 : 0;
    const targetTop = usableRect
      ? areaEl.scrollTop + usableRect.top - areaRect.top - topBoundary - centeredTop
      : Math.floor((target - 1) / viewer.dims.pagesPerRow) *
        (viewer.dims.pageHeight + viewer.dims.gap);
    const targetLeft = usableRect
      ? areaEl.scrollLeft + usableRect.left - areaRect.left - paddingLeft -
        (contentWidth - usableRect.width) / 2
      : areaEl.scrollLeft;

    const settleTargetPosition = () => {
      if (!areaEl) return;
      const currentSlot = areaEl.querySelector<HTMLElement>(`[data-page="${target}"]`);
      if (!currentSlot) return;
      const currentAreaRect = areaEl.getBoundingClientRect();
      const currentSlotRect = currentSlot.getBoundingClientRect();
      if (currentSlotRect.height <= 0 || currentSlotRect.width <= 0) return;
      const currentContentHeight = Math.max(
        0,
        areaEl.clientHeight - areaPadding.top - areaPadding.bottom,
      );
      const currentContentWidth = Math.max(
        0,
        areaEl.clientWidth - areaPadding.left - areaPadding.right,
      );
      const currentOversizedPage = currentSlotRect.height > currentContentHeight;
      const currentTopBoundary =
        align === "top" && currentOversizedPage ? 0 : areaPadding.top;
      const currentCenteredTop =
        align === "center"
          ? (currentContentHeight - currentSlotRect.height) / 2
          : 0;
      const correctedTop =
        areaEl.scrollTop +
        currentSlotRect.top -
        currentAreaRect.top -
        currentTopBoundary -
        currentCenteredTop;
      const correctedLeft =
        areaEl.scrollLeft +
        currentSlotRect.left -
        currentAreaRect.left -
        areaPadding.left -
        (currentContentWidth - currentSlotRect.width) / 2;
      areaEl.scrollTo({
        top: Math.max(0, correctedTop),
        left: Math.max(0, correctedLeft),
        behavior: "instant",
      });
    };

    scrollLocked = true;
    clearTimeout(scrollLockTimer);

    const settleAfterMount = () => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (areaEl && activeTab && !scrollLocked) {
            // La posición inicial puede quedar desfasada por la virtualización
            // o por el desplazamiento suave. Se corrige al terminar usando la
            // caja real del destino, sin quitar la animación.
            settleTargetPosition();
          }
        });
      });
    };

    if (smooth) {
      areaEl.scrollTo({ top: targetTop, left: targetLeft, behavior: "smooth" });
      const unlock = () => {
        scrollLocked = false;
        clearTimeout(scrollLockTimer);
        // La ventana de páginas montadas depende de las páginas visibles: tras
        // un salto programático hay que recalcularlas o el destino queda hueco.
        updateCurrentPageFromScroll();
        settleAfterMount();
      };
      if ("onscrollend" in areaEl) {
        areaEl.addEventListener("scrollend", unlock, { once: true });
      }
      scrollLockTimer = window.setTimeout(unlock, 800);
    } else {
      areaEl.scrollTo({ top: targetTop, left: targetLeft, behavior: "instant" });
      requestAnimationFrame(() => {
        scrollLocked = false;
        updateCurrentPageFromScroll();
        settleAfterMount();
      });
    }
  }

  // ── Detección de página actual por scroll ─────────────────────────
  function handleScroll() {
    if (scrollRaf) return;
    scrollRaf = requestAnimationFrame(() => {
      scrollRaf = 0;
      if (scrollLocked || zoomCtl.isSettling) return;
      updateCurrentPageFromScroll();
    });
  }

  function updateCurrentPageFromScroll() {
    if (areaEl) {
      // Extremos del scroll: deshabilitan los botones arriba/abajo.
      viewer.scrollAtTop = areaEl.scrollTop <= 1;
      viewer.scrollAtBottom =
        areaEl.scrollTop + areaEl.clientHeight >= areaEl.scrollHeight - 1;
    }
    if (
      zoomCtl.isBusy ||
      zoomCtl.isSettling ||
      viewer.zooming ||
      scrollLocked ||
      !areaEl ||
      !activeTab ||
      !doc ||
      !continuousLayout
    ) return;
    const dims = viewer.dims;
    const paddingTop = areaPadding.top;
    const paddingBottom = areaPadding.bottom;
    const rowHeight = dims.pageHeight + dims.gap;
    const totalRows = Math.ceil(doc.pages / dims.pagesPerRow);
    const usableHeight = areaEl.clientHeight - paddingTop - paddingBottom;
    // El centro del viewport en coordenadas de scroll.
    const gridY = areaEl.scrollTop + areaEl.clientHeight / 2;
    const centerRow = clamp(
      Math.round((gridY - paddingTop - dims.pageHeight / 2) / rowHeight),
      0,
      Math.max(0, totalRows - 1),
    );

    if (dims.pagesPerRow === 1) {
      const upper = doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
      const current = clamp(centerRow + 1, 1, upper);
      if (current !== activeTab.page) {
        updateTabPage(activeTab.id, current, activeTab.zoom);
      }
      updateVisiblePagesFromRows(
        Math.max(0, Math.floor((areaEl.scrollTop - paddingTop) / rowHeight)),
        Math.min(
          totalRows - 1,
          Math.floor((areaEl.scrollTop - paddingTop + areaEl.clientHeight) / rowHeight),
        ),
      );
      return;
    }

    const rowFrom = Math.max(0, centerRow - 1);
    const rowTo = Math.min(totalRows - 1, centerRow + 1);
    const areaRect = areaEl.getBoundingClientRect();
    const viewportCenterX = areaRect.left + areaEl.clientWidth / 2;
    const viewportCenterY = areaRect.top + paddingTop + usableHeight / 2;
    let nearestPage = activeTab.page;
    let nearestDistance = Number.POSITIVE_INFINITY;

    for (let row = rowFrom; row <= rowTo; row++) {
      const base = row * dims.pagesPerRow + 1;
      for (let col = 0; col < dims.pagesPerRow; col++) {
        const pageNumber = base + col;
        if (pageNumber > doc.pages) break;
        const slot = areaEl.querySelector<HTMLElement>(`[data-page="${pageNumber}"]`);
        if (!slot) continue;
        const rect = slot.getBoundingClientRect();
        const dx = rect.left + rect.width / 2 - viewportCenterX;
        const dy = rect.top + rect.height / 2 - viewportCenterY;
        const distance = dx * dx + dy * dy;
        if (distance < nearestDistance) {
          nearestDistance = distance;
          nearestPage = pageNumber;
        }
      }
    }

    const upper = doc.pages > 0 ? doc.pages : Number.POSITIVE_INFINITY;
    const current = clamp(nearestPage, 1, upper);
    if (current !== activeTab.page) {
      updateTabPage(activeTab.id, current, activeTab.zoom);
    }
    updateVisiblePagesFromRows(rowFrom, rowTo);
  }

  function updateVisiblePagesFromRows(firstRow: number, lastRow: number): void {
    if (!doc) return;
    const visible = new Set<number>();
    const perRow = viewer.dims.pagesPerRow;
    for (let row = firstRow; row <= lastRow; row++) {
      for (let col = 0; col < perRow; col++) {
        const page = row * perRow + col + 1;
        if (page > doc.pages) break;
        visible.add(page);
      }
    }
    setVisiblePages(doc.id, visible);
  }

  // ── Zoom con Ctrl + rueda ─────────────────────────────────────────
  function handleWheel(e: WheelEvent) {
    if (zoomCtl.handleWheel(e)) e.preventDefault();
  }

  onDestroy(() => {
    zoomCtl.destroy();
    clearTimeout(scrollLockTimer);
    clearTimeout(centerAnchorTimer);
    if (scrollRaf) cancelAnimationFrame(scrollRaf);
  });
</script>

<section class="viewer-wrap">
  <div
    class="viewer-area custom-scroll"
    class:continuous={continuousLayout}
    class:single-continuous={singleColumnContinuous}
    class:wheel-zooming={viewer.zooming}
    id="viewerArea"
    bind:this={areaEl}
    use:scrollbar={{ always: settings.scrollbarMode === "always" }}
    onscroll={handleScroll}
    onwheel={handleWheel}
    tabindex="-1"
    aria-label="Área del documento"
  >
    {#if doc && activeTab}
      {#if continuousLayout}
        <ContinuousGrid {doc} />
      {:else}
        <SpreadView {doc} />
      {/if}
    {:else}
      <div class="empty-state">
        <h2>Ninguna pestaña activa</h2>
        <p>
          Abre un documento desde la barra de documentos o usa la búsqueda.
          Las pestañas hibernadas se restauran al seleccionarlas.
        </p>
      </div>
    {/if}
  </div>

  <ZoomOverlay />
</section>

<style>
  .viewer-wrap {
    flex: 1;
    min-width: 0;
    position: relative;
    display: flex;
    flex-direction: column;
    background: var(--bg-content);
  }

  .viewer-area {
    flex: 1;
    overflow: auto;
    padding: calc(var(--titlebar-offset) + 10px) 28px 98px;
    padding-left: calc(var(--content-left, 0px) + 28px);
    padding-right: calc(var(--content-right, 0px) + 28px);
    outline: none;
    overscroll-behavior: contain;
    scroll-behavior: auto !important;
    transition:
      padding-left var(--animation-normal),
      padding-right var(--animation-normal);
  }

  .viewer-area.wheel-zooming :global(.page-slot),
  .viewer-area.wheel-zooming :global(canvas) {
    transition: none !important;
    scroll-behavior: auto !important;
  }

  .viewer-area.wheel-zooming :global(.page-canvas) {
    image-rendering: pixelated;
  }

  .empty-state {
    width: min(620px, 100%);
    margin: 6vh auto 0;
    border: 1px solid var(--border);
    background: var(--bg-card);
    border-radius: 26px;
    padding: 30px;
    box-shadow: var(--shadow);
  }

  .empty-state h2 {
    margin: 0 0 8px;
    font-size: 24px;
    letter-spacing: -0.03em;
  }

  .empty-state p {
    margin: 0;
    color: var(--text-secondary);
    line-height: 1.6;
  }
</style>
