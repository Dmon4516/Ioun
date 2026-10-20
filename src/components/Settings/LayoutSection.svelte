<script lang="ts">
  import { settings, updateSettings } from "$lib/stores/settings.svelte";
  import type { DocumentView, Settings } from "$lib/types";
  import { documents, updateDocView, clearDocView } from "$lib/stores/documents.svelte";
  import { getActiveTab, setConsistentDocZoom } from "$lib/stores/tabs.svelte";
  import {
    effectiveContinuousScroll,
    effectivePagesPerRow,
    docHasViewOverrides,
  } from "$lib/utils/documentView";
  import { showToast } from "$lib/stores/toast.svelte";

  const pageCounts = [1, 2, 3, 4] as const;

  type BooleanSetting = {
    [K in keyof Settings]: Settings[K] extends boolean ? K : never;
  }[keyof Settings];

  function toggleBool(key: BooleanSetting): void {
    updateSettings(key, !settings[key]);
  }

  const activeDoc = $derived.by(() => {
    const tab = getActiveTab();
    return tab ? documents.items.get(tab.docId) ?? null : null;
  });

  async function setDocumentView(patch: DocumentView): Promise<void> {
    if (!activeDoc) return;
    try {
      await updateDocView(activeDoc.id, patch);
      showToast("Preferencia del documento guardada.");
    } catch (error) {
      console.error("[settings] No se pudo guardar la preferencia del documento", error);
      showToast("No se pudo guardar la preferencia del documento.");
    }
  }

  async function resetDocumentView(): Promise<void> {
    if (!activeDoc) return;
    try {
      await clearDocView(activeDoc.id);
      showToast("Se usan de nuevo los valores predeterminados.");
    } catch (error) {
      console.error("[settings] No se pudo restablecer la vista del documento", error);
      showToast("No se pudo restablecer la vista del documento.");
    }
  }

  function saveDocumentViewAsDefault(): void {
    if (!activeDoc) return;
    updateSettings("pagesPerRow", effectivePagesPerRow(activeDoc));
    updateSettings("continuousScroll", effectiveContinuousScroll(activeDoc));
    showToast("Preferencias guardadas como predeterminadas.");
  }
</script>

