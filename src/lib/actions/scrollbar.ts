/**
 * Sustituye a initCustomScrollbars() del prototipo.
 * Crea dos thumbs (vertical/horizontal) dentro del parent del nodo,
 * con márgenes calculados a partir del border-radius del contenedor.
 *
 * Uso:
 *   <div class="custom-scroll" use:scrollbar>...</div>
 *
 * Requisito: el parent debe tener (o se le asigna) position: relative.
 */

interface ScrollbarOptions {
  thin?: boolean;
  /** Mantiene el thumb visible sin pasar el cursor (`settings.scrollbarMode`). */
  always?: boolean;
}

export function scrollbar(node: HTMLElement, opts: ScrollbarOptions = {}) {
  const parent = node.parentElement;
  if (!parent) return {};

  if (getComputedStyle(parent).position === "static") {
    parent.style.position = "relative";
  }

  const vThumb = document.createElement("div");
  vThumb.className = "custom-scrollbar-thumb";
  parent.appendChild(vThumb);

  const hThumb = document.createElement("div");
  hThumb.className = "custom-scrollbar-thumb horizontal";
  parent.appendChild(hThumb);

  let active = false;
  let hovering = false;
  let dragging = false;
  let draggingH = false;
  let vHover = false;
  let hHover = false;
  let hideTimer = 0;
  let raf = 0;
  let activeDragCleanup: (() => void) | null = null;

  const state = {
    thin: !!opts.thin,
    always: !!opts.always,
  };

  /**
   * Margen del thumb respecto a la esquina que le toca. El thumb vertical va
   * pegado al borde derecho y el horizontal al inferior, así que se mira el
   * radio de esa esquina (y se usa el mayor de los dos radios del lado para no
   * ignorar nunca un radio que recortaría el thumb).
   */
  function margin(corners: [string, string]): number {
    const ps = getComputedStyle(parent!);
    const radius = Math.max(
      parseFloat(ps.getPropertyValue(corners[0])) || 0,
      parseFloat(ps.getPropertyValue(corners[1])) || 0,
    );
    const base = state.thin ? 6 : 8;
    return Math.max(base, Math.round(radius * 0.55));
  }

  function update() {
    if (!parent) return;
    const scrollableV = node.scrollHeight > node.clientHeight + 2;
    const scrollableH = node.scrollWidth > node.clientWidth + 2;

    // Vertical
    if (!scrollableV || node.clientHeight <= 0) {
      vThumb.classList.remove("visible");
      vThumb.style.display = "none";
    } else {
      vThumb.style.display = "";
      const size = state.thin ? 8 : 12;
      // El thumb vertical va pegado al borde derecho: su separación superior la
      // marcan las esquinas de arriba y su separación horizontal las de la
      // **derecha** (usar las de arriba mezclaba una esquina izquierda con una
      // derecha, que solo coinciden cuando el radio es simétrico).
      const mTop = margin(["border-top-right-radius", "border-top-left-radius"]);
      const mRight = margin(["border-top-right-radius", "border-bottom-right-radius"]);
      const mBottom = margin(["border-bottom-right-radius", "border-bottom-left-radius"]);
      const totalM = mTop + mBottom;
      const thumbH = Math.max(state.thin ? 20 : 34, (node.clientHeight / node.scrollHeight) * node.clientHeight);
      const maxTop = Math.max(0, node.clientHeight - thumbH - totalM);
      const top = node.scrollTop / Math.max(1, node.scrollHeight - node.clientHeight) * maxTop;
      const parentRect = parent.getBoundingClientRect();
      const nodeRect = node.getBoundingClientRect();
      const offsetLeft = nodeRect.left - parentRect.left;
      const offsetTop = nodeRect.top - parentRect.top;

      vThumb.style.width = `${size}px`;
      vThumb.style.height = `${thumbH}px`;
      vThumb.style.left = `${offsetLeft + node.clientWidth - size - mRight}px`;
      vThumb.style.top = `${offsetTop + mTop}px`;
      vThumb.style.transform = `translateY(${top}px)`;

      const visible = state.always || hovering || active || vHover || dragging;
      vThumb.classList.toggle("visible", visible);
    }

    // Horizontal
    if (!scrollableH || node.clientWidth <= 0) {
      hThumb.classList.remove("visible");
      hThumb.style.display = "none";
    } else {
      hThumb.style.display = "";
      const size = 12;
      const mLeft = margin(["border-bottom-left-radius", "border-top-left-radius"]);
      // El thumb horizontal va pegado al borde inferior: su separación vertical la
      // marcan las esquinas de abajo (antes usaba las de la derecha).
      const mBottom = margin(["border-bottom-right-radius", "border-bottom-left-radius"]);
      const mRight = margin(["border-bottom-right-radius", "border-top-right-radius"]);
      const totalM = mLeft + mRight;
      const thumbW = Math.max(34, (node.clientWidth / node.scrollWidth) * node.clientWidth);
      const maxLeft = Math.max(0, node.clientWidth - thumbW - totalM);
      const left = node.scrollLeft / Math.max(1, node.scrollWidth - node.clientWidth) * maxLeft;
      const parentRect = parent.getBoundingClientRect();
      const nodeRect = node.getBoundingClientRect();
      const offsetLeft = nodeRect.left - parentRect.left;
      const offsetTop = nodeRect.top - parentRect.top;

      hThumb.style.height = `${size}px`;
      hThumb.style.width = `${thumbW}px`;
      hThumb.style.left = `${offsetLeft + mLeft}px`;
      hThumb.style.top = `${offsetTop + node.clientHeight - size - mBottom}px`;
      hThumb.style.transform = `translateX(${left}px)`;

      const visible = state.always || hovering || active || hHover || draggingH;
      hThumb.classList.toggle("visible", visible);
    }
  }

  function scheduleUpdate() {
    if (raf) return;
    raf = requestAnimationFrame(() => {
      raf = 0;
      update();
    });
  }

  function show() {
    active = true;
    update();
    clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => {
      if (!hovering && !vHover && !hHover && !dragging && !draggingH) {
        active = false;
        update();
      }
    }, 250);
  }

  function onScroll() {
    show();
  }

  function onMouseMove(e: MouseEvent) {
    const rect = node.getBoundingClientRect();
    const nearRight = e.clientX >= rect.right - 26;
    const nearBottom = e.clientY >= rect.bottom - 22;
    if (nearRight || nearBottom || dragging || draggingH || vHover || hHover) {
      hovering = true;
      show();
    } else {
      hovering = false;
      clearTimeout(hideTimer);
      hideTimer = window.setTimeout(() => {
        if (!hovering && !vHover && !hHover && !dragging && !draggingH) {
          active = false;
          update();
        }
      }, 180);
    }
  }

  function onMouseLeave() {
    hovering = false;
    clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => {
      if (!vHover && !hHover && !dragging && !draggingH) {
        active = false;
        update();
      }
    }, 140);
  }

  function onVEnter() { vHover = true; update(); }
  function onVLeave() { vHover = false; scheduleUpdate(); }
  function onHEnter() { hHover = true; update(); }
  function onHLeave() { hHover = false; scheduleUpdate(); }

  function onVPointerDown(e: PointerEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragging = true;
    update();
    const startY = e.clientY;
    const startTop = node.scrollTop;
    const thumbH = vThumb.offsetHeight || 34;
    const track = Math.max(1, node.clientHeight - thumbH);
    const dist = Math.max(1, node.scrollHeight - node.clientHeight);

    const move = (ev: PointerEvent) => {
      node.scrollTop = startTop + ((ev.clientY - startY) / track) * dist;
    };
    const up = () => {
      dragging = false;
      scheduleUpdate();
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      activeDragCleanup = null;
    };
    activeDragCleanup = up;
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  }

  function onHPointerDown(e: PointerEvent) {
    e.preventDefault();
    e.stopPropagation();
    draggingH = true;
    update();
    const startX = e.clientX;
    const startLeft = node.scrollLeft;
    const thumbW = hThumb.offsetWidth || 40;
    const track = Math.max(1, node.clientWidth - thumbW);
    const dist = Math.max(1, node.scrollWidth - node.clientWidth);

    const move = (ev: PointerEvent) => {
      node.scrollLeft = startLeft + ((ev.clientX - startX) / track) * dist;
    };
    const up = () => {
      draggingH = false;
      scheduleUpdate();
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      activeDragCleanup = null;
    };
    activeDragCleanup = up;
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  }

  node.addEventListener("scroll", onScroll, { passive: true });
  node.addEventListener("mousemove", onMouseMove);
  node.addEventListener("mouseleave", onMouseLeave);
  vThumb.addEventListener("mouseenter", onVEnter);
  vThumb.addEventListener("mouseleave", onVLeave);
  hThumb.addEventListener("mouseenter", onHEnter);
  hThumb.addEventListener("mouseleave", onHLeave);
  vThumb.addEventListener("pointerdown", onVPointerDown);
  hThumb.addEventListener("pointerdown", onHPointerDown);

  const ro = new ResizeObserver(scheduleUpdate);
  ro.observe(node);
  if (parent !== node) ro.observe(parent);

  // El tamaño del thumb depende de `scrollHeight / clientHeight`, y el contenido
  // puede crecer sin que cambie la caja del contenedor (el sidebar suma filas tras
  // un sync, el inspector carga comentarios): el `ResizeObserver` no se entera y el
  // thumb se quedaba con el tamaño viejo hasta el siguiente scroll.
  const mo = new MutationObserver(scheduleUpdate);
  mo.observe(node, { childList: true, subtree: true });

  // Sincroniza con cambios de tamaño de la ventana
  window.addEventListener("resize", scheduleUpdate);

  // Estado inicial
  requestAnimationFrame(update);

  return {
    update(newOpts: ScrollbarOptions) {
      state.thin = !!newOpts.thin;
      state.always = !!newOpts.always;
      if (state.thin) node.setAttribute("data-thin-scroll", "");
      else node.removeAttribute("data-thin-scroll");
      scheduleUpdate();
    },
    destroy() {
      activeDragCleanup?.();
      clearTimeout(hideTimer);
      cancelAnimationFrame(raf);
      ro.disconnect();
      mo.disconnect();
      window.removeEventListener("resize", scheduleUpdate);
      node.removeEventListener("scroll", onScroll);
      node.removeEventListener("mousemove", onMouseMove);
      node.removeEventListener("mouseleave", onMouseLeave);
      vThumb.remove();
      hThumb.remove();
    },
  };
}
