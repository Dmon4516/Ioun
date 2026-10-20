<script lang="ts">
  // Prototipo aislado de capa de texto. Se abre con `?proto=text` y además una
  // de estas dos fuentes:
  //   ?pdf=<url>          archivo servido por HTTP (sirve en un navegador;
  //                       en la app, con el PDF en `public/`)
  //   ?local=<ruta>       ruta absoluta del disco cargada con el cargador de
  //                       la app (protocolo `lyra-pdf`); solo dentro de la app
  // Y opcionalmente:
  //   &page=<n>           página a medir (por defecto 1)
  //   &scale=<n>          escala (por defecto 1.5)
  //
  // Mide con la clase que se integraría (`PageTextLayer`) y con las reglas
  // oficiales de pdf.js, para que el resultado sea comparable con el canvas.
  // Ver TECH_DEBT.md.

  import { onDestroy, onMount } from "svelte";
  import { version as pdfjsVersion } from "pdfjs-dist";
  import type { PDFDocumentProxy } from "pdfjs-dist";
  // Reglas oficiales de `.textLayer`: sin ellas los spans no se colocan igual.
  import "pdfjs-dist/web/pdf_viewer.css";
  import { closeDocument, openDocument } from "$lib/pdf/loader";
  import { PageTextLayer } from "$lib/pdf/textLayer.svelte";
  import { getProcessRss } from "$lib/tauri/commands";

  const DOC_ID = "prototype";
  const WORKER_SRC = new URL(
    "pdfjs-dist/build/pdf.worker.min.mjs",
    import.meta.url,
  ).toString();

  let containerEl: HTMLDivElement | null = $state(null);
  let canvasEl: HTMLCanvasElement | null = $state(null);
  let status = $state("Esperando PDF…");
  let metrics = $state<{
    page: number;
    scale: number;
    pdfLoadMs: number;
    canvasRenderMs: number;
    textLayerMs: number;
    spansCount: number;
    rssAppMb: number | null;
    rssTreeBeforeMb: number | null;
    rssTreeAfterMb: number | null;
  } | null>(null);

  const layer = new PageTextLayer();
  let usedAppLoader = $state(false);
  let loadedPdf: PDFDocumentProxy | null = null;

  /**
   * RSS real medido desde Rust. Se usa en lugar de `performance.memory`, que
   * solo existe en Chromium. `treeRssMb` incluye el proceso del webview, que es
   * donde pdf.js reserva la memoria que nos interesa.
   */
  async function readRss(): Promise<{ app: number; tree: number } | null> {
    try {
      const memory = await getProcessRss();
      return { app: memory.rssMb, tree: memory.treeRssMb };
    } catch {
      // Fuera de Tauri, o build de producción sin el comando.
      return null;
    }
  }

  async function loadPdf(path: string, useAppLoader: boolean): Promise<PDFDocumentProxy> {
    if (useAppLoader) {
      // Camino real de la app: mide también el protocolo `lyra-pdf`.
      usedAppLoader = true;
      return openDocument(DOC_ID, path);
    }
    // Carga directa con pdf.js: permite medir en un navegador normal.
    const pdfjs = await import("pdfjs-dist");
    pdfjs.GlobalWorkerOptions.workerSrc = WORKER_SRC;
    return pdfjs.getDocument({ url: path, isEvalSupported: false }).promise;
  }

  async function run(
    path: string,
    pageNumber: number,
    scale: number,
    useAppLoader: boolean,
  ): Promise<void> {
    status = `Cargando ${path}…`;
    const loadStart = performance.now();
    const pdf = await loadPdf(path, useAppLoader);
    loadedPdf = pdf;
    const pdfLoadMs = performance.now() - loadStart;

    const page = await pdf.getPage(pageNumber);
    const viewport = page.getViewport({ scale });
    if (!canvasEl || !containerEl) return;

    canvasEl.width = Math.ceil(viewport.width);
    canvasEl.height = Math.ceil(viewport.height);
    const ctx = canvasEl.getContext("2d");
    if (!ctx) throw new Error("no 2d context");

    const renderStart = performance.now();
    await page.render({ canvasContext: ctx, viewport }).promise;
    const canvasRenderMs = performance.now() - renderStart;

    const rssBefore = await readRss();
    const layerStart = performance.now();
    await layer.render(pdf, DOC_ID, pageNumber, scale, containerEl);
    const textLayerMs = performance.now() - layerStart;
    const rssAfter = await readRss();

    metrics = {
      page: pageNumber,
      scale,
      pdfLoadMs,
      canvasRenderMs,
      textLayerMs,
      spansCount: containerEl.querySelectorAll("span").length,
      rssAppMb: rssAfter?.app ?? null,
      rssTreeBeforeMb: rssBefore?.tree ?? null,
      rssTreeAfterMb: rssAfter?.tree ?? null,
    };
    status = "Listo";
  }

  onMount(async () => {
    const params = new URLSearchParams(window.location.search);
    const url = params.get("pdf");
    const local = params.get("local");
    if (!url && !local) {
      status = "Indica ?pdf=<url> o ?local=<ruta absoluta>.";
      return;
    }
    const pageParam = Number(params.get("page"));
    const scaleParam = Number(params.get("scale"));
    const pageNumber = pageParam > 0 ? Math.floor(pageParam) : 1;
    const scale = scaleParam > 0 ? scaleParam : 1.5;
    try {
      await run(local ?? url ?? "", pageNumber, scale, Boolean(local));
    } catch (error) {
      status = `ERROR: ${String(error)}`;
      console.error("[prototype]", error);
    }
  });

  onDestroy(() => {
    // Cancelar la capa antes de destruir el documento.
    layer.destroy();
    if (usedAppLoader) void closeDocument(DOC_ID);
    else void loadedPdf?.destroy();
    loadedPdf = null;
  });
