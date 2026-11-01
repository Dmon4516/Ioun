<script lang="ts">
  import { settings, updateSettings, resetSettingsToDefaults } from "$lib/stores/settings.svelte";
  import type { Settings } from "$lib/types";
  import Icon from "$components/Icon.svelte";
  import { confirmDialog } from "$lib/tauri/dialogs";
  import {
    exportLibraryToFile,
    importLibraryFromFile,
    listCategories,
    listDocuments,
  } from "$lib/tauri/commands";
  import { reloadLibrary } from "$lib/stores/library.svelte";
  import { reconcileDocuments, setCategories } from "$lib/stores/documents.svelte";
  import { showToast } from "$lib/stores/toast.svelte";

  let open = $state(false);
  let busy = $state(false);
  type NumericSetting = {
    [K in keyof Settings]: Settings[K] extends number ? K : never;
  }[keyof Settings];

  function clampInt(v: number, min: number, max: number, fallback: number): number {
    const n = Number.isFinite(v) ? Math.trunc(v) : fallback;
    return Math.min(max, Math.max(min, n));
  }

  function bump(key: NumericSetting, dir: 1 | -1, min: number, max: number, step: number): void {
    const cur = Number(settings[key]) || min;
    const next = clampInt(cur + dir * step, min, max, min);
    updateSettings(key, next);
  }

  async function resetSettings(): Promise<void> {
    if (!(await confirmDialog("¿Restaurar la configuración a los valores originales?"))) return;
    resetSettingsToDefaults();
  }

  async function exportLibraryToJson(): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      const path = await exportLibraryToFile();
      if (!path) return;
      showToast("Biblioteca exportada.");
    } catch (error) {
      console.error("[export]", error);
      showToast("No se pudo exportar la biblioteca.");
    } finally {
      busy = false;
    }
  }

  async function importLibraryFromJson(): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      if (!(await confirmDialog("¿Importar una biblioteca desde un archivo JSON?"))) {
        return;
      }
      const replace = await confirmDialog(
        "¿Reemplazar la biblioteca actual por la importada?\n\n" +
          "Aceptar: reemplaza todo.\nCancelar: fusiona con lo existente.",
        { title: "Importar biblioteca", kind: "warning" },
      );
      if (!replace && !(await confirmDialog(
        "¿Fusionar la biblioteca importada con la actual?",
        { title: "Importar biblioteca", kind: "info" },
      ))) {
        return;
      }

      const summary = await importLibraryFromFile(replace ? "replace" : "merge");
      if (!summary) return;

      const [, storedDocuments, categories] = await Promise.all([
        reloadLibrary(),
        listDocuments(),
        listCategories(),
      ]);
      reconcileDocuments(storedDocuments, replace);
      setCategories(categories);

      showToast(
        replace
          ? `Biblioteca reemplazada (${summary.inserted} elementos).`
          : `Biblioteca fusionada: ${summary.inserted} nuevos, ${summary.skipped} ya existían.`,
      );
    } catch (error) {
      console.error("[import]", error);
      showToast(`No se pudo importar la biblioteca: ${String(error)}`);
    } finally {
      busy = false;
    }
  }
</script>

