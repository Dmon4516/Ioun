
<script lang="ts">
  import type { DocumentRecord } from "$lib/types";
  import type { OutlineNode } from "$lib/pdf/outline";
  import { buildOutline } from "$lib/pdf/outline";
  import { openDocument } from "$lib/pdf/loader";
  import { requestScrollToPage } from "$lib/stores/viewerCommands.svelte";
  import { nextTick } from "$lib/utils/dom";
  import OutlineNodeComponent from "./OutlineNode.svelte";

  let { doc }: { doc: DocumentRecord } = $props();

  // El store reemplaza el objeto del documento en cada zoom o cambio de página.
  // Con `$derived` estas claves solo cambian cuando cambia el valor, así que
  // volver a hacer zoom no vuelve a construir el índice.
  const docId = $derived(doc.id);
  const docPath = $derived(doc.path);

  let tree = $state<OutlineNode[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let listEl: HTMLDivElement | null = $state(null);
  let destroyed = false;

  // El ancho del panel se adapta al título más largo del índice cargado.
  const OUTLINE_MIN_WIDTH = 324;
  const OUTLINE_WIDTH_RATIO = 0.9;
  const PANEL_CHROME = 10 + 14 + 1; // padding de la fila, del panel y borde

  function setOutlineWidth(value: string): void {
    document.body.style.setProperty("--inspector-outline-width", value);
  }

  function compactOutlineWidth(): number {
    const max = window.innerWidth * OUTLINE_WIDTH_RATIO;
    return Math.round(Math.min(OUTLINE_MIN_WIDTH, max));
  }

  function requiredOutlineWidth(): number {
    const list = listEl;
    const panel = document.getElementById("inspectorEl");
    const rows = list?.querySelectorAll<HTMLElement>(".outline-row") ?? [];
    const panelLeft = panel?.getBoundingClientRect().left ?? 0;

    const probe = document.createElement("span");
    probe.style.cssText =
      "position:absolute;top:-9999px;left:-9999px;visibility:hidden;white-space:pre;width:max-content;pointer-events:none;";
    document.body.appendChild(probe);

    let required = 0;
    for (const row of rows) {
      const title = row.querySelector<HTMLElement>(".title");
      const label = title?.textContent ?? "";
      if (!title || !label) continue;
      const styles = getComputedStyle(title);
      probe.style.fontFamily = styles.fontFamily;
      probe.style.fontSize = styles.fontSize;
      probe.style.fontWeight = styles.fontWeight;
      probe.style.letterSpacing = styles.letterSpacing;
      probe.textContent = label;
      const page = row.querySelector<HTMLElement>(".page");
      const pageWidth = page ? page.getBoundingClientRect().width + 6 : 0;
      required = Math.max(
        required,
        title.getBoundingClientRect().left -
          panelLeft +
          probe.getBoundingClientRect().width +
          pageWidth +
          PANEL_CHROME,
      );
    }
    probe.remove();
    return required;
  }

  function applyOutlineWidth(): void {
    if (destroyed) return;
    const required = requiredOutlineWidth();
    const max = window.innerWidth * OUTLINE_WIDTH_RATIO;
    if (required <= 0) {
      setOutlineWidth(`${compactOutlineWidth()}px`);
      return;
    }
    const width = Math.round(Math.min(Math.max(required, compactOutlineWidth()), max));
    setOutlineWidth(`${width}px`);
  }

  $effect(() => {
    void tree;
    if (loading) return;
    void nextTick().then(applyOutlineWidth);
  });

  $effect(() => {
    const list = listEl;
    if (!list) {
      applyOutlineWidth();
      return;
    }
    let alive = true;
    const observer = new MutationObserver(() => applyOutlineWidth());
    observer.observe(list, { childList: true, subtree: true });
    const onResize = () => applyOutlineWidth();
    window.addEventListener("resize", onResize);
    void document.fonts?.ready.then(() => {
      if (alive) applyOutlineWidth();
    });
    return () => {
      alive = false;
      observer.disconnect();
      window.removeEventListener("resize", onResize);
    };
  });

  $effect(() => {
    return () => {
      destroyed = true;
      document.body.style.removeProperty("--inspector-outline-width");
    };
  });

  $effect(() => {
    const id = docId;
    const path = docPath;
    let cancelled = false;
    loading = true;
    error = null;
    tree = [];
    void (async () => {
      try {
        const pdf = await openDocument(id, path);
        if (!pdf || cancelled) return;
        const outline = await buildOutline(pdf);
        if (cancelled) return;
        tree = outline;
      } catch (err) {
        if (!cancelled) error = err instanceof Error ? err.message : String(err);
      } finally {
        if (!cancelled) loading = false;
      }
    })();
    return () => { cancelled = true; };
  });

  function goTo(page: number | null) {
    if (!page) return;
    requestScrollToPage(page, true);
  }
</script>

<div class="panel-title">Índice</div>

{#if loading}
  <div class="hint">Cargando índice…</div>
{:else if error}
  <div class="hint">Error: {error}</div>
{:else if tree.length === 0}
  <div class="hint">Este documento no tiene índice.</div>
{:else}
  <div class="outline-list" bind:this={listEl}>
    {#each tree as node, i (i)}
      <OutlineNodeComponent {node} level={1} onNavigate={goTo} />
    {/each}
  </div>
{/if}

<style>
  .panel-title { font-size: 17px; font-weight: 700; letter-spacing: -0.02em; margin: 2px 0 14px; }
  .hint { color: var(--text-secondary); font-size: 13px; padding: 4px 0; }
  .outline-list { display: grid; gap: 2px; }
</style>
