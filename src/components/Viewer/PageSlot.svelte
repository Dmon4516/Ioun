<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import {
    viewer,
    registerLoaded,
    unregisterLoaded,
    beginRender,
    endRender,
    isRenderBudgetExhausted,
  } from "$lib/stores/viewer.svelte";
  import { getContext } from "svelte";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { openDocument as loadPdfDocument, getLoadedDocument } from "$lib/pdf/loader";
  import { documents } from "$lib/stores/documents.svelte";
  import {
    cleanupPage,
    getPageCached,
    renderDpr,
    renderPageToCanvas,
    releaseCanvas,
  } from "$lib/pdf/renderPage";
  import { protectPage, unprotectPage } from "$lib/pdf/protectedPages";
  import { getDocumentAspect } from "$lib/pdf/documentAspect.svelte";
  import { markDocumentLoaded } from "$lib/pdf/documentState.svelte";
  import type { PageTextLayer } from "$lib/pdf/textLayer.svelte";
  import type { PageVisibilityRegistry } from "$lib/pdf/pageVisibility.svelte";

  let {
    page,
    docId,
    current = false,
  }: { page: number; docId: string; current?: boolean } = $props();

  let rootEl: HTMLDivElement | null = $state(null);
  let canvasA: HTMLCanvasElement | null = $state(null);
  let canvasB: HTMLCanvasElement | null = $state(null);
  let visibleCanvas = $state<"a" | "b" | null>(null);
  let hasContent = $state(false);
  // Sembrado con la proporción que midió el cargador al abrir el documento: el
  // hueco de la página ya tiene su forma antes de que llegue el primer render.
  // El slot se recrea al cambiar de documento, así que basta el valor inicial.
  //
  // El `untrack` es a propósito: si el efecto reaccionara a la proporción global
  // del documento, al llegar la medición (o la de otra página del mismo
  // documento) recalcularía y volvería a pintar el hueco. La forma real de *esta*
  // página la fija `doRender` con el viewport de pdf.js, que es más exacto que la
  // proporción del documento, así que aquí solo interesa el valor inicial.
  let pageAspectRatio = $state(untrack(() => getDocumentAspect(docId)));

  /**
   * Capa de texto (`settings.textLayerEnabled`, encendida por defecto).
   * Se construye después de pintar el canvas, sobre las medidas reales de ese
   * canvas, y se destruye al limpiar el slot.
   */
  let textLayerEl: HTMLDivElement | null = $state(null);
  let textLayer: PageTextLayer | null = null;
  let textLayerCssLoaded = false;
  /** Firma de la capa ya construida, para no reconstruirla si nada cambió. */
  let textLayerKey = "";
  let failedTextLayerKey = "";
  /**
   * La capa solo se muestra cuando coincide con el canvas actual. Se apaga en
   * cuanto cambia la geometría (zoom/dimensiones) y vuelve cuando `buildTextLayer`
   * la reconstruye: así el subrayado nunca se pinta sobre posiciones viejas, ni
   * durante el zoom ni en el lapso entre que acaba el gesto y termina el render.
   */
  let textLayerInSync = $state(false);
  let pageViewportSize = { width: 0, height: 0 };

  async function buildTextLayer(
    pdf: Awaited<ReturnType<typeof loadPdfDocument>>,
    canvas: HTMLCanvasElement,
  ): Promise<void> {
    const seq = renderSeq;
    const host = textLayerEl;
    if (!host || !settings.textLayerEnabled || pageViewportSize.width <= 0) return;
    // El canvas se ajusta por CSS (`max-width/height`), así que la escala real es
    // la de la caja visible; si aún no hay layout, se usa el tamaño del mapa de
    // bits dividido por el DPR del render.
    const dpr = renderDpr();
    const width = canvas.clientWidth || canvas.width / dpr;
    const height = canvas.clientHeight || canvas.height / dpr;
    if (width <= 0 || height <= 0) return;

    if (!textLayerCssLoaded) {
      // Solo al activarla: ni el módulo de la capa (que importa pdf.js) ni las
      // reglas oficiales de `.textLayer` forman parte del arranque.
      try {
        await Promise.all([
          import("$lib/pdf/textLayer.svelte"),
          import("pdfjs-dist/web/pdf_viewer.css"),
        ]);
      } catch (error) {
        console.error(`[PageSlot ${page}] no se pudo cargar la capa de texto`, error);
        // Do not permanently poison this slot: a later render can retry the
        // chunk after a transient dynamic-import/network failure.
        textLayerCssLoaded = false;
        failedTextLayerKey = "";
        return;
      }
      textLayerCssLoaded = true;
    }
    let PageTextLayer: typeof import("$lib/pdf/textLayer.svelte").PageTextLayer;
    try {
      ({ PageTextLayer } = await import("$lib/pdf/textLayer.svelte"));
    } catch (error) {
      console.error(`[PageSlot ${page}] no se pudo cargar la capa de texto`, error);
      failedTextLayerKey = "";
      return;
    }
    if (!settings.textLayerEnabled) return;

    const slotRect = rootEl?.getBoundingClientRect();
    const canvasRect = canvas.getBoundingClientRect();
    if (slotRect) {
      host.style.left = `${canvasRect.left - slotRect.left}px`;
      host.style.top = `${canvasRect.top - slotRect.top}px`;
    }
    host.style.width = `${width}px`;
    host.style.height = `${height}px`;

    const key = `${docId}|${page}|${width}x${height}`;
    if (failedTextLayerKey === key) return;
    if (textLayer && textLayerKey === key) {
      // Ya está construida para esta geometría exacta.
      if (seq === renderSeq) textLayerInSync = true;
      return;
    }
    if (seq !== renderSeq) return;

    textLayer?.destroy();
    const layer = new PageTextLayer();
    textLayer = layer;
    textLayerKey = key;
    try {
      await layer.render(pdf, docId, page, width / pageViewportSize.width, host);
      if (textLayer === layer) textLayerInSync = true;
    } catch (error) {
      if (textLayer === layer) {
        failedTextLayerKey = key;
        textLayer = null;
        textLayerInSync = false;
      }
      console.error(`[PageSlot ${page}] text layer error`, error);
    }
  }

  // Secuencia de render para descartar renders obsoletos
  let renderSeq = 0;
  let renderInFlight = false;
  let lastRenderedKey = "";
  let clearAfterRender = false;
  let cleanupTimer = 0;
  let renderTimer = 0;
  let renderRaf = 0;
  let activeRenderTask: { cancel: () => void } | null = null;

  const visibility = getContext<PageVisibilityRegistry | undefined>("pageVisibility");

  const activeTab = $derived(getActiveTab());
  const zoom = $derived(activeTab?.zoom ?? 100);
  const tabStatus = $derived(activeTab?.status ?? null);
  const dims = $derived(viewer.dims);
  const pageHeight = $derived(
    settings.consistentPageSize
      ? dims.pageHeight
      : Math.max(1, Math.round(dims.pageWidth / pageAspectRatio)),
  );

  const radius = $derived(
    Math.min(20, Math.max(6, Math.round(dims.pageWidth / 95))),
  );

  function renderKey(): string {
    return `${docId}|${page}|${zoom.toFixed(2)}|${dims.pageWidth}`;
  }

  async function doRender(): Promise<void> {
    if (!canvasA || !canvasB || viewer.zooming || tabStatus === "restoring") return;
    const key = renderKey();
    if (visibleCanvas && lastRenderedKey === key) {
      // Geometría sin cambios: si la capa ya estaba construida, vuelve a mostrarse
      // (un zoom de ida y vuelta exacto la había ocultado sin reconstruir).
      if (textLayer) textLayerInSync = true;
      return;
    }
    if (renderInFlight) return;
    if (isRenderBudgetExhausted(docId, settings.maxPagesLoaded, page)) return;

    clearTimeout(cleanupTimer);
    const seq = ++renderSeq;
    renderInFlight = true;
    beginRender(docId, page, settings.maxPagesLoaded);
    const previousVisible = visibleCanvas;
    const targetName = visibleCanvas === "a" ? "b" : "a";
    const targetCanvas = targetName === "a" ? canvasA : canvasB;

    try {
      const doc = documents.items.get(docId);
      if (!doc) return;
      const pdf = await loadPdfDocument(docId, doc.path);
      if (seq !== renderSeq) return;

      // El mismo proxy que va a usar `renderPageToCanvas`: pedirlo aparte
      // creaba dos proxies de la misma página por cada render.
      const pdfPage = await getPageCached(pdf, page, docId);
      const baseViewport = pdfPage.getViewport({ scale: 1 });
      pageAspectRatio = baseViewport.width / baseViewport.height;
      pageViewportSize = { width: baseViewport.width, height: baseViewport.height };
      const scale = dims.pageWidth / baseViewport.width;
      const dpr = renderDpr();

      const handle = renderPageToCanvas(pdf, page, targetCanvas, scale, dpr, docId);
      activeRenderTask = handle;
      try {
        await handle.promise;
      } finally {
        if (activeRenderTask === handle) activeRenderTask = null;
      }
      if (seq !== renderSeq) return;

      visibleCanvas = targetName;
      hasContent = true;
      lastRenderedKey = key;
      registerLoaded(docId, page);
      markDocumentLoaded(docId);
      if (settings.textLayerEnabled) void buildTextLayer(pdf, targetCanvas);
      if (previousVisible) {
        const previousCanvas = previousVisible === "a" ? canvasA : canvasB;
        cleanupTimer = window.setTimeout(() => {
          if (visibleCanvas !== previousVisible) releaseCanvas(previousCanvas);
        }, 180);
      }
    } catch (err) {
      console.error(`[PageSlot ${page}] render error`, err);
    } finally {
      endRender(docId, page, settings.maxPagesLoaded);
      if (seq === renderSeq) {
        renderInFlight = false;
        if (clearAfterRender) {
          clearAfterRender = false;
          clearContent();
        } else if (!viewer.zooming && key !== renderKey()) {
          requestAnimationFrame(() => void doRender());
        }
      }
    }
  }

  function clearContent(): void {
    if (!canvasA || !canvasB) return;
    if (renderInFlight) {
      activeRenderTask?.cancel();
      activeRenderTask = null;
      clearAfterRender = true;
      return;
    }
    clearTimeout(cleanupTimer);
    // Antes de soltar los canvas: el render de la capa en curso debe cancelarse.
    textLayer?.destroy();
    textLayer = null;
    textLayerKey = "";
    failedTextLayerKey = "";
    textLayerInSync = false;
    releaseCanvas(canvasA);
    releaseCanvas(canvasB);
    visibleCanvas = null;
    hasContent = false;
    lastRenderedKey = "";
    unregisterLoaded(docId, page);
    unprotectPage(docId, page);
    // Liberar la página del caché de pdf.js. Se consulta el documento ya
    // cargado: reabrirlo aquí cargaría un PDF que puede estar hibernado,
    // justo lo contrario de lo que busca la hibernación.
    void getLoadedDocument(docId).then((pdf) => {
      if (!pdf) return;
      return cleanupPage(pdf, page, docId);
    });
  }

  // ── Visibilidad compartida: renderiza cuando entra, libera cuando sale ─
  $effect(() => {
    if (!rootEl || !visibility) return;
    const rootElRef = rootEl;
    return visibility.observe(rootElRef, (isIntersecting, entry) => {
      if (isIntersecting) {
        // Visible: el LRU no debe expulsar esta página.
        protectPage(docId, page);
        void doRender();
        return;
      }
      if (viewer.zooming) return;
      const scrollRoot = document.getElementById("viewerArea");
      const viewportHeight = scrollRoot?.clientHeight ?? 0;
      if (entry.boundingClientRect.bottom < 0) {
        // Slot completamente por encima del viewport → liberar
        unprotectPage(docId, page);
        clearContent();
        return;
      }
      if (entry.boundingClientRect.top > viewportHeight + 2000) {
        // Slot muy por debajo → liberar
        unprotectPage(docId, page);
        clearContent();
      }
    });
  });

  // ── La capa de texto sigue al ajuste, también cuando cambia en caliente ──
  //
  // Al apagarla, el `{#if}` desmonta el contenedor, pero la instancia que pdf.js
  // llenó de `span` sigue viva apuntando a un nodo desprendido: sin este efecto
  // solo se liberaba si la página se volvía a pintar, así que apagar el ajuste en
  // un documento largo dejaba una capa colgando por cada página ya pintada.
  //
  // La única dependencia es el ajuste: todo lo demás se lee dentro de `untrack`
  // porque el efecto escribe ese mismo estado y se invalidaría a sí mismo.
  $effect(() => {
    const enabled = settings.textLayerEnabled;
    untrack(() => {
      if (!enabled) {
        if (textLayer) {
          textLayer.destroy();
          textLayer = null;
          textLayerKey = "";
          textLayerInSync = false;
        }
        return;
      }
      // Se acaba de encender: reconstruir sobre la página ya pintada para que el
      // texto sea seleccionable sin tener que hacer zoom.
      if (!hasContent || textLayerInSync || renderInFlight) return;
      const canvas = visibleCanvas === "a"
        ? canvasA
        : visibleCanvas === "b"
          ? canvasB
          : null;
      const doc = documents.items.get(docId);
      if (!canvas || !doc) return;
      void loadPdfDocument(docId, doc.path).then((pdf) => {
        if (canvas.isConnected) void buildTextLayer(pdf, canvas);
      });
    });
  });

  // ── Re-render al cambiar zoom o dimensiones, si está visible ────────
  $effect(() => {
    void zoom;
    void dims.pageWidth;
    void viewer.zooming;
    void tabStatus;
    void viewer.renderBudgetFull;
    // Primer síntoma de geometría cambiada: ocultar la capa ya, sin esperar a que
    // el render termine (o empiece: con `zooming` el render se pospone).
    if (hasContent && lastRenderedKey !== renderKey()) textLayerInSync = false;
    if (viewer.zooming) return;
    if (!rootEl) return;
    const scrollRoot = document.getElementById("viewerArea");
    if (!scrollRoot) return;

    const rect = rootEl.getBoundingClientRect();
    const scrollRect = scrollRoot.getBoundingClientRect();
    const nearVisible =
      rect.bottom > scrollRect.top - 700 &&
      rect.top < scrollRect.bottom + 700;

    if (nearVisible) {
      const centerDistance = Math.abs(
        (rect.top + rect.bottom) / 2 -
        (scrollRect.top + scrollRect.bottom) / 2,
      );
      renderTimer = window.setTimeout(() => {
        renderRaf = requestAnimationFrame(() => void doRender());
      }, 100 + Math.min(centerDistance * 0.06, 300));
    }
    return () => {
      clearTimeout(renderTimer);
      cancelAnimationFrame(renderRaf);
    };
  });

  onDestroy(() => {
    renderSeq++;
    // Sin esto, `viewer.loadedPages` crecía sin límite al recorrer un documento
    // largo (la ventana monta y desmonta slots) e inflaba la estimación de
    // memoria que usa `enforceMemoryLimit`.
    unregisterLoaded(docId, page);
    unprotectPage(docId, page);
    activeRenderTask?.cancel();
    activeRenderTask = null;
    clearTimeout(cleanupTimer);
    clearTimeout(renderTimer);
    cancelAnimationFrame(renderRaf);
    textLayer?.destroy();
    textLayer = null;
    textLayerKey = "";
    textLayerInSync = false;
    if (canvasA) releaseCanvas(canvasA);
    if (canvasB) releaseCanvas(canvasB);
  });
