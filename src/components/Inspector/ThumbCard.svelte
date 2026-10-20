<script lang="ts">
  import { getContext, untrack } from "svelte";
  import { openDocument } from "$lib/pdf/loader";
  import { getPageCached, renderPageToCanvas, releaseCanvas } from "$lib/pdf/renderPage";
  import type { ThumbVisibility } from "$lib/pdf/thumbVisibility.svelte";
  import { FALLBACK_PAGE_ASPECT, pageAspect, thumbSheetSize } from "$lib/utils/thumbGrid";

  let {
    docId,
    docPath,
    page,
    aspect = FALLBACK_PAGE_ASPECT,
    current = false,
    onclick,
  }: {
    docId: string;
    docPath: string;
    page: number;
    aspect?: number;
    current?: boolean;
    onclick: () => void;
  } = $props();

  /** 4px de padding de la tarjeta + 4px de marco + 3px de resalte de la hoja. */
  const CARD_INSET = 14;

  let rootEl: HTMLButtonElement | null = $state(null);
  let canvasEl: HTMLCanvasElement | null = $state(null);
  let rendered = $state(false);
  let pageRatio = $state(0);
  let sheetWidth = $state(0);
  let sheetHeight = $state(0);
  let busy = false;
  let nearViewport = false;
  let renderedWidth = 0;
  let renderGeneration = 0;
  let resizeTimer = 0;
  let activeRenderTask: { cancel: () => void } | null = null;
  const visibility = getContext<ThumbVisibility | undefined>("thumbVisibility");

  /** Proporción de la página: la medida al renderizar y, si aún no la hay, la del documento. */
  const sheetAspect = $derived(pageRatio || aspect);
  const sheetWidthCss = $derived(sheetWidth > 0 ? `${sheetWidth}px` : "100%");
  const sheetHeightCss = $derived(sheetHeight > 0 ? `${sheetHeight}px` : "auto");

  function measureSheet(): void {
    if (!rootEl) return;
    const size = thumbSheetSize(
      rootEl.clientWidth - CARD_INSET,
      rootEl.clientHeight - CARD_INSET,
      sheetAspect,
    );
    sheetWidth = size.width;
    sheetHeight = size.height;
  }

  $effect(() => {
    void sheetAspect;
    measureSheet();
  });

  async function doRender() {
    if (!canvasEl || !rootEl || rendered || busy || !nearViewport) return;
    const targetWidth = Math.floor(rootEl.clientWidth);
    if (targetWidth < 1) return;
    const generation = renderGeneration;
    busy = true;
    try {
      const pdf = await openDocument(docId, docPath);
      if (generation !== renderGeneration) return;
      const pdfPage = await getPageCached(pdf, page, docId);
      if (generation !== renderGeneration) return;
      const baseViewport = pdfPage.getViewport({ scale: 1 });
      pageRatio = pageAspect(baseViewport.width, baseViewport.height);
      const scale = targetWidth / baseViewport.width;
      // DPR adaptativo: no forzar 2× en pantallas de 1×.
      const dpr = Math.min(Math.max(window.devicePixelRatio || 1, 1), 2);
      const handle = renderPageToCanvas(pdf, page, canvasEl, scale, dpr, docId);
      activeRenderTask = handle;
      try {
        await handle.promise;
      } finally {
        if (activeRenderTask === handle) activeRenderTask = null;
      }
      if (generation !== renderGeneration) {
        releaseCanvas(canvasEl);
        return;
      }
      rendered = true;
      renderedWidth = targetWidth;
    } catch (err) {
      console.error(`[thumb ${page}]`, err);
    } finally {
      busy = false;
      if (generation !== renderGeneration) {
        if (nearViewport) void doRender();
      } else if (
        nearViewport &&
        rendered &&
        rootEl &&
        canvasEl &&
        Math.abs(rootEl.clientWidth - renderedWidth) > 1
      ) {
        releaseCanvas(canvasEl);
        rendered = false;
        void doRender();
      }
    }
  }

  /**
   * Clave de lo que pinta la miniatura. El store sustituye el objeto del
   * documento cada vez que guarda progreso (cada cambio de página), así que el
   * panel se re-renderiza: comparar la clave evita que eso vuelva a renderizar
   * las miniaturas ya cargadas.
   */
  const renderKey = $derived(`${docId}\u0000${docPath}\u0000${page}`);
  let renderedKey = "";

  $effect(() => {
    if (renderKey === renderedKey) return;
    renderedKey = renderKey;
    // `untrack`: `doRender` lee `rendered`, que aquí también se escribe; sin
    // aislarlo el efecto se invalidaría a sí mismo en bucle.
    untrack(() => {
      renderGeneration++;
      activeRenderTask?.cancel();
      activeRenderTask = null;
      rendered = false;
      renderedWidth = 0;
      pageRatio = 0;
      if (canvasEl) releaseCanvas(canvasEl);
      if (nearViewport) void doRender();
    });
  });

  $effect(() => {
    if (!rootEl || !visibility) return;
    const unobserve = visibility.observe(rootEl, (isVisible) => {
      nearViewport = isVisible;
      if (isVisible) untrack(doRender);
    });
    const unsubscribeLayout = visibility.onLayoutChange(() => {
      measureSheet();
      if (!rootEl || !canvasEl || Math.abs(rootEl.clientWidth - renderedWidth) <= 1) return;
      clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        if (
          !rootEl ||
          !canvasEl ||
          busy ||
          !rendered ||
          Math.abs(rootEl.clientWidth - renderedWidth) <= 1
        ) return;
        rendered = false;
        releaseCanvas(canvasEl);
        if (nearViewport) void doRender();
      }, 100);
    });
    return () => {
      unobserve();
      unsubscribeLayout();
      clearTimeout(resizeTimer);
    };
  });

  $effect(() => () => {
    renderGeneration++;
    activeRenderTask?.cancel();
    activeRenderTask = null;
    clearTimeout(resizeTimer);
    if (canvasEl) releaseCanvas(canvasEl);
  });
