import { describe, expect, it } from "vitest";
import { validateSettings } from "./settingsValidation";
import { DEFAULT_SETTINGS } from "./settings.svelte";

describe("validateSettings", () => {
  it("devuelve los defaults para entradas vacías o no-objeto", () => {
    expect(validateSettings(null)).toEqual(DEFAULT_SETTINGS);
    expect(validateSettings("foo")).toEqual(DEFAULT_SETTINGS);
    expect(validateSettings(42)).toEqual(DEFAULT_SETTINGS);
  });

  it("respeta valores válidos", () => {
    const result = validateSettings({ ...DEFAULT_SETTINGS, ramLimitMB: 4096 });
    expect(result.ramLimitMB).toBe(4096);
  });

  it("sustituye valores inválidos por los defaults", () => {
    const result = validateSettings({
      ramLimitMB: "foo",
      pagesPerRow: 99,
      themeMode: "rainbow",
    });
    expect(result.ramLimitMB).toBe(DEFAULT_SETTINGS.ramLimitMB);
    expect(result.pagesPerRow).toBe(4);
    expect(result.themeMode).toBe(DEFAULT_SETTINGS.themeMode);
  });

  it("descarta colores no-hex", () => {
    const result = validateSettings({
      colors: { dark: { accentColor: "red" }, light: {} },
    });
    expect(result.colors.dark.accentColor).toBeUndefined();
  });

  it("conserva colores hex", () => {
    const result = validateSettings({
      colors: { dark: { accentColor: "#ff00ff" }, light: {} },
    });
    expect(result.colors.dark.accentColor).toBe("#ff00ff");
    // Los tres formatos que admite CSS, además del de seis dígitos.
    for (const color of ["#f0f", "#f0fa", "#ff00ffff"]) {
      const result = validateSettings({ colors: { dark: { accentColor: color }, light: {} } });
      expect(result.colors.dark.accentColor).toBe(color);
    }
  });

  it("conserva el color de selección", () => {
    const result = validateSettings({
      colors: { dark: { selectionColor: "#123456" }, light: {} },
    });
    expect(result.colors.dark.selectionColor).toBe("#123456");
  });

  // CSS solo define hex de 3, 4, 6 y 8 dígitos: `#abcde` pasaba la validación,
  // el navegador lo ignoraba y el usuario veía que su color no se aplicaba.
  it("descarta hex que CSS no admite", () => {
    for (const color of ["#abcde", "#abcdefa", "#ab", "#abcdefghi"]) {
      const result = validateSettings({ colors: { dark: { accentColor: color }, light: {} } });
      expect(result.colors.dark.accentColor).toBeUndefined();
    }
  });

  it("aplica los límites", () => {
    const result = validateSettings({ ramLimitMB: 999_999, pagesPerRow: -5 });
    expect(result.ramLimitMB).toBeLessThanOrEqual(65_536);
    expect(result.pagesPerRow).toBeGreaterThanOrEqual(1);
  });
});
