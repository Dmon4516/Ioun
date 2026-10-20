import { describe, expect, it } from "vitest";
import { displayTitle, formatBytes, pathSummary } from "./format";

describe("pathSummary", () => {
  it("devuelve cadena vacía si el archivo no tiene carpeta", () => {
    expect(pathSummary("file.pdf", 2)).toBe("");
  });
  it("recorta a la profundidad pedida", () => {
    expect(pathSummary("/a/b/c/d/file.pdf", 2)).toBe("… / c / d");
  });
  it("muestra la ruta completa si cabe en la profundidad", () => {
    expect(pathSummary("/home/user/file.pdf", 2)).toBe("home / user");
  });
  it("muestra todo si depth es 0", () => {
    expect(pathSummary("/a/b/file.pdf", 0)).toBe("a / b");
  });
});

describe("displayTitle", () => {
  it("quita la extensión si showExt es false", () => {
    expect(displayTitle("doc.pdf", false)).toBe("doc");
  });
  it("la mantiene si showExt es true", () => {
    expect(displayTitle("doc.pdf", true)).toBe("doc.pdf");
  });
});

describe("formatBytes", () => {
  it("formatea bytes, KB, MB y GB", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(2 * 1024 * 1024)).toBe("2.0 MB");
    expect(formatBytes(3 * 1024 * 1024 * 1024)).toBe("3.00 GB");
  });
  it("devuelve guion para null", () => {
    expect(formatBytes(null)).toBe("—");
  });
});
