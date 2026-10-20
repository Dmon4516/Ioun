import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";
import TrashPanel from "./TrashPanel.svelte";
import { ui } from "$lib/stores/ui.svelte";
import { trash } from "$lib/stores/trash.svelte";
import { setInvokeMock } from "../../test/setup";

interface TrashItemLike {
  id: string;
  kind: string;
  label: string;
  createdAt: number;
}

let listTrashItems: TrashItemLike[] = [];
const restoreCalls: string[] = [];

setInvokeMock("list_trash", () => listTrashItems);
setInvokeMock("restore_from_trash", (args) => {
  const { id } = args as { id: string };
  restoreCalls.push(id);
  listTrashItems = listTrashItems.filter((item) => item.id !== id);
});
setInvokeMock("empty_trash", () => {
  listTrashItems = [];
});

beforeEach(() => {
  trash.items = [];
  restoreCalls.length = 0;
  listTrashItems = [];
  ui.trashOpen = true;
});

afterEach(() => {
  ui.trashOpen = false;
  trash.items = [];
});

describe("TrashPanel", () => {
  it("lista lo que hay en la papelera con su tipo", async () => {
    listTrashItems = [
      { id: "t-1", kind: "folder", label: 'Eliminar carpeta «libro»', createdAt: 0 },
      { id: "t-2", kind: "document", label: "Quitar «manual»", createdAt: 0 },
    ];

    const { findByText, getByText } = render(TrashPanel);

    expect(await findByText("Eliminar carpeta «libro»")).toBeTruthy();
    expect(getByText("Quitar «manual»")).toBeTruthy();
    expect(getByText("Carpeta")).toBeTruthy();
    expect(getByText("2 elementos")).toBeTruthy();
  });

  it("restaura un elemento y refresca la lista", async () => {
    listTrashItems = [{ id: "t-1", kind: "document", label: "Quitar «manual»", createdAt: 0 }];

    const { findByText, getByTitle, queryByText } = render(TrashPanel);
    await findByText("Quitar «manual»");

    await fireEvent.click(getByTitle("Restaurar"));

    await waitFor(() => expect(restoreCalls).toEqual(["t-1"]));
    await waitFor(() => expect(queryByText("Quitar «manual»")).toBeNull());
    expect(await findByText(/No hay nada en la papelera/)).toBeTruthy();
  });

  it("no pide la lista si el panel está cerrado", async () => {
    const { container } = render(TrashPanel);
    ui.trashOpen = false;
    await waitFor(() => expect(container.querySelector(".overlay")).toBeNull());
  });

  it("desactiva «Vaciar papelera» cuando no hay nada", async () => {
    const { findByText } = render(TrashPanel);
    const button = (await findByText("Vaciar papelera")) as HTMLButtonElement;
    await waitFor(() => expect(button.disabled).toBe(true));
  });
});

// El mock de `confirm` del plugin de diálogo se registra aquí para que el módulo
// de diálogos no intente hablar con Tauri al vaciar la papelera.
vi.mock("$lib/tauri/dialogs", () => ({
  confirmDialog: vi.fn(async () => true),
}));
