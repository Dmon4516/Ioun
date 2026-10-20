import { tick } from "svelte";

function getFocusable(node: HTMLElement): HTMLElement[] {
  return Array.from(
    node.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), ' +
      'select:not([disabled]), textarea:not([disabled]), ' +
      '[tabindex]:not([tabindex="-1"])',
    ),
  ).filter(
    (el) =>
      // `offsetParent` es null para elementos `position: fixed`, así que se
      // complementa con las cajas reales del elemento.
      el.offsetParent !== null ||
      el.getClientRects().length > 0 ||
      el === document.activeElement,
  );
}

/**
 * ¿Hay algún diálogo modal abierto distinto de este nodo? Es la condición que
 * evita que un trap que se desmonta le robe el foco al que se acaba de montar.
 */
function hasAnotherModal(node: HTMLElement): boolean {
  return Array.from(document.querySelectorAll('[aria-modal="true"]')).some(
    (dialog) => dialog !== node && dialog.isConnected,
  );
}

/**
 * Atrapa el foco dentro del nodo. Al desmontar, restaura el foco al elemento
 * previo solo si el foco quedó huérfano (en `body`, en el propio nodo o dentro
 * de él) o si otro componente ya tomó el control.
 *
 * Ojo: un clic fuera sobre algo **no enfocable** deja el foco en `body`, así que
 * ese caso también restaura; lo que no restaura es un clic fuera sobre un
 * elemento enfocable (ahí manda el usuario) ni que otro diálogo haya tomado el
 * foco.
 *
 * Ver TECH_DEBT.md: el criterio de "gesto reciente" es heurístico y puede
 * fallar en casos límite. Un rediseño posterior puede simplificarlo a "nunca
 * restaurar".
 */
export function focusTrap(node: HTMLElement) {
  const previouslyFocused =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    const focusable = getFocusable(node);
    if (focusable.length === 0) {
      e.preventDefault();
      return;
    }
    const first = focusable[0]!;
    const last = focusable[focusable.length - 1]!;
    const active = document.activeElement;
    if (e.shiftKey && active === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }

  // `tick()` espera al flush de Svelte en curso (un microtask) en lugar de a un
  // fotograma completo: en menús que se abren y cierran mucho, un rAF por montaje
  // y otro por desmontaje es espera de más.
  void tick().then(() => {
    const focusable = getFocusable(node);
    if (focusable.length > 0) {
      focusable[0]!.focus();
    } else if (node.tabIndex >= 0) {
      node.focus();
    }
  });

  node.addEventListener("keydown", onKeydown);

  return {
    destroy() {
      node.removeEventListener("keydown", onKeydown);
      void tick().then(() => {
        // Si otro diálogo modal está abierto, el foco es suyo: restaurarlo aquí
        // se lo quitaría al que acaba de abrirse en el mismo fotograma.
        if (hasAnotherModal(node)) return;

        // Solo restaurar si el foco quedó huérfano.
        const active = document.activeElement;
        const abandoned =
          active === document.body ||
          active === null ||
          active === node ||
          node.contains(active);
        if (!abandoned) return;

        if (
          previouslyFocused &&
          document.contains(previouslyFocused) &&
          typeof previouslyFocused.focus === "function"
        ) {
          previouslyFocused.focus();
        }
      });
    },
  };
}
