import { beforeEach, describe, expect, it } from "vitest";
import { setInvokeMock } from "../../test/setup";
import {
  childrenOf,
  findFolder,
  folderChildCount,
  folderContaining,
  isDocInLibrary,
  library,
  loadFolderChildren,
  mergeRootSnapshot,
  reloadLibrary,
} from "./library.svelte";
import type { FolderRecord, LibrarySnapshot } from "$lib/types";

function folder(
  id: string,
  parentId: string | null,
  position = 0,
  expanded = false,
): FolderRecord {
  return { id, parentId, name: id, kind: "synced", path: null, expanded, position, createdAt: 0 };
}

function snapshot(
  folders: FolderRecord[],
  rootDocIds: string[] = [],
  folderDocs: Record<string, string[]> = {},
  folderChildCounts: Record<string, number> = {},
): LibrarySnapshot {
  return { folders, rootDocIds, folderDocs, folderChildCounts };
}

describe("biblioteca con árbol bajo demanda", () => {
  let childrenByFolder: Record<string, FolderRecord[]>;
  let requested: string[];

  beforeEach(() => {
    library.folders = [];
    library.rootDocIds = [];
    library.folderDocs = {};
    childrenByFolder = {};
    requested = [];

    setInvokeMock("list_library_root", () =>
      snapshot([folder("root", null, 0, true)], ["root-doc"], { deep: ["deep-doc"] }));
    setInvokeMock("list_folder_children", (args) => {
      const { folderId } = args as { folderId: string };
      requested.push(folderId);
      return { folders: childrenByFolder[folderId] ?? [] };
    });
  });

  it("mantiene completo el índice docId → carpeta con un snapshot de raíces", async () => {    mergeRootSnapshot(
      snapshot([folder("root", null)], ["root-doc"], { deep: ["deep-doc"] }),
    );

    // La carpeta `deep` no está cargada, pero el documento sí se localiza.
    expect(findFolder("deep")).toBeNull();
    expect(folderContaining("deep-doc")).toBe("deep");
    expect(isDocInLibrary("deep-doc")).toBe(true);
    expect(folderContaining("root-doc")).toBeNull();
  });

  it("no da por cargada una carpeta hasta que se piden sus hijos", async () => {
    mergeRootSnapshot(snapshot([folder("root", null)]));
    expect(childrenOf("root")).toEqual({ folders: [], docs: [], loaded: false });

    childrenByFolder.root = [folder("sub", "root")];
    await loadFolderChildren("root");

    expect(childrenOf("root")).toEqual({ folders: ["sub"], docs: [], loaded: true });
  });

  it("pide los hijos una sola vez aunque se pida dos veces a la vez", async () => {
    mergeRootSnapshot(snapshot([folder("root", null)]));
    childrenByFolder.root = [folder("sub", "root")];

    await Promise.all([loadFolderChildren("root"), loadFolderChildren("root")]);

    expect(requested).toEqual(["root"]);
  });

  it("un snapshot de raíces conserva las ramas ya cargadas", async () => {
    mergeRootSnapshot(snapshot([folder("root", null)]));
    childrenByFolder.root = [folder("sub", "root")];
    await loadFolderChildren("root");

    mergeRootSnapshot(snapshot([folder("root", null)]));

    expect(childrenOf("root").folders).toEqual(["sub"]);
    expect(childrenOf("root").loaded).toBe(true);
  });

  it("el contador de una carpeta no depende de tener sus hijos cargados", () => {
    mergeRootSnapshot(
      snapshot([folder("root", null)], [], { root: ["doc-a"] }, { root: 12 }),
    );

    // 12 subcarpetas sin cargar + 1 documento ya conocido.
    expect(folderChildCount("root")).toBe(13);
    expect(childrenOf("root").folders).toEqual([]);
  });

  it("descarta ramas que ya no cuelgan de ninguna raíz", async () => {
    mergeRootSnapshot(snapshot([folder("root", null)]));
    library.folders = [...library.folders, folder("huérfana", "raíz-inexistente")];

    mergeRootSnapshot(snapshot([folder("root", null)]));

    expect(findFolder("huérfana")).toBeNull();
  });

  it("reloadLibrary refresca las raíces y las carpetas expandidas", async () => {
    mergeRootSnapshot(snapshot([folder("root", null, 0, true)]));
    childrenByFolder.root = [folder("sub", "root", 0, true)];
    await loadFolderChildren("root");
    childrenByFolder.sub = [folder("nieta", "sub")];
    await loadFolderChildren("sub");
    requested = [];

    await reloadLibrary();

    // Solo la raíz y `sub` están expandidas: `nieta` se pedirá al expandirla.
    expect(requested).toEqual(["root", "sub"]);
  });

  // El árbol se pintaba con `folders.find`/`filter` por fila; ahora los hijos y
  // las búsquedas por id salen de un índice derivado, así que el orden por
  // posición tiene que seguir saliendo de ahí.
  it("sirve los hijos ordenados por posición desde el índice", async () => {
    mergeRootSnapshot(
      snapshot([
        folder("z", null, 2),
        folder("a", null, 0),
        folder("m", null, 1),
        folder("nieta", "z", 1),
        folder("nieta-antes", "z", 0),
      ]),
    );

    expect(childrenOf(null).folders).toEqual(["a", "m", "z"]);
    expect(childrenOf("z").folders).toEqual(["nieta-antes", "nieta"]);
    expect(childrenOf("inexistente").folders).toEqual([]);
  });
});
