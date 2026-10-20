/**
 * Preserva el punto bajo el cursor al cambiar el zoom.
 *
 * 1. Antes del cambio: `captureAnchor()` guarda la posición del cursor
 *    relativa a una página concreta.
 * 2. Tras el re-layout: `restoreAnchor()` recoloca el scroll para que
 *    ese mismo punto relativo quede bajo el cursor original.
 */

export interface ZoomAnchor {
  page: number;
  relX: number;
  relY: number;
  oldWidth: number;
  oldHeight: number;
  viewportX: number;
  viewportY: number;
}

export function captureAnchor(
  areaEl: HTMLElement,
  clientX: number,
  clientY: number,
): ZoomAnchor | null {
  const areaRect = areaEl.getBoundingClientRect();
  if (
    clientX < areaRect.left ||
    clientX > areaRect.right ||
    clientY < areaRect.top ||
    clientY > areaRect.bottom
  ) return null;

  const el = document.elementFromPoint(clientX, clientY);
  let slot = el?.closest(".page-slot") as HTMLElement | null;
  if (slot && !areaEl.contains(slot)) slot = null;

  if (!slot) {
    let nearestDistance = Number.POSITIVE_INFINITY;
    for (const candidate of areaEl.querySelectorAll<HTMLElement>(".page-slot[data-page]")) {
      const rect = candidate.getBoundingClientRect();
      if (
        rect.width <= 0 ||
        rect.height <= 0 ||
        rect.right < areaRect.left ||
        rect.left > areaRect.right ||
        rect.bottom < areaRect.top ||
        rect.top > areaRect.bottom
      ) continue;

      const dx = Math.max(rect.left - clientX, 0, clientX - rect.right);
      const dy = Math.max(rect.top - clientY, 0, clientY - rect.bottom);
      const distance = dx * dx + dy * dy;
      if (distance < nearestDistance) {
        nearestDistance = distance;
        slot = candidate;
      }
    }
  }
  if (!slot) return null;

  const page = Number(slot.dataset.page);
  if (!Number.isFinite(page)) return null;

  const slotRect = slot.getBoundingClientRect();

  if (slotRect.width <= 0 || slotRect.height <= 0) return null;

  return {
    page,
    relX: clientX - slotRect.left,
    relY: clientY - slotRect.top,
    oldWidth: slotRect.width,
    oldHeight: slotRect.height,
    viewportX: clientX - areaRect.left,
    viewportY: clientY - areaRect.top,
  };
}

export function restoreAnchor(areaEl: HTMLElement, anchor: ZoomAnchor): boolean {
  const slot = areaEl.querySelector<HTMLElement>(
    `.page-slot[data-page="${anchor.page}"]`,
  );
  if (!slot) return false;

  const slotRect = slot.getBoundingClientRect();
  if (
    slotRect.width <= 0 ||
    slotRect.height <= 0 ||
    anchor.oldWidth <= 0 ||
    anchor.oldHeight <= 0
  ) return false;

  const scaleX = slotRect.width / anchor.oldWidth;
  const scaleY = slotRect.height / anchor.oldHeight;
  const areaRect = areaEl.getBoundingClientRect();
  const pointX = slotRect.left - areaRect.left + anchor.relX * scaleX;
  const pointY = slotRect.top - areaRect.top + anchor.relY * scaleY;
  const maxLeft = Math.max(0, areaEl.scrollWidth - areaEl.clientWidth);
  const maxTop = Math.max(0, areaEl.scrollHeight - areaEl.clientHeight);
  const targetLeft = Math.min(
    maxLeft,
    Math.max(0, areaEl.scrollLeft + pointX - anchor.viewportX),
  );
  const targetTop = Math.min(
    maxTop,
    Math.max(0, areaEl.scrollTop + pointY - anchor.viewportY),
  );
  const previousScrollBehavior = areaEl.style.getPropertyValue("scroll-behavior");
  const previousScrollPriority = areaEl.style.getPropertyPriority("scroll-behavior");

  try {
    areaEl.style.setProperty("scroll-behavior", "auto", "important");
    if (Math.abs(areaEl.scrollLeft - targetLeft) > 0.05) areaEl.scrollLeft = targetLeft;
    if (Math.abs(areaEl.scrollTop - targetTop) > 0.05) areaEl.scrollTop = targetTop;
  } finally {
    if (previousScrollBehavior) {
      areaEl.style.setProperty("scroll-behavior", previousScrollBehavior, previousScrollPriority);
    } else {
      areaEl.style.removeProperty("scroll-behavior");
    }
  }

  return true;
}

/**
 * Igual que `captureAnchor` pero tomando el centro del área visible
 * (para zooms disparados por teclado o botones, sin cursor).
 */
export function captureAnchorCenter(areaEl: HTMLElement): ZoomAnchor | null {
  const rect = areaEl.getBoundingClientRect();
  const styles = getComputedStyle(areaEl);
  const paddingLeft = Number.parseFloat(styles.paddingLeft) || 0;
  const paddingRight = Number.parseFloat(styles.paddingRight) || 0;
  const paddingTop = Number.parseFloat(styles.paddingTop) || 0;
  const paddingBottom = Number.parseFloat(styles.paddingBottom) || 0;
  const clientX = rect.left + paddingLeft +
    (areaEl.clientWidth - paddingLeft - paddingRight) / 2;
  const clientY = rect.top + paddingTop +
    (areaEl.clientHeight - paddingTop - paddingBottom) / 2;
  const areaRight = rect.right;
  const areaBottom = rect.bottom;

  let nearestSlot: HTMLElement | null = null;
  let nearestDistance = Number.POSITIVE_INFINITY;
  for (const slot of areaEl.querySelectorAll<HTMLElement>(".page-slot[data-page]")) {
    const slotRect = slot.getBoundingClientRect();
    if (
      slotRect.width <= 0 ||
      slotRect.height <= 0 ||
      slotRect.right < rect.left ||
      slotRect.left > areaRight ||
      slotRect.bottom < rect.top ||
      slotRect.top > areaBottom
    ) continue;

    const dx = Math.max(slotRect.left - clientX, 0, clientX - slotRect.right);
    const dy = Math.max(slotRect.top - clientY, 0, clientY - slotRect.bottom);
    const distance = dx * dx + dy * dy;
    if (distance < nearestDistance) {
      nearestDistance = distance;
      nearestSlot = slot;
    }
  }

  if (!nearestSlot) return captureAnchor(areaEl, clientX, clientY);

  const slotRect = nearestSlot.getBoundingClientRect();
  const anchorX = Math.min(slotRect.right, Math.max(slotRect.left, clientX));
  const anchorY = Math.min(slotRect.bottom, Math.max(slotRect.top, clientY));
  const page = Number(nearestSlot.dataset.page);
  if (!Number.isFinite(page)) return null;

  return {
    page,
    relX: anchorX - slotRect.left,
    relY: anchorY - slotRect.top,
    oldWidth: slotRect.width,
    oldHeight: slotRect.height,
    viewportX: clientX - rect.left,
    viewportY: clientY - rect.top,
  };
}
