<script lang="ts">
  import { setContext, untrack } from "svelte";
  import type { DocumentRecord } from "$lib/types";
  import { settings, updateSettings } from "$lib/stores/settings.svelte";
  import { getActiveTab } from "$lib/stores/tabs.svelte";
  import { requestScrollToPage } from "$lib/stores/viewerCommands.svelte";
  import { ThumbVisibility } from "$lib/pdf/thumbVisibility.svelte";
  import { getDocumentAspect, hasDocumentAspect } from "$lib/pdf/documentAspect.svelte";
  import { thumbRowHeight } from "$lib/utils/thumbGrid";
  import ThumbCard from "./ThumbCard.svelte";

  let { doc }: { doc: DocumentRecord } = $props();

  const activeTab = $derived(getActiveTab());
  const activePage = $derived(activeTab?.page ?? 1);

  const visibility = new ThumbVisibility();
  setContext<ThumbVisibility>("thumbVisibility", visibility);
  let gridEl: HTMLElement | null = $state(null);
  let columns = $state(1);
  let rowHeight = $state(160);
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let gridOffset = 0;
  /** Mientras el usuario navega la lista (rueda, toca o arrastra) no se le interrumpe. */
  let userBrowsing = $state(false);
  let browseIdleTimer = 0;
  // Proporción medida por el cargador al abrir el documento (página 1).
  const pageRatio = $derived(getDocumentAspect(doc.id));
  const aspectKnown = $derived(hasDocumentAspect(doc.id));
  let positionedFor: string | null = null;
  let measureRetry = 0;

  const GAP = 10;
  const OVERSCAN_ROWS = 4;
  const totalRows = $derived(Math.ceil(doc.pages / columns));
  const rowPitch = $derived(rowHeight + GAP);
  const visibleRowStart = $derived(
    Math.max(0, Math.floor(scrollTop / rowPitch) - OVERSCAN_ROWS),
  );
  const visibleRowEnd = $derived(
    Math.min(
      totalRows - 1,
      Math.ceil((scrollTop + viewportHeight) / rowPitch) + OVERSCAN_ROWS,
    ),
  );
  const firstPage = $derived(visibleRowStart * columns + 1);
  const lastPage = $derived(Math.min(doc.pages, (visibleRowEnd + 1) * columns));
  const visiblePages = $derived.by(() => {
    const result: number[] = [];
    for (let page = firstPage; page <= lastPage; page++) result.push(page);
    return result;
  });
  const topRows = $derived(visibleRowStart);
  const bottomRows = $derived(Math.max(0, totalRows - visibleRowEnd - 1));

  function measureGrid(): boolean {
    if (!gridEl) return false;
    const tracks = getComputedStyle(gridEl).gridTemplateColumns.split(" ").filter(Boolean);
    if (!tracks.length || tracks[0] === "none") return false;
    columns = Math.max(1, tracks.length);
    // El ancho sale de la primera celda medida; interpretar la plantilla daría
    // un valor falso si alguien la cambiara a `fr` puras (`parseFloat("1fr")` = 1).
    const measured = gridEl.querySelector<HTMLElement>(".thumb-card")?.getBoundingClientRect().width ?? 0;
    const width = measured > 0 ? measured : Number.parseFloat(tracks[0]!) || 0;
    if (width <= 0) return false;
    rowHeight = thumbRowHeight(width, pageRatio);
    return true;
  }

  function measureGridOffset(): void {
    if (!gridEl) return;
    const root = document.getElementById("inspectorContent");
    if (!root) return;
    gridOffset = gridEl.getBoundingClientRect().top -
      root.getBoundingClientRect().top + root.scrollTop;
  }

  function readScrollState(): void {
    const root = document.getElementById("inspectorContent");
    if (!root) return;
    scrollTop = Math.max(0, root.scrollTop - gridOffset);
    viewportHeight = root.clientHeight;
  }

  $effect(() => {
    void settings.thumbSizePx;
    void pageRatio;
    if (!gridEl) return;
    const frame = requestAnimationFrame(() => {
      // The inspector can be mounted before its panel transition has produced
      // a measurable grid. Keep retrying for a few frames instead of leaving
      // virtualization with a zero-sized layout after a slow open.
      if (!measureGrid() && measureRetry < 12) {
        measureRetry += 1;
        requestAnimationFrame(() => {
          measureGrid();
          measureGridOffset();
          readScrollState();
        });
      } else {
        measureRetry = 0;
      }
      measureGridOffset();
      readScrollState();
      visibility.notifyLayoutChange();
    });
    const observer = new ResizeObserver(() => {
      measureGrid();
      measureGridOffset();
      readScrollState();
    });
    observer.observe(gridEl);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  });

  $effect(() => {
    const root = document.getElementById("inspectorContent");
    if (!root) return;
    const onScroll = () => readScrollState();
    root.addEventListener("scroll", onScroll, { passive: true });
    const observer = new ResizeObserver(() => readScrollState());
    observer.observe(root);
    readScrollState();
    return () => {
      root.removeEventListener("scroll", onScroll);
      observer.disconnect();
    };
  });

  $effect(() => {
    const root = document.getElementById("inspectorContent");
    visibility.attach(root, gridEl);
    return () => visibility.detach();
  });

  /**
   * Marca que el usuario está navegando la lista por su cuenta. Mientras dura el
   * gesto (y un margen de reposo) el seguimiento de la página activa no
   * recoloca el scroll: si el visor sigue asentándose (un zoom o un salto
   * suave pendiente) y cambia la página, la lista no debe arrebatarle la
   * posición al usuario.
   */
  $effect(() => {
    const el = gridEl;
    if (!el) return;
    const mark = () => {
      userBrowsing = true;
      clearTimeout(browseIdleTimer);
      browseIdleTimer = window.setTimeout(() => {
        userBrowsing = false;
      }, 450);
    };
    el.addEventListener("wheel", mark, { passive: true });
    el.addEventListener("touchstart", mark, { passive: true });
    el.addEventListener("pointerdown", mark);
    return () => {
      el.removeEventListener("wheel", mark);
      el.removeEventListener("touchstart", mark);
      el.removeEventListener("pointerdown", mark);
      clearTimeout(browseIdleTimer);
    };
  });

  /**
   * Deja centrada la miniatura de una página. Al abrir se coloca de golpe (el
   * recorrido puede ser de cientos de filas) y al seguir al visor se anima, para
   * que la lista acompañe al documento con el mismo desplazamiento.
   */
  function centerOnPage(page: number, smooth: boolean): boolean {
    const root = document.getElementById("inspectorContent");
    if (!root || !measureGrid()) return false;
    measureGridOffset();
    const row = Math.max(0, Math.floor((page - 1) / columns));
    const top = Math.max(
      0,
      gridOffset + row * rowPitch - Math.max(0, (root.clientHeight - rowHeight) / 2),
    );
    if (smooth) root.scrollTo({ top, behavior: "smooth" });
    else root.scrollTop = top;
    readScrollState();
    return true;
  }

  /**
   * Al abrir el panel se sitúa en la página actual: el panel se monta cada vez
   * que se abre, así que el scroll inicial tiene que salir de la pestaña activa.
   */
  $effect(() => {
    const docId = doc.id;
    if (!aspectKnown || positionedFor === docId) return;
    // La página se lee sin suscribirse: cambiar de página se sigue aparte.
    const page = untrack(() => activePage);
    if (centerOnPage(page, false)) {
      positionedFor = docId;
      return;
    }
    // La rejilla aún no tiene medidas: se reintenta en el siguiente frame.
    const frame = requestAnimationFrame(() => {
      if (centerOnPage(page, false)) positionedFor = docId;
    });
    return () => cancelAnimationFrame(frame);
  });

  /**
   * El panel sigue a la página que se está viendo: cuando el visor avanza (o
   * retrocede) deja centrada su miniatura, con el mismo desplazamiento y de
   * forma animada. La única dependencia del efecto es `activePage`.
   *
   * `centerOnPage` lee y escribe estado propio de la lista (`columns`,
   * `rowHeight`, `scrollTop`, …): se llama dentro de `untrack` para que esas
   * lecturas no se conviertan en dependencias y el efecto no se re-dispare por
   * su propio trabajo. `userBrowsing` también se lee sin suscribirse: no se
   * quiere «recuperar» el scroll al terminar el gesto, solo dejar de interrumpir
   * al usuario mientras navega.
   */
  $effect(() => {
    const page = activePage;
    if (!aspectKnown || positionedFor !== doc.id) return;
    if (untrack(() => userBrowsing)) return;
    untrack(() => centerOnPage(page, true));
  });


  function goTo(page: number) {
    requestScrollToPage(page, true);
  }

