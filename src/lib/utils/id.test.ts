import { describe, expect, it } from "vitest";
import { uid } from "./id";

describe("uid", () => {
  it("genera IDs con prefijo y 32 hex chars", () => {
    const id = uid("doc");
    expect(id).toMatch(/^doc-[0-9a-f]{32}$/);
  });

  it("no colisiona en 10.000 generaciones", () => {
    const seen = new Set<string>();
    for (let i = 0; i < 10_000; i++) {
      const id = uid("x");
      expect(seen.has(id)).toBe(false);
      seen.add(id);
    }
  });
});