</script>

<div class="prototype">
  <div class="toolbar">
    <strong>Prototipo TextLayer</strong>
    <span class="status">{status}</span>
  </div>

  {#if metrics}
    <table class="metrics">
      <tbody>
        <tr><td>pdfjs-dist</td><td>{pdfjsVersion}</td></tr>
        <tr><td>Página / escala</td><td>{metrics.page} / {metrics.scale}</td></tr>
        <tr><td>Carga</td><td>{usedAppLoader ? "cargador de la app (lyra-pdf)" : "pdf.js directo (fetch)"}</td></tr>
        <tr><td>PDF load</td><td>{metrics.pdfLoadMs.toFixed(1)} ms</td></tr>
        <tr><td>Canvas render</td><td>{metrics.canvasRenderMs.toFixed(1)} ms</td></tr>
        <tr><td>TextLayer build</td><td>{metrics.textLayerMs.toFixed(1)} ms</td></tr>
        <tr><td>Spans</td><td>{metrics.spansCount}</td></tr>
        <tr>
          <td>RSS app / árbol</td>
          <td>
            {metrics.rssAppMb !== null ? `${metrics.rssAppMb.toFixed(1)} MB` : "no disponible"} /
            {metrics.rssTreeAfterMb !== null ? `${metrics.rssTreeAfterMb.toFixed(1)} MB` : "no disponible"}
          </td>
        </tr>
        <tr>
          <td>RSS árbol Δ (capa)</td>
          <td>
            {metrics.rssTreeBeforeMb !== null && metrics.rssTreeAfterMb !== null
              ? `${(metrics.rssTreeAfterMb - metrics.rssTreeBeforeMb).toFixed(1)} MB`
              : "no disponible"}
          </td>
        </tr>
      </tbody>
    </table>
  {/if}

  <div class="page-wrap">
    <canvas bind:this={canvasEl}></canvas>
    <div class="textLayer" bind:this={containerEl} aria-hidden="false"></div>
  </div>

  <p class="hint">
    Selecciona texto con el ratón. Compara con lo que se ve pintado en el canvas.
    Si los caracteres seleccionados no coinciden con los visibles, la capa está
    desalineada.
  </p>
</div>

<style>
  .prototype { padding: 16px; font-family: system-ui, sans-serif; background: #0c0e12; color: #f3f5f8; min-height: 100vh; }
  .toolbar { display: flex; gap: 12px; align-items: center; margin-bottom: 12px; }
  .status { font-size: 12px; opacity: 0.7; }
  .metrics { border-collapse: collapse; margin-bottom: 12px; font-size: 12px; }
  .metrics td { border: 1px solid #262b33; padding: 4px 10px; }
  .metrics td:first-child { color: #99a1ad; }
  .page-wrap { position: relative; display: inline-block; background: white; box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4); }
  .page-wrap canvas { display: block; }
  /* Geometría y spans vienen de pdf_viewer.css; solo se añade lo del prototipo. */
  .textLayer { z-index: 2; pointer-events: auto; user-select: text; }
  .hint { margin-top: 16px; font-size: 13px; opacity: 0.8; max-width: 640px; }
</style>
