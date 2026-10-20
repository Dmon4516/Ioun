<script lang="ts">
  import {
    settings,
    COLOR_DEFS,
    setColorOverride,
    resetColors,
    currentEditTheme,
    hasCustomColors,
    updateSettings,
  } from "$lib/stores/settings.svelte";
  import type { ColorOverrides } from "$lib/types";
  import Icon from "$components/Icon.svelte";

  const themeMode = $derived(settings.themeMode);
  const editTheme = $derived(currentEditTheme());

  function modeIcon(mode: "light" | "dark" | "system") {
    return mode === "light" ? "sun" : mode === "dark" ? "moon" : "monitor";
  }

  const interfaceDefs = $derived(COLOR_DEFS.filter((d) => d.group === "interface"));
  const textDefs = $derived(COLOR_DEFS.filter((d) => d.group === "text"));
  const stateDefs = $derived(COLOR_DEFS.filter((d) => d.group === "state"));

  function currentValue(def: (typeof COLOR_DEFS)[number]): string {
    const custom = settings.colors[editTheme][def.key];
    if (custom) return custom;
    return editTheme === "dark" ? def.dark : def.light;
  }

  function onChange(def: (typeof COLOR_DEFS)[number], value: string): void {
    setColorOverride(editTheme, def.key as keyof ColorOverrides, value);
  }

  const showReset = $derived(hasCustomColors(editTheme));
</script>

<div class="settings-group">
  <div class="settings-group-title">Tema</div>

  <div class="theme-icons">
    {#each (["light", "dark", "system"] as const) as mode (mode)}
      <button
        class="theme-icon-button"
        class:active={themeMode === mode}
        type="button"
        onclick={() => updateSettings("themeMode", mode)}
      >
        <Icon name={modeIcon(mode)} size={22} />
        <span>{mode === "light" ? "Claro" : mode === "dark" ? "Oscuro" : "Sistema"}</span>
      </button>
    {/each}
  </div>

  <div class="sub-title">Interfaz</div>
  <div class="color-grid">
    {#each interfaceDefs as def (def.key)}
      {@const value = currentValue(def)}
      <label class="color-card">
        <span class="swatch" style:background={value}></span>
        <span class="info">
          <span class="name">{def.label}</span>
          <span class="hex">{value.toUpperCase()}</span>
        </span>
        <input
          class="color-hidden"
          type="color"
          value={value}
          onchange={(e) => onChange(def, e.currentTarget.value)}
        />
      </label>
    {/each}
  </div>

  <div class="sub-title">Texto</div>
  <div class="color-grid">
    {#each textDefs as def (def.key)}
      {@const value = currentValue(def)}
      <label class="color-card">
        <span class="swatch" style:background={value}></span>
        <span class="info">
          <span class="name">{def.label}</span>
          <span class="hex">{value.toUpperCase()}</span>
        </span>
        <input
          class="color-hidden"
          type="color"
          value={value}
          onchange={(e) => onChange(def, e.currentTarget.value)}
        />
      </label>
    {/each}
  </div>

  <div class="sub-title">Estados</div>
  <div class="color-grid">
    {#each stateDefs as def (def.key)}
      {@const value = currentValue(def)}
      <label class="color-card">
        <span class="swatch" style:background={value}></span>
        <span class="info">
          <span class="name">{def.label}</span>
          <span class="hex">{value.toUpperCase()}</span>
        </span>
        <input
          class="color-hidden"
          type="color"
          value={value}
          onchange={(e) => onChange(def, e.currentTarget.value)}
        />
      </label>
    {/each}
  </div>

  {#if showReset}
    <button class="subtle-button" type="button" onclick={() => resetColors(editTheme)}>
      Restaurar colores del tema {editTheme === "dark" ? "oscuro" : "claro"}
    </button>
  {/if}
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
  .sub-title {
    font-size: 10px;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-secondary);
    margin: 12px 0 8px;
  }

  .theme-icons {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    margin-bottom: 12px;
  }
  .theme-icon-button {
    display: grid;
    gap: 6px;
    place-items: center;
    padding: 12px 6px;
    border-radius: var(--radius-card);
    border: 1px solid var(--border);
    background: var(--bg-button);
    color: var(--text-secondary);
    font-size: 12px;
    transition: background var(--animation-fast), color var(--animation-fast);
  }
  .theme-icon-button:hover { background: var(--bg-active); color: var(--text-primary); }
  .theme-icon-button.active {
    background: var(--bg-active);
    border-color: color-mix(in srgb, var(--accent) 42%, var(--border));
    color: var(--text-primary);
  }

  .color-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .color-card {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 62px;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    padding: 10px;
    background: color-mix(in srgb, var(--bg-button) 42%, transparent);
    overflow: hidden;
    cursor: pointer;
    transition: border-color var(--animation-fast), background var(--animation-fast);
  }
  .color-card:hover {
    border-color: color-mix(in srgb, var(--accent) 32%, var(--border));
    background: var(--bg-active);
  }

  .swatch {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
    box-shadow:
      inset 0 0 0 2px color-mix(in srgb, #ffffff 12%, transparent),
      0 0 0 3px color-mix(in srgb, var(--bg-menu) 55%, transparent);
    flex: 0 0 auto;
  }

  .info { min-width: 0; display: grid; gap: 3px; }
  .name { font-size: 12px; font-weight: 680; white-space: nowrap; }
  .hex {
    font-size: 11px;
    color: var(--text-secondary);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .color-hidden {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
  }

  .subtle-button {
    width: 100%;
    margin-top: 12px;
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