</script>

<div
  class="page-slot"
  class:loaded={hasContent}
  class:current={current}
  bind:this={rootEl}
  data-page={page}
  role="img"
  aria-current={current ? "page" : undefined}
  aria-label={`Página ${page}${current ? " (actual)" : ""}`}
  style:width={`${dims.pageWidth}px`}
  style:height={`${pageHeight}px`}
  style:border-radius={`${radius}px`}
>
  <span class="page-marker" aria-hidden="true"></span>
  <span class="page-current-indicator" aria-hidden="true"></span>
  <canvas class="page-canvas" class:visible={visibleCanvas === "a"} bind:this={canvasA} aria-hidden="true"></canvas>
  <canvas class="page-canvas" class:visible={visibleCanvas === "b"} bind:this={canvasB} aria-hidden="true"></canvas>
  <!-- Con el zoom en marcha la geometría de los spans es la vieja y el subrayado
       de la selección se desplaza en diagonal hasta que el render reconstruye la
       capa: se oculta mientras dura el gesto. -->
  {#if settings.textLayerEnabled}
    <div
      class="page-text-layer textLayer"
      class:visible={hasContent && textLayerInSync}
      class:selectable={settings.textLayerSelectable}
      bind:this={textLayerEl}
      aria-hidden={settings.textLayerSelectable ? undefined : "true"}
    ></div>
  {/if}
</div>

<style>
  .page-slot {
    position: relative;
    overflow: visible;
    background: #ffffff;
    /* Solo la capa de texto es seleccionable: el hueco blanco (slot y canvas) no
       debe entrar nunca en la selección, ni al arrastrar entre páginas. */
    user-select: none;
    -webkit-user-select: none;
    /* El tamaño es explícito (width/height inline), así que no se necesita
       `contain-intrinsic-size`; `content-visibility` ya implica containment. */
    content-visibility: auto;
  }

  .page-slot:not(.loaded) {
    background: color-mix(in srgb, var(--bg-card) 52%, transparent);
    border: 1px dashed color-mix(in srgb, var(--border) 82%, transparent);
    box-shadow: none;
  }

  .page-slot.loaded { background: transparent; }

  .page-canvas {
    user-select: none;
    -webkit-user-select: none;
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: block;
    max-width: 100%;
    max-height: 100%;
    width: auto;
    height: auto;
    object-fit: contain;
    background: #ffffff;
    border-radius: inherit;
    box-shadow: var(--shadow-page);
    opacity: 0;
  }

  .page-canvas.visible { opacity: 1; }

  /* La geometría de los spans la pone `pdf_viewer.css`; aquí solo se coloca el
     contenedor sobre el canvas y se deja que el texto sea seleccionable. */
  .page-text-layer {
    position: absolute;
    z-index: 2;
    /* Sin `selectable` la capa existe para buscar y para accesibilidad, pero no
       captura el ratón: el gesto sigue siendo de scroll. */
    pointer-events: none;
    user-select: none;
    opacity: 0;
    transition: opacity var(--animation-fast);
  }
  .page-text-layer.selectable {
    pointer-events: auto;
    user-select: text;
    -webkit-user-select: text;
    touch-action: auto;
  }

  .page-text-layer.selectable :global(span),
  .page-text-layer.selectable :global(br) {
    pointer-events: auto;
    user-select: text;
    -webkit-user-select: text;
  }

  .page-text-layer.selectable :global(span)::selection,
  .page-text-layer.selectable :global(br)::selection {
    background: var(--selection-bg, color-mix(in srgb, var(--accent) 34%, transparent));
    color: transparent;
  }
  .page-text-layer.visible { opacity: 1; }

  .page-slot::after {
    content: "";
    position: absolute;
    left: 50%;
    bottom: -8px;
    width: 24px;
    height: 2px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text-secondary) 25%, transparent);
    transform: translateX(-50%);
    transition: width var(--animation-fast), background var(--animation-fast);
  }

  .page-slot.current::after {
    width: 36px;
    background: var(--accent);
  }

  .page-current-indicator {
    position: absolute;
    top: 8px;
    bottom: 8px;
    left: -7px;
    width: 3px;
    border-radius: 3px;
    background: var(--accent);
    opacity: 0;
    transform: scaleY(0.72);
    transition: opacity var(--animation-fast), transform var(--animation-fast);
    pointer-events: none;
  }

  .page-slot.current .page-current-indicator {
    opacity: 1;
    transform: scaleY(1);
  }

  :global(.viewer-area.single-continuous) .page-slot::after {
    display: none;
  }

  :global(.viewer-area.single-continuous) .page-current-indicator {
    display: none;
  }

  .page-marker {
    display: none;
  }

  :global(.viewer-area.single-continuous) .page-marker {
    position: sticky;
    top: 14px;
    z-index: 5;
    display: block;
    width: 3px;
    height: min(19px, calc(100vh - 28px), max(0px, calc(100% - 24px)));
    margin-left: -10px;
    margin-bottom: max(0px, calc(24px - 100%));
    border-radius: 3px;
    background: var(--text-secondary);
    pointer-events: none;
  }

  :global(.viewer-area.single-continuous) .page-slot.current .page-marker {
    background: var(--accent);
  }

</style>
