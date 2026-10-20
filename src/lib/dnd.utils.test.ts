import { describe, expect, it } from "vitest";
import type { FolderRecord } from "./types";
import {
  computeTargetIndex,
  isDescendant,
  positionWithin,
  type DndLibrary,
} from "./dnd.utils";

function folder(id: string, parentId: string | null, position: number): FolderRecord {
  return {
    id,
    parentId,
    name: id,
    kind: "temp",
    path: null,
    expanded: true,
    position,
    createdAt: 0,
  };
}

describe("positionWithin", () => {
  it("returns the index of a root folder among siblings", () => {
    const lib: DndLibrary = {
      folders: [folder("a", null, 0), folder("b", null, 1), folder("c", null, 2)],
      rootDocIds: [],
      folderDocs: {},
    };
    expect(positionWithin(lib, null, "folder", "b")).toBe(1);
  });

  it("returns the index of a folder under a specific parent", () => {
    const lib: DndLibrary = {
      folders: [
        folder("a", "parent", 0),
        folder("b", "parent", 1),
        folder("other", "elsewhere", 0),
      ],
      rootDocIds: [],
      folderDocs: {},
    };
    expect(positionWithin(lib, "parent", "folder", "b")).toBe(1);
    expect(positionWithin(lib, "elsewhere", "folder", "b")).toBe(-1);
  });

  it("sorts folder siblings by position", () => {
    const lib: DndLibrary = {
      folders: [folder("c", null, 2), folder("a", null, 0), folder("b", null, 1)],
      rootDocIds: [],
      folderDocs: {},
    };
    expect(positionWithin(lib, null, "folder", "c")).toBe(2);
  });

  it("returns the index of a document within a folder", () => {
    const lib: DndLibrary = {
      folders: [],
      rootDocIds: [],
      folderDocs: { f1: ["d1", "d2", "d3"] },
    };
    expect(positionWithin(lib, "f1", "doc", "d2")).toBe(1);
  });

  it("returns the index of a root document", () => {
    const lib: DndLibrary = {
      folders: [],
      rootDocIds: ["r1", "r2", "r3"],
      folderDocs: {},
    };
    expect(positionWithin(lib, null, "doc", "r3")).toBe(2);
  });

  it("returns -1 when a document is not in the target folder", () => {
    const lib: DndLibrary = {
      folders: [],
      rootDocIds: [],
      folderDocs: { f1: ["d1"] },
    };
    expect(positionWithin(lib, "f1", "doc", "missing")).toBe(-1);
  });
});

describe("computeTargetIndex", () => {
  const lib: DndLibrary = {
    folders: [folder("a", null, 0), folder("b", null, 1)],
    rootDocIds: [],
    folderDocs: { f1: ["d1", "d2", "d3"] },
  };

  it("returns zero when dropping inside a folder", () => {
    expect(computeTargetIndex(lib, "folder", "f1", "folder", "inside", null)).toBe(0);
  });

  it("returns the target index when dropping before it", () => {
    expect(computeTargetIndex(lib, "doc", "d2", "doc", "before", "f1")).toBe(1);
  });

  it("returns the next index when dropping after it", () => {
    expect(computeTargetIndex(lib, "doc", "d2", "doc", "after", "f1")).toBe(2);
  });

  it("returns zero when the target is not found", () => {
    expect(computeTargetIndex(lib, "doc", "ghost", "doc", "before", "f1")).toBe(0);
    expect(computeTargetIndex(lib, "doc", "ghost", "doc", "after", "f1")).toBe(0);
  });

  it("computes folder sibling indexes", () => {
    expect(computeTargetIndex(lib, "folder", "b", "folder", "before", null)).toBe(1);
    expect(computeTargetIndex(lib, "folder", "b", "folder", "after", null)).toBe(2);
  });

  it("appends when the moved item and the target have different kinds", () => {
    // Dos carpetas raíz: una carpeta que se suelta sobre un documento va al final.
    const rootFolders = lib.folders.filter((f) => f.parentId === null).length;
    expect(computeTargetIndex(lib, "folder", "d1", "doc", "before", null)).toBe(rootFolders);
    // Un documento que se suelta sobre una carpeta usa la lista de documentos
    // de esa carpeta, no el índice de la carpeta.
    expect(computeTargetIndex(lib, "doc", "f1", "folder", "before", null)).toBe(lib.rootDocIds.length);
  });
});

describe("isDescendant", () => {
  it("detects a direct child", () => {
    expect(isDescendant([folder("a", null, 0), folder("b", "a", 0)], "b", "a")).toBe(true);
  });

  it("detects a deep descendant", () => {
    const folders = [
      folder("a", null, 0),
      folder("b", "a", 0),
      folder("c", "b", 0),
    ];
    expect(isDescendant(folders, "c", "a")).toBe(true);
  });

  it("returns false for unrelated folders", () => {
    expect(isDescendant([folder("a", null, 0), folder("b", null, 1)], "b", "a")).toBe(false);
  });

  it("returns false when the relation is inverted", () => {
    expect(isDescendant([folder("a", null, 0), folder("b", "a", 0)], "a", "b")).toBe(false);
  });

  it("returns false for a broken parent chain", () => {
    expect(isDescendant([folder("orphan", "missing-parent", 0)], "orphan", "a")).toBe(false);
  });

  it("terminates when folder parent data contains a cycle", () => {
    expect(isDescendant([folder("a", "b", 0), folder("b", "a", 1)], "a", "missing")).toBe(false);
  });
});
