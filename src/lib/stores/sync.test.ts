import { describe, expect, it } from "vitest";
import { applySyncProgress, syncStatus, syncingFolderName } from "./sync.svelte";

describe("estado del sync de carpetas", () => {
  it("queda activo mientras escanea y muestra la carpeta en curso", () => {
    applySyncProgress({ phase: "scan", path: "/Users/david/Books" });

    expect(syncStatus.active).toBe(true);
    expect(syncingFolderName()).toBe("Books");
  });

  it("se apaga al terminar", () => {
    applySyncProgress({ phase: "scan", path: "/Users/david/Books" });

    applySyncProgress({ phase: "done", changed: true });

    expect(syncStatus.active).toBe(false);
  });

  it("se apaga también si falla", () => {
    applySyncProgress({ phase: "scan", path: "/Users/david/Books" });

    applySyncProgress({ phase: "error", message: "disco lleno" });

    expect(syncStatus.active).toBe(false);
  });
});