</script>

<button
  class="thumb-card"
  class:current
  type="button"
  aria-current={current ? "page" : undefined}
  onclick={onclick}
  bind:this={rootEl}
>
  <span class="thumb-page">
    <span class="thumb-sheet" style:--sheet-w={sheetWidthCss} style:--sheet-h={sheetHeightCss}>
      <canvas bind:this={canvasEl}></canvas>
      {#if !rendered}
        <span class="thumb-skel"></span>
      {/if}
      <span class="thumb-badge" class:current class:single={String(page).length === 1}>{page}</span>
    </span>
  </span>
</button>

<style>
  .thumb-card {
    position: relative;
    box-sizing: border-box;
    width: 100%;
    min-width: 60px;
    min-height: 80px;
    border: none;
    border-radius: var(--radius-card);
    background: transparent;
    padding: 4px;
    cursor: pointer;
    transition: transform 140ms ease-out;
    height: 100%;
  }
  .thumb-card:hover { transform: translateY(-2px); }
  .thumb-card:active { transform: translateY(0); }
  .thumb-card:focus-visible { outline: none; }

  .thumb-page {
    position: absolute;
    inset: 4px;
    display: grid;
    place-items: center;
  }

  /* La hoja manda: el resalte y el anillo de foco siguen su forma, así que se
     ajustan solos a los distintos formatos de página. */
  .thumb-sheet {
    position: relative;
    box-sizing: content-box;
    width: var(--sheet-w, 100%);
    height: var(--sheet-h, auto);
    padding: 3px;
    border-radius: 12px;
    transition: background 140ms ease-out;
  }

  .thumb-card:hover .thumb-sheet {
    background: var(--bg-active);
  }

  .thumb-card:focus-visible .thumb-sheet {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .thumb-sheet canvas {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
    border-radius: 8px;
    background: #fff;
    box-shadow: var(--shadow-page);
  }

  .thumb-skel {
    position: absolute;
    inset: 3px;
    border-radius: 8px;
    background: linear-gradient(90deg, var(--bg-button), var(--bg-active), var(--bg-button));
    background-size: 220% 100%;
    animation: pulse 1.2s infinite linear;
  }

  @keyframes pulse {
    0% { background-position: 100% 0; }
    100% { background-position: -100% 0; }
  }

  .thumb-badge {
    position: absolute;
    left: 50%;
    bottom: 9px;
    z-index: 2;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 750;
    border-radius: 999px;
    background: var(--bg-menu);
    border: 1px solid var(--border);
    color: var(--text-primary);
    box-shadow: var(--shadow);
    transform: translateX(-50%);
  }
  .thumb-badge.single { width: 18px; padding: 0; }
  .thumb-badge.current {
    background: var(--state-active);
    border-color: transparent;
    color: #081018;
  }
</style>