<div class="settings-group">
  <div class="settings-group-title">Documento actual</div>
  {#if activeDoc}
    <div class="row pages-per-view-row">
      <div>
        <div class="label">Páginas por vista</div>
        <div class="desc">Aplica solo al documento de la pestaña activa.</div>
      </div>
      <div class="segmented four pages-per-view-control" role="group" aria-label="Páginas por vista">
        {#each pageCounts as count (count)}
          <button
            class="segmented-button"
            class:active={effectivePagesPerRow(activeDoc) === count}
            type="button"
            aria-label={`${count} página${count === 1 ? "" : "s"} por vista`}
            aria-pressed={effectivePagesPerRow(activeDoc) === count}
            onclick={() => void setDocumentView({ pagesPerRow: count })}
          >{count}</button>
        {/each}
      </div>
    </div>

    <div class="row">
      <div>
        <div class="label">Scroll continuo</div>
        <div class="desc">Desplazarse por todas las páginas.</div>
      </div>
      <button
        class="switch"
        class:active={effectiveContinuousScroll(activeDoc)}
        type="button"
        role="switch"
        aria-label="Scroll continuo para el documento actual"
        aria-checked={effectiveContinuousScroll(activeDoc)}
        onclick={() => void setDocumentView({
          continuousScroll: !effectiveContinuousScroll(activeDoc),
        })}
      ></button>
    </div>

    <div class="document-view-actions">
      {#if docHasViewOverrides(activeDoc)}
        <button class="subtle-button" type="button" onclick={() => void resetDocumentView()}>
          Volver al valor predeterminado
        </button>
      {/if}
      <button class="subtle-button" type="button" onclick={saveDocumentViewAsDefault}>
        Guardar como predeterminado
      </button>
    </div>
  {:else}
    <div class="desc">Abre un documento para configurar su vista.</div>
  {/if}
</div>

<div class="settings-group">
  <div class="settings-group-title">Layout</div>

  <div class="row">
    <div>
      <div class="label">Abrir última posición</div>
      <div class="desc">Al abrir un documento, ir a la última página y zoom guardados.</div>
    </div>
    <button
      class="switch"
      class:active={settings.openLastLocation}
      type="button"
      role="switch"
      aria-label="Abrir en la última posición guardada"
      aria-checked={settings.openLastLocation}
      onclick={() => toggleBool("openLastLocation")}
    ></button>
  </div>

  <div class="row">
    <div>
      <div class="label">Rutas</div>
      <div class="desc">Mostrar siempre, no mostrar o expandir al pasar el cursor.</div>
    </div>
    <div class="segmented three">
      {#each ["show", "hide", "expand"] as mode (mode)}
        <button
          class="segmented-button"
          class:active={settings.pathDisplay === mode}
          type="button"
          onclick={() => updateSettings("pathDisplay", mode as "show" | "hide" | "expand")}
        >
          {mode === "show" ? "Mostrar" : mode === "hide" ? "Ocultar" : "Hover"}
        </button>
      {/each}
    </div>
  </div>

  <div class="row">
    <div>
      <div class="label">Ruta en barra de título</div>
      <div class="desc">Si se desactiva, el nombre del documento se muestra más grande.</div>
    </div>
    <button
      class="switch"
      class:active={settings.showTitlebarPath}
      type="button"
      role="switch"
      aria-label="Mostrar ruta en la barra superior"
      aria-checked={settings.showTitlebarPath}
      onclick={() => toggleBool("showTitlebarPath")}
    ></button>
  </div>

  <div class="row page-indicator-row">
    <div>
      <div class="label">Indicador de página</div>
      <div class="desc">Elige cuándo aparece la barra al cambiar de página.</div>
    </div>
    <div class="segmented three page-indicator-control" role="group" aria-label="Cuándo mostrar el indicador de página">
      {#each [["always", "Siempre"], ["change", "Al cambiar"], ["never", "Nunca"]] as [mode, text] (mode)}
        <button
          class="segmented-button"
          class:active={settings.pageIndicatorMode === mode}
          type="button"
          aria-pressed={settings.pageIndicatorMode === mode}
          onclick={() => updateSettings("pageIndicatorMode", mode as "always" | "change" | "never")}
        >{text}</button>
      {/each}
    </div>
  </div>

  <div class="row page-indicator-row">
    <div>
      <div class="label">Texto del indicador</div>
      <div class="desc">Mostrar «Página» antes del número o solo el número.</div>
    </div>
    <div class="segmented two page-indicator-control" role="group" aria-label="Formato del indicador de página">
      {#each [["page", "Página + n.º"], ["number", "Solo n.º"]] as [format, text] (format)}
        <button
          class="segmented-button"
          class:active={settings.pageIndicatorFormat === format}
          type="button"
          aria-pressed={settings.pageIndicatorFormat === format}
          onclick={() => updateSettings("pageIndicatorFormat", format as "page" | "number")}
        >{text}</button>
      {/each}
    </div>
  </div>

  <div class="row">
    <div>
      <div class="label">Desplazar títulos largos</div>
      <div class="desc">Si el nombre no cabe, se desplaza de ida y vuelta (en la lista de archivos, al pasar el ratón).</div>
    </div>
    <button
      class="switch"
      class:active={settings.marqueeTitle}
      type="button"
      role="switch"
      aria-label="Animar títulos largos"
      aria-checked={settings.marqueeTitle}
      onclick={() => toggleBool("marqueeTitle")}
    ></button>
  </div>

  <div class="row">
    <div>
      <div class="label">Mostrar extensión de archivos</div>
      <div class="desc">Si se desactiva, se oculta la extensión (.pdf) en los nombres.</div>
    </div>
    <button
      class="switch"
      class:active={settings.showExtensions}
      type="button"
      role="switch"
      aria-label="Mostrar extensiones de archivo"
      aria-checked={settings.showExtensions}
      onclick={() => toggleBool("showExtensions")}
    ></button>
  </div>

  <div class="row">
    <div>
      <div class="label">Mostrar iconos</div>
      <div class="desc">Iconos de carpeta y archivo junto a los nombres.</div>
    </div>
    <button
      class="switch"
      class:active={settings.showIcons}
      type="button"
      role="switch"
      aria-label="Mostrar iconos"
      aria-checked={settings.showIcons}
      onclick={() => toggleBool("showIcons")}
    ></button>
  </div>

  <div class="row">
    <div>
      <div class="label">Nivel de carpeta</div>
      <div class="desc">Cantidad de niveles de carpeta mostrados.</div>
    </div>
    <div class="segmented four">
      {#each [1, 2, 3, 0] as d (d)}
        <button
          class="segmented-button"
          class:active={settings.pathDepth === d}
          type="button"
          onclick={() => updateSettings("pathDepth", d)}
        >
          {d === 0 ? "Todo" : d}
        </button>
      {/each}
    </div>
  </div>

  <div class="row">
    <div>
      <div class="label">Tamaño de página consistente</div>
      <div class="desc">Mantiene el tamaño al redimensionar la ventana.</div>
    </div>
    <button
      class="switch"
      class:active={settings.consistentPageSize}
      type="button"
      role="switch"
      aria-label="Mantener tamaño de página consistente"
      aria-checked={settings.consistentPageSize}
      onclick={() => toggleBool("consistentPageSize")}
    ></button>
  </div>

  <div class="row">
    <div>
      <div class="label">Zoom común entre documentos</div>
      <div class="desc">Al activarlo, el zoom actual se aplica a los documentos abiertos y a los que abras después.</div>
    </div>
    <button
      class="switch"
      class:active={settings.consistentDocZoom}
      type="button"
      role="switch"
      aria-label="Usar un zoom común entre documentos"
      aria-checked={settings.consistentDocZoom}
      onclick={() => setConsistentDocZoom(!settings.consistentDocZoom)}
    ></button>
  </div>

  <div class="row">
    <div>
      <div class="label">Barra principal</div>
      <div class="desc">Automática se oculta tras inactividad. Siempre visible la mantiene fija.</div>
    </div>
    <div class="segmented two">
      {#each [["auto", "Automática"], ["always", "Siempre"]] as [mode, text] (mode)}
        <button
          class="segmented-button"
          class:active={settings.bottomBarMode === mode}
          type="button"
          onclick={() => updateSettings("bottomBarMode", mode as "auto" | "always")}
        >{text}</button>
      {/each}
    </div>
  </div>

  <div class="row">
    <div>
      <div class="label">Barras de desplazamiento</div>
      <div class="desc">Automático las muestra al pasar el cursor. Siempre las mantiene visibles.</div>
    </div>
    <div class="segmented two">
      {#each [["auto", "Automático"], ["always", "Siempre"]] as [mode, text] (mode)}
        <button
          class="segmented-button"
          class:active={settings.scrollbarMode === mode}
          type="button"
          onclick={() => updateSettings("scrollbarMode", mode as "auto" | "always")}
        >{text}</button>
      {/each}
    </div>
  </div>

  <div class="row">
    <div>
      <div class="label">Estilo de superficies</div>
      <div class="desc">Frosted usa transparencia y desenfoque. Opaco usa colores sólidos.</div>
    </div>
    <div class="segmented two">
      {#each [["frosted", "Frosted"], ["opaque", "Opaco"]] as [mode, text] (mode)}
        <button
          class="segmented-button"
          class:active={settings.surfaceStyle === mode}
          type="button"
          onclick={() => updateSettings("surfaceStyle", mode as "frosted" | "opaque")}
        >{text}</button>
      {/each}
    </div>
  </div>

  <div class="row">
    <div>
      <div class="label">Barra de información</div>
      <div class="desc">Panel que abre por defecto el botón de información.</div>
    </div>
    <div class="segmented three">
      {#each [["info", "Información"], ["outline", "Índice"], ["thumbs", "Miniaturas"]] as [panel, text] (panel)}
        <button
          class="segmented-button"
          class:active={settings.defaultPanel === panel}
          type="button"
          onclick={() => updateSettings("defaultPanel", panel as "info" | "outline" | "thumbs")}
        >{text}</button>
      {/each}
    </div>
  </div>

  <div class="row">
    <div>
      <div class="label">Recentrar al ocultar barras</div>
      <div class="desc">Centra el documento en el nuevo espacio disponible.</div>
    </div>
    <button
      class="switch"
      class:active={settings.recenterOnToggle}
      type="button"
      role="switch"
      aria-label="Recentrar al cambiar paneles"
      aria-checked={settings.recenterOnToggle}
      onclick={() => toggleBool("recenterOnToggle")}
    ></button>
  </div>

  <div class="row titlebar-visibility-row">
    <div>
      <div class="label">Visibilidad de la barra de título</div>
      <div class="desc">Al ocultarse, vuelve a aparecer al acercar el puntero al borde superior.</div>
    </div>
    <div class="segmented four titlebar-visibility-control" role="group" aria-label="Visibilidad de la barra de título">
      {#each [["always", "Ocultar siempre"], ["never", "Nunca ocultar"], ["auto", "Automáticamente"], ["fullscreen", "Automáticamente solo en pantalla completa"]] as [mode, text] (mode)}
        <button
          class="segmented-button"
          class:active={settings.titlebarVisibility === mode}
          type="button"
          aria-pressed={settings.titlebarVisibility === mode}
          onclick={() => updateSettings("titlebarVisibility", mode as "always" | "never" | "auto" | "fullscreen")}
        >{text}</button>
      {/each}
    </div>
  </div>
</div>

<style>
  .settings-group {
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg-button) 36%, transparent);
    border-radius: var(--radius-panel);
    padding: 13px;
    margin-top: 12px;
  }

  .settings-group-title {
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-secondary);
    margin-bottom: 10px;
  }

  .titlebar-visibility-row {
    flex-wrap: wrap;
  }

  .segmented.four.titlebar-visibility-control {
    width: 100%;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .titlebar-visibility-control .segmented-button {
    min-width: 0;
    white-space: normal;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 8px 0;
  }

  .pages-per-view-row {
    align-items: stretch;
    flex-direction: column;
    gap: 10px;
  }

  .pages-per-view-control {
    width: 100%;
  }

  .page-indicator-control {
    width: 100%;
  }

  .page-indicator-row {
    align-items: stretch;
    flex-direction: column;
    gap: 10px;
  }

  .label { font-size: 13px; font-weight: 630; }
  .desc { margin-top: 3px; font-size: 12px; color: var(--text-secondary); line-height: 1.45; max-width: 280px; }

  .switch {
    width: 44px;
    height: 27px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--bg-button);
    position: relative;
    flex: 0 0 auto;
    transition: background var(--animation-fast), border-color var(--animation-fast);
  }
  .switch::after {
    content: "";
    position: absolute;
    width: 21px;
    height: 21px;
    border-radius: 50%;
    background: #fff;
    top: 2px;
    left: 2px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.22);
    transition: transform var(--animation-fast);
  }
  .switch.active {
    background: color-mix(in srgb, var(--accent) 24%, var(--bg-button));
    border-color: color-mix(in srgb, var(--accent) 42%, var(--border));
  }
  .switch.active::after { transform: translateX(17px); }

  .segmented {
    display: grid;
    gap: 6px;
    flex: 0 0 auto;
  }
  .segmented.two { grid-template-columns: 1fr 1fr; }
  .segmented.three { grid-template-columns: repeat(3, 1fr); }
  .segmented.four { grid-template-columns: repeat(4, 1fr); }

  .segmented-button {
    border: 1px solid var(--border);
    background: var(--bg-button);
    border-radius: 10px;
    padding: 7px 9px;
    font-size: 12px;
    color: var(--text-primary);
    white-space: nowrap;
    transition: background var(--animation-fast), border-color var(--animation-fast), color var(--animation-fast);
  }
  .segmented-button.active {
    background: var(--bg-active);
    border-color: color-mix(in srgb, var(--accent) 36%, var(--border));
  }

  .document-view-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  .subtle-button {
    flex: 1 1 0;
    width: 100%;
    border: 1px solid var(--border);
    background: var(--bg-button);
    color: var(--text-primary);
    border-radius: var(--radius-card);
    padding: 11px 12px;
    font-size: 13px;
    font-weight: 650;
  }

  .subtle-button:hover { background: var(--bg-active); }
</style>
