import { describe, expect, it } from "vitest";
import { normalizeCopiedText } from "./copySelection";

describe("normalizeCopiedText", () => {
  it("une las líneas de una misma frase con un espacio", () => {
    expect(normalizeCopiedText("Hola que\ntal", false)).toBe("Hola que tal");
  });

  it("conserva el salto cuando la frase anterior cierra", () => {
    expect(normalizeCopiedText("Primera frase.\nSegunda frase.", false)).toBe(
      "Primera frase.\nSegunda frase.",
    );
  });

  it("conserva el salto cuando la siguiente línea empieza en mayúscula", () => {
    expect(normalizeCopiedText("el final\nDe nuevo", false)).toBe("el final\nDe nuevo");
  });

  it("une la separación silábica sin espacio", () => {
    expect(normalizeCopiedText("perro-\ngato", false)).toBe("perrogato");
  });

  it("convierte una línea vacía en salto de párrafo", () => {
    expect(normalizeCopiedText("párrafo uno\n\npárrafo dos", false)).toBe(
      "párrafo uno\n\npárrafo dos",
    );
  });

  it("ignora la marca de agua solo si se pide", () => {
    const raw = ["CONFIDENCIAL", "Texto real", "CONFIDENCIAL"].join("\n");
    expect(normalizeCopiedText(raw, true)).toBe("Texto real");
    // Sin la opción, se conserva todo (con el salto de la unión de párrafos).
    expect(normalizeCopiedText(raw, false)).toBe(raw);
  });

  it("no toca encabezados numerados", () => {
    const raw = ["Página 1", "Texto real", "Página 2"].join("\n");
    expect(normalizeCopiedText(raw, true)).toBe(raw);
  });
});