<div class="settings-group">
  <button
    class="summary"
    class:open
    type="button"
    onclick={() => (open = !open)}
  >
    <span>Configuración avanzada</span>
    <span class="chev">
      <Icon name="chevron-down" size={12} />
    </span>
  </button>

  {#if open}
    <div class="content">
      <div class="grid">
        <div class="field">
          <label for="ramLimitInput">Límite de memoria (MB)</label>
          <div class="stepper">
            <button class="stepper-button" type="button" onclick={() => bump("ramLimitMB", -1, 512, 8192, 256)}>
              <Icon name="minus" size={14} />
            </button>
            <input
              class="number"
              id="ramLimitInput"
              type="number"
              min="512"
              max="8192"
              step="256"
              value={settings.ramLimitMB}
              onchange={(e) => updateSettings("ramLimitMB", clampInt(Number(e.currentTarget.value), 512, 8192, 2048))}
            />
            <button class="stepper-button" type="button" onclick={() => bump("ramLimitMB", 1, 512, 8192, 256)}>
              <Icon name="plus" size={14} />
            </button>
          </div>
        </div>

        <div class="field">
          <label for="librarySortInput">Orden de archivos y carpetas</label>
          <select
            class="select"
            id="librarySortInput"
            value={settings.librarySort}
            onchange={(e) =>
              updateSettings("librarySort", e.currentTarget.value as Settings["librarySort"])}
          >
            <option value="manual">Manual (arrastrar y soltar)</option>
            <option value="name">Nombre (A-Z)</option>
            <option value="type">Tipo de archivo</option>
            <option value="newest">Más recientes primero</option>
            <option value="oldest">Más antiguos primero</option>
          </select>
        </div>

        <div class="field">
          <label for="maxLoadedTabsInput">Pestañas cargadas</label>
          <div class="stepper">
            <button class="stepper-button" type="button" onclick={() => bump("maxLoadedTabs", -1, 1, 20, 1)}>
              <Icon name="minus" size={14} />
            </button>
            <input
              class="number"
              id="maxLoadedTabsInput"
              type="number"
              min="1"
              max="20"
              step="1"
              value={settings.maxLoadedTabs}
              onchange={(e) => updateSettings("maxLoadedTabs", clampInt(Number(e.currentTarget.value), 1, 20, 5))}
            />
            <button class="stepper-button" type="button" onclick={() => bump("maxLoadedTabs", 1, 1, 20, 1)}>
              <Icon name="plus" size={14} />
            </button>
          </div>
        </div>

        <div class="field">
          <label for="maxPagesLoadedInput">Páginas cargadas</label>
          <div class="stepper">
            <button class="stepper-button" type="button" onclick={() => bump("maxPagesLoaded", -1, 3, 60, 1)}>
              <Icon name="minus" size={14} />
            </button>
            <input
              class="number"
              id="maxPagesLoadedInput"
              type="number"
              min="3"
              max="60"
              step="1"
              value={settings.maxPagesLoaded}
              onchange={(e) => updateSettings("maxPagesLoaded", clampInt(Number(e.currentTarget.value), 3, 60, 30))}
            />
            <button class="stepper-button" type="button" onclick={() => bump("maxPagesLoaded", 1, 3, 60, 1)}>
              <Icon name="plus" size={14} />
            </button>
          </div>
        </div>

        <div class="field">
          <label for="maxActiveFilesInput">Archivos abiertos</label>
          <div class="stepper">
            <button class="stepper-button" type="button" onclick={() => bump("maxActiveFiles", -1, 1, 30, 1)}>
              <Icon name="minus" size={14} />
            </button>
            <input
              class="number"
              id="maxActiveFilesInput"
              type="number"
              min="1"
              max="30"
              step="1"
              value={settings.maxActiveFiles}
              onchange={(e) => updateSettings("maxActiveFiles", clampInt(Number(e.currentTarget.value), 1, 30, 10))}
            />
            <button class="stepper-button" type="button" onclick={() => bump("maxActiveFiles", 1, 1, 30, 1)}>
              <Icon name="plus" size={14} />
            </button>
          </div>
        </div>

        <div class="field">
          <label for="maxRecentDocsInput">Archivos recientes</label>
          <div class="stepper">
            <button class="stepper-button" type="button" onclick={() => bump("maxRecentDocs", -1, 1, 100, 1)}>
              <Icon name="minus" size={14} />
            </button>
            <input
              class="number"
              id="maxRecentDocsInput"
              type="number"
              min="1"
              max="100"
              step="1"
              value={settings.maxRecentDocs}
              onchange={(e) => updateSettings("maxRecentDocs", clampInt(Number(e.currentTarget.value), 1, 100, 20))}
            />
            <button class="stepper-button" type="button" onclick={() => bump("maxRecentDocs", 1, 1, 100, 1)}>
              <Icon name="plus" size={14} />
            </button>
          </div>
        </div>

        <div class="field">
          <label for="maxClosedTabsInput">Pestañas cerradas</label>
          <div class="stepper">
            <button class="stepper-button" type="button" onclick={() => bump("maxClosedTabs", -1, 1, 30, 1)}>
              <Icon name="minus" size={14} />
            </button>
            <input
              class="number"
              id="maxClosedTabsInput"
              type="number"
              min="1"
              max="30"
              step="1"
              value={settings.maxClosedTabs}
              onchange={(e) => updateSettings("maxClosedTabs", clampInt(Number(e.currentTarget.value), 1, 30, 5))}
            />
            <button class="stepper-button" type="button" onclick={() => bump("maxClosedTabs", 1, 1, 30, 1)}>
              <Icon name="plus" size={14} />
            </button>
          </div>
        </div>

        <div class="field wide">
          <label for="inactiveTimeoutInput">Inactividad antes de hibernar (segundos)</label>
          <div class="stepper">
            <button class="stepper-button" type="button" onclick={() => bump("inactiveTimeoutSec", -1, 15, 7200, 15)}>
              <Icon name="minus" size={14} />
            </button>
            <input
              class="number"
              id="inactiveTimeoutInput"
              type="number"
              min="15"
              max="7200"
              step="15"
              value={settings.inactiveTimeoutSec}
              onchange={(e) => updateSettings("inactiveTimeoutSec", clampInt(Number(e.currentTarget.value), 15, 7200, 300))}
            />
            <button class="stepper-button" type="button" onclick={() => bump("inactiveTimeoutSec", 1, 15, 7200, 15)}>
              <Icon name="plus" size={14} />
            </button>
          </div>
        </div>

        <div class="field wide">
          <label for="libraryRefreshInput">Refresco de la biblioteca (ms)</label>
          <div class="stepper">
            <button class="stepper-button" type="button" onclick={() => bump("libraryRefreshDelayMs", -1, 200, 10000, 200)}>
              <Icon name="minus" size={14} />
            </button>
            <input
              class="number"
              id="libraryRefreshInput"
              type="number"
              min="200"
              max="10000"
              step="200"
              value={settings.libraryRefreshDelayMs}
              onchange={(e) => updateSettings("libraryRefreshDelayMs", clampInt(Number(e.currentTarget.value), 200, 10000, 1500))}
            />
            <button class="stepper-button" type="button" onclick={() => bump("libraryRefreshDelayMs", 1, 200, 10000, 200)}>
              <Icon name="plus" size={14} />
            </button>
          </div>
        </div>
      </div>

      <div class="sub-title">Carpetas</div>
      <div class="row">
        <div>
          <div class="label">Quitar contenido de Lyra al eliminar una carpeta</div>
          <div class="desc">Elimina de Lyra los documentos de la carpeta y sus subcarpetas, pero conserva los PDF en el disco.</div>
        </div>
        <button
          class="switch"
          class:active={settings.deleteFolderContents}
          type="button"
          role="switch"
          aria-label="Eliminar también los documentos de una carpeta"
          aria-checked={settings.deleteFolderContents}
          onclick={() => updateSettings("deleteFolderContents", !settings.deleteFolderContents)}
        ></button>
      </div>

      <div class="row">
        <div>
          <div class="label">Ignorar la marca de agua al copiar</div>
          <div class="desc">Al copiar texto, quita las líneas idénticas que se repiten en varias páginas (la firma de una marca de agua). Es una heurística: puede no distinguir un encabezado repetido.</div>
        </div>
        <button
          class="switch"
          class:active={settings.ignoreWatermarkOnCopy}
          type="button"
          role="switch"
          aria-label="Ignorar la marca de agua al copiar texto"
          aria-checked={settings.ignoreWatermarkOnCopy}
          onclick={() => updateSettings("ignoreWatermarkOnCopy", !settings.ignoreWatermarkOnCopy)}
        ></button>
      </div>

      <div class="row">
        <div>
          <div class="label">Capa de texto seleccionable</div>
          <div class="desc">Superpone el texto real de cada página para poder seleccionarlo y copiarlo. Consume memoria y tiempo de pintado por página.</div>
        </div>
        <button
          class="switch"
          class:active={settings.textLayerEnabled}
          type="button"
          role="switch"
          aria-label="Activar la capa de texto seleccionable"
          aria-checked={settings.textLayerEnabled}
          onclick={() => updateSettings("textLayerEnabled", !settings.textLayerEnabled)}
        ></button>
      </div>

      <div class="row">
        <div>
          <div class="label">Permitir seleccionar el texto</div>
          <div class="desc">Si se desactiva, la capa sigue existiendo para buscar y copiar, pero arrastrar sobre la página no selecciona: el gesto vuelve a ser de desplazamiento. La lectura por lector de pantalla de la página aún no está soportada.</div>
        </div>
        <button
          class="switch"
          class:active={settings.textLayerSelectable}
          type="button"
          role="switch"
          aria-label="Permitir seleccionar el texto de la página"
          aria-checked={settings.textLayerSelectable}
          disabled={!settings.textLayerEnabled}
          onclick={() => updateSettings("textLayerSelectable", !settings.textLayerSelectable)}
        ></button>
      </div>

      <div class="row">
        <div>
          <div class="label">Permitir mover raíces sincronizadas</div>
          <div class="desc">Si se desactiva, no se puede arrastrar una carpeta sincronizada raíz dentro de otra.</div>
        </div>
        <button
          class="switch"
          class:active={settings.allowReparentSyncedRoot}
          type="button"
          role="switch"
          aria-label="Permitir mover carpetas sincronizadas raíz"
          aria-checked={settings.allowReparentSyncedRoot}
          onclick={() => updateSettings("allowReparentSyncedRoot", !settings.allowReparentSyncedRoot)}
        ></button>
      </div>

      <div class="sub-title">Cierre</div>
      <div class="row">
        <div>
          <div class="label">Flush al cerrar la ventana</div>
          <div class="desc">Si se activa, Lyra espera a guardar ajustes y progreso antes de cerrar. Si se desactiva, un cierre rápido puede perder el último cambio.</div>
        </div>
        <button
          class="switch"
          class:active={settings.flushOnClose}
          type="button"
          role="switch"
          aria-label="Guardar ajustes y progreso al cerrar"
          aria-checked={settings.flushOnClose}
          onclick={() => updateSettings("flushOnClose", !settings.flushOnClose)}
        ></button>
      </div>

      <div class="sub-title">Biblioteca</div>
      <div class="grid-buttons">
        <button class="subtle-button" type="button" disabled={busy} onclick={() => void exportLibraryToJson()}>
          Exportar biblioteca…
        </button>
        <button class="subtle-button" type="button" disabled={busy} onclick={() => void importLibraryFromJson()}>
          Importar biblioteca…
        </button>
      </div>

      <div class="sub-title">Datos</div>
      <div class="grid-buttons">
        <button class="subtle-button" type="button" onclick={() => void resetSettings()}>
          Regresar a configuración original
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .settings-group {
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg-button) 36%, transparent);
    border-radius: var(--radius-panel);
    padding: 0;
    margin-top: 12px;
    overflow: hidden;
  }

  .summary {
    list-style: none;
    cursor: pointer;
    width: 100%;
    padding: 13px;
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    justify-content: space-between;
    border: none;
    background: transparent;
  }
  .summary:hover { color: var(--text-primary); }

  .chev {
    display: grid;
    place-items: center;
    transition: transform var(--animation-fast);
  }
  .summary.open .chev { transform: rotate(180deg); }

  .content { padding: 0 13px 13px; }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .grid-buttons { display: grid; gap: 8px; }
  .field.wide { grid-column: 1 / -1; }

  .field { display: grid; gap: 6px; }
  .field label {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.35;
  }

  .stepper {
    display: grid;
    grid-template-columns: 36px 1fr 36px;
    gap: 7px;
    align-items: center;
  }
  .stepper-button {
    height: 36px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg-button);
    color: var(--text-primary);
    display: grid;
    place-items: center;
  }
  .stepper-button:hover { background: var(--bg-active); }

  .number {
    width: 100%;
    height: 36px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg-input);
    padding: 0 10px;
    text-align: center;
    outline: none;
    color: var(--text-primary);
    appearance: textfield;
    -moz-appearance: textfield;
  }
  .number::-webkit-outer-spin-button,
  .number::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .sub-title {
    font-size: 10px;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-secondary);
    margin: 16px 0 8px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 6px 0;
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

  .subtle-button {
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

  @media (max-width: 640px) {
    .grid { grid-template-columns: 1fr; }
  }
</style>
