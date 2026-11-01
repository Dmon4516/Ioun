<script lang="ts">
  import type { DocumentRecord } from "$lib/types";
  import { settings } from "$lib/stores/settings.svelte";
  import { documents } from "$lib/stores/documents.svelte";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { displayTitle, formatBytes, sectionTitle } from "$lib/utils/format";
  import { openDocument } from "$lib/pdf/loader";
  import Icon from "$components/Icon.svelte";
  import CommentsSection from "./CommentsSection.svelte";
  import BookmarksSection from "./BookmarksSection.svelte";

  let { doc }: { doc: DocumentRecord } = $props();

  // Igual que en el índice: solo recalcular si cambia el documento, no en cada
  // zoom o cambio de página (el store sustituye el objeto entero).
  const docId = $derived(doc.id);
  const docPath = $derived(doc.path);

  const activeTab = $derived(getActiveTab());
  const categories = $derived(
    doc.categoryIds
      .map((id) => documents.categories.find((candidate) => candidate.id === id))
      .filter((candidate): candidate is (typeof documents.categories)[number] => candidate !== undefined),
  );

  let pdfMeta = $state<{ title?: string; author?: string; producer?: string; createdAt?: string } | null>(null);

  let metaGeneration = 0;

  // Dependencias granulares: id y ruta derivados, no el objeto `doc`.
  $effect(() => {
    const id = docId;
    const path = docPath;
    const token = ++metaGeneration;
    pdfMeta = null;
    void (async () => {
      try {
        const pdf = await openDocument(id, path);
        if (token !== metaGeneration) return;
        const { info } = await pdf.getMetadata();
        if (token !== metaGeneration) return;
        const anyInfo = info as Record<string, unknown> | undefined;
        pdfMeta = {
          title: typeof anyInfo?.Title === "string" ? anyInfo.Title : undefined,
          author: typeof anyInfo?.Author === "string" ? anyInfo.Author : undefined,
          producer: typeof anyInfo?.Producer === "string" ? anyInfo.Producer : undefined,
          createdAt: typeof anyInfo?.CreationDate === "string" ? anyInfo.CreationDate : undefined,
        };
      } catch {
        if (token === metaGeneration) pdfMeta = null;
      }
    })();
  });

  let infoCollapsed = $state(false);

  function toggleInfo() {
    infoCollapsed = !infoCollapsed;
  }
</script>

<div class="collapsible" class:collapsed={infoCollapsed}>
  <div class="title-row">
    <div class="panel-title">Información</div>
    <button
      class="collapse-arrow"
      class:collapsed={infoCollapsed}
      type="button"
      title={infoCollapsed ? "Expandir" : "Contraer"}
      onclick={toggleInfo}
    >
      <Icon name="chevron-down" size={14} />
    </button>
  </div>

  {#if !infoCollapsed}
    <div class="info-grid">
      <div class="row"><div class="label">Documento</div><div class="value">{displayTitle(doc.title, settings.showExtensions)}</div></div>
      {#if pdfMeta?.title && pdfMeta.title !== doc.title}
        <div class="row"><div class="label">Título PDF</div><div class="value">{pdfMeta.title}</div></div>
      {/if}
      {#if pdfMeta?.author}
        <div class="row"><div class="label">Autor</div><div class="value">{pdfMeta.author}</div></div>
      {/if}
      {#if pdfMeta?.producer}
        <div class="row"><div class="label">Productor</div><div class="value">{pdfMeta.producer}</div></div>
      {/if}
      {#if pdfMeta?.createdAt}
        <div class="row"><div class="label">Creado</div><div class="value">{pdfMeta.createdAt}</div></div>
      {/if}
      <div class="row"><div class="label">Ruta</div><div class="value mono">{doc.path}</div></div>
      <div class="row"><div class="label">Tamaño</div><div class="value">{formatBytes(doc.sizeBytes)}</div></div>
      <div class="row"><div class="label">Páginas</div><div class="value">{doc.pages > 0 ? doc.pages : "—"}</div></div>
      <div class="row"><div class="label">Tipo</div><div class="value">PDF · <span class="dim">application/pdf</span></div></div>
      {#if categories.length}
        <div class="row">
          <div class="label">Categorías</div>
          <div class="value">{categories.map((category) => category.name).join(", ")}</div>
        </div>
      {/if}
      {#if activeTab}
        <div class="row"><div class="label">Página</div><div class="value">{activeTab.page} · {sectionTitle(Math.ceil(activeTab.page / 5))}</div></div>
        <div class="row"><div class="label">Zoom</div><div class="value">{Math.round(activeTab.zoom)}%</div></div>
      {/if}
    </div>
  {/if}
</div>

<CommentsSection docId={doc.id} />
<BookmarksSection docId={doc.id} />

<style>
  .collapsible { margin-bottom: 6px; }

  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin: 2px 0 14px;
  }

  .panel-title {
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .collapse-arrow {
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    transition: transform var(--animation-fast), background var(--animation-fast), color var(--animation-fast);
  }
  .collapse-arrow:hover { background: var(--bg-active); color: var(--text-primary); }
  .collapse-arrow.collapsed { transform: rotate(-90deg); }

  .info-grid {
    display: grid;
    gap: 10px;
    margin-bottom: 18px;
  }

  .row {
    display: grid;
    grid-template-columns: 92px 1fr;
    gap: 10px;
    font-size: 13px;
    line-height: 1.5;
  }
  .label { color: var(--text-secondary); }
  .value { overflow-wrap: anywhere; }
  .value.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11.5px; }
  .dim { color: var(--text-secondary); }
</style>
