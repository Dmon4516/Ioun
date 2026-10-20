import { describe, expect, it } from "vitest";
import { spreadStartPage } from "./documentView";

describe("spreadStartPage", () => {
  it("keeps the active page inside its horizontal set", () => {
    expect(spreadStartPage(1, 10, 2)).toBe(1);
    expect(spreadStartPage(2, 10, 2)).toBe(1);
    expect(spreadStartPage(3, 10, 2)).toBe(3);
    expect(spreadStartPage(4, 10, 2)).toBe(3);
  });

  it("anchors the final incomplete set without moving before the active page", () => {
    expect(spreadStartPage(10, 10, 3)).toBe(8);
    expect(spreadStartPage(9, 10, 3)).toBe(7);
  });
});
