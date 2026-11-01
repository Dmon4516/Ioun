import { describe, expect, it } from "vitest";
import { clamp, clampInt, hexWithAlpha, safeColor } from "./colors";

describe("safeColor", () => {
  it("acepta hex válidos", () => {
    expect(safeColor("#fff")).toBe("#fff");
    expect(safeColor("#0a84ff")).toBe("#0a84ff");
  });
  it("reemplaza valores inválidos", () => {
    expect(safeColor("red")).toBe("#888888");
    expect(safeColor("javascript:alert(1)")).toBe("#888888");
  });

  it("rechaza longitudes hexadecimales que CSS no admite", () => {
    expect(safeColor("#abcde")).toBe("#888888");
    expect(safeColor("#abcdefa")).toBe("#888888");
    expect(safeColor("#ab")).toBe("#888888");
    expect(safeColor("#abcdefghi")).toBe("#888888");
  });

  it("acepta hex de 3, 4, 6 y 8 dígitos", () => {
    for (const color of ["#fff", "#f0fa", "#ff00ff", "#ff00ffff"]) {
      expect(safeColor(color)).toBe(color);
    }
  });
});

describe("hexWithAlpha", () => {
  it("añade alpha a hex de 6", () => {
    expect(hexWithAlpha("#1a1d23", "E6")).toBe("#1a1d23E6");
  });
  it("expande hex RGB de 3 dígitos antes de añadir alpha", () => {
    expect(hexWithAlpha("#fff", "E6")).toBe("#ffffffE6");
    expect(hexWithAlpha("#1a3", "80")).toBe("#11aa33" + "80");
  });
  it("reemplaza el alpha existente en hex RGBA de 4 dígitos", () => {
    expect(hexWithAlpha("#abcd", "E6")).toBe("#aabbccE6");
  });
});

describe("clamp", () => {
  it("limita en los extremos", () => {
    expect(clamp(5, 0, 10)).toBe(5);
    expect(clamp(-1, 0, 10)).toBe(0);
    expect(clamp(11, 0, 10)).toBe(10);
  });
});

describe("clampInt", () => {
  it("trunca y limita", () => {
    expect(clampInt(5.7, 0, 10, 0)).toBe(5);
    expect(clampInt("abc", 0, 10, 7)).toBe(7);
    expect(clampInt(NaN, 0, 10, 3)).toBe(3);
  });
});