</script>

<div class="thumb-heading">
  <div class="panel-title">Miniaturas</div>
  <label class="thumb-size-control">
    <span>Tamaño</span>
    <input
      type="range"
      min="60"
      max="200"
      step="4"
      aria-label="Tamaño de miniaturas"
      value={settings.thumbSizePx}
      oninput={(event) =>
        updateSettings("thumbSizePx", Number(event.currentTarget.value))}
    />
  </label>
</div>

<div
  class="thumb-grid"
  bind:this={gridEl}
  style:--thumb-min={`${settings.thumbSizePx}px`}
  style:--thumb-row-height={`${rowHeight}px`}
>
  {#if topRows > 0}
    <div class="thumb-spacer" style:grid-row={`span ${topRows}`} aria-hidden="true"></div>
  {/if}
  {#each visiblePages as p (`${doc.id}:${p}`)}
    <ThumbCard
      docId={doc.id}
      docPath={doc.path}
      page={p}
      aspect={pageRatio}
      current={p === activePage}
      onclick={() => goTo(p)}
    />
  {/each}
  {#if bottomRows > 0}
    <div class="thumb-spacer" style:grid-row={`span ${bottomRows}`} aria-hidden="true"></div>
  {/if}
</div>

<style>
  .thumb-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 2px 0 14px;
  }

  .panel-title {
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.02em;
    margin: 0;
  }

  .thumb-size-control {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .thumb-size-control input {
    width: 92px;
    accent-color: var(--accent);
  }

  .thumb-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(var(--thumb-min, 100px), 100%), 1fr));
    grid-auto-rows: var(--thumb-row-height, 160px);
    gap: 10px;
    padding-bottom: 10px;
  }

  .thumb-spacer {
    grid-column: 1 / -1;
    pointer-events: none;
  }
</style>
