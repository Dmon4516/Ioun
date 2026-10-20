import { beforeEach, describe, expect, it } from "vitest";
import { focusTrap } from "./focusTrap";

/** Acción aplicada a mano sobre un nodo real del DOM. */
function mount(node: HTMLElement): () => void {
  const action = focusTrap(node) as { destroy?: () => void };
  return () => action.destroy?.();
}

function modal(content = ""): HTMLDivElement {
  const element = document.createElement("div");
  element.setAttribute("aria-modal", "true");
  element.setAttribute("role", "dialog");
  element.tabIndex = -1;
  element.innerHTML = content;
  document.body.appendChild(element);
  return element;
}

/** Vacía el `body` entre pruebas sin romper las referencias del test. */
beforeEach(() => {
  document.body.innerHTML = "";
});

describe("focusTrap", () => {
  it("devuelve el foco al elemento anterior al desmontarse", async () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();
    expect(document.activeElement).toBe(trigger);

    const dialog = modal();
    const destroy = mount(dialog);
    // El foco queda huérfano (en el propio nodo) al desmontar.
    dialog.focus();
    expect(document.activeElement).toBe(dialog);
    destroy();

    await Promise.resolve();
    await Promise.resolve();

    expect(document.activeElement).toBe(trigger);
  });

  it("no le roba el foco a un diálogo que se monta en el mismo fotograma", async () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();

    const closing = modal();
    const destroy = mount(closing);
    closing.focus();

    // Otro diálogo se abre antes de que se desmonte el primero.
    const opening = modal();
    destroy();

    await Promise.resolve();
    await Promise.resolve();

    expect(document.activeElement).not.toBe(trigger);
    expect(opening.isConnected).toBe(true);
  });

  it("no restaura el foco si el usuario lo puso en otro sitio", async () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();

    const dialog = modal();
    const destroy = mount(dialog);

    // El usuario hizo clic en otra parte mientras el diálogo estaba abierto.
    const elsewhere = document.createElement("button");
    document.body.appendChild(elsewhere);
    elsewhere.focus();
    destroy();

    await Promise.resolve();
    await Promise.resolve();

    expect(document.activeElement).toBe(elsewhere);
  });
});
