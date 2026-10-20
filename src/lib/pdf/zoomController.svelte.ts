import { tick } from "svelte";
import { getActiveTab, updateTabPage } from "$lib/stores/tabs.svelte";
import { viewer } from "$lib/stores/viewer.svelte";
import {
  captureAnchor,
  captureAnchorCenter,
  restoreAnchor,
  type ZoomAnchor,
} from "$lib/pdf/zoomAnchor";
import { clamp } from "$lib/utils/colors";
import {
  COMMAND_DURATION_MS,
  MAX_ZOOM,
  MIN_ZOOM,
  SETTLE_LOCK_MS,
  WHEEL_IDLE_MS,
} from "$lib/constants";

type ZoomState = "idle" | "wheel" | "command";
type AreaProvider = () => HTMLElement | null;

interface ZoomControllerOptions {
  area: AreaProvider;
  onSettled?: () => void;
}

const COMMAND_TIMEOUT_MS = 800;

export class ZoomController {
  #area: AreaProvider;
  #onSettled: (() => void) | undefined;
  #state: ZoomState = "idle";
  #anchor: ZoomAnchor | null = null;
  #tabId: string | null = null;
  #wheelDelta = 0;
  #wheelCursorX = 0;
  #wheelCursorY = 0;
  #wheelRaf = 0;
  #wheelIdleTimer = 0;
  #commandTimer = 0;
  #commandToken = 0;
  #wheelToken = 0;
  #wheelTickPending = false;
  #commandRaf = 0;
  #settledAt = Number.NEGATIVE_INFINITY;
  #commandStartZoom = 0;
  #commandTargetZoom = 0;
  #commandStartTime = 0;

  constructor(options: ZoomControllerOptions) {
    this.#area = options.area;
    this.#onSettled = options.onSettled;
  }

  get isBusy(): boolean {
    return this.#state !== "idle";
  }

  get isSettling(): boolean {
    return performance.now() - this.#settledAt < SETTLE_LOCK_MS;
  }

  command(targetZoom: number): void {
    const area = this.#area();
    const tab = getActiveTab();
    if (!area || !tab) return;
    if (this.#state === "wheel") this.#finishWheel(true);
    const nextZoom = clamp(targetZoom, MIN_ZOOM, MAX_ZOOM);
    if (Math.abs(nextZoom - tab.zoom) < 0.01) return;

    const continuing = this.#state === "command" &&
      this.#tabId === tab.id &&
      this.#anchor !== null;
    this.#invalidateCommand();
    const anchor = continuing ? this.#anchor : captureAnchorCenter(area);
    if (!anchor) {
      this.#anchor = null;
      this.#tabId = null;
      this.#state = "idle";
      viewer.zooming = false;
      return;
    }

    this.#state = "command";
    viewer.zooming = true;
    this.#anchor = anchor;
    this.#tabId = tab.id;
    this.#commandStartZoom = tab.zoom;
    this.#commandTargetZoom = nextZoom;
    this.#commandStartTime = performance.now();
    const token = this.#commandToken;
    this.#commandRaf = requestAnimationFrame(() => this.#animateCommand(token));
    this.#commandTimer = window.setTimeout(() => {
      if (token === this.#commandToken && this.#state === "command") {
        this.#finishCommand(true, token);
      }
    }, COMMAND_TIMEOUT_MS);
  }

  preserveLayout(): void {
    const area = this.#area();
    const tab = getActiveTab();
    if (!area || !tab) return;
    if (this.#state === "wheel") this.#finishWheel(true);
    this.#invalidateCommand();
    this.#state = "command";
    viewer.zooming = true;
    this.#anchor = captureAnchorCenter(area);
    this.#tabId = tab.id;
    this.#scheduleCommandRestore();
  }

  handleWheel(event: WheelEvent): boolean {
    if (!event.ctrlKey && !event.metaKey) return false;
    const area = this.#area();
    const tab = getActiveTab();
    if (!area || !tab || tab.status === "restoring") return false;

    let delta = event.deltaY;
    if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) delta *= 16;
    else if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) delta *= area.clientHeight;
    if (!delta && event.deltaX) delta = event.deltaX;
    if (!Number.isFinite(delta) || delta === 0) return false;

    const anchor = this.#anchor && this.#tabId === tab.id
      ? this.#anchor
      : captureAnchor(area, event.clientX, event.clientY) ?? captureAnchorCenter(area);
    if (!anchor) return false;

    if (this.#state === "command") {
      this.#invalidateCommand();
      this.#anchor = null;
      viewer.zooming = false;
    }
    this.#wheelCursorX = event.clientX;
    this.#wheelCursorY = event.clientY;

    if (this.#state !== "wheel" || this.#tabId !== tab.id) {
      if (this.#state === "wheel") this.#finishWheel(true);
      this.#state = "wheel";
      this.#anchor = anchor;
      this.#tabId = tab.id;
      viewer.zooming = true;
    }

    const sensitivity = Math.abs(delta) < 12 ? 0.0075 : 0.0018;
    this.#wheelDelta = clamp(this.#wheelDelta + delta * sensitivity, -1.2, 1.2);
    clearTimeout(this.#wheelIdleTimer);
    this.#wheelIdleTimer = window.setTimeout(() => {
      if (!this.#wheelRaf && !this.#wheelTickPending) this.#finishWheel();
    }, WHEEL_IDLE_MS);
    if (!this.#wheelRaf && !this.#wheelTickPending) {
      this.#wheelRaf = requestAnimationFrame(() => this.#processWheelQueue());
    }
    return true;
  }

  settle(preserve = true): void {
    if (this.#state === "idle") return;
    if (this.#state === "wheel") {
      this.#finishWheel(preserve);
      return;
    }
    const tab = getActiveTab();
    const shouldCommitTarget =
      preserve &&
      this.#state === "command" &&
      tab?.id === this.#tabId &&
      Math.abs(tab.zoom - this.#commandTargetZoom) >= 0.01;
    this.#invalidateCommand();
    if (shouldCommitTarget && tab) {
      updateTabPage(tab.id, tab.page, this.#commandTargetZoom);
    }
    if (preserve && this.#anchor) {
      const area = this.#area();
      if (area) restoreAnchor(area, this.#anchor);
    }
    this.#anchor = null;
    this.#tabId = null;
    this.#state = "idle";
    viewer.zooming = false;
    this.#markSettled();
  }

  cancel(): void {
    this.settle(false);
  }

  destroy(): void {
    this.cancel();
    clearTimeout(this.#wheelIdleTimer);
  }

  #markSettled(): void {
    this.#settledAt = performance.now();
    this.#onSettled?.();
  }

  #animateCommand(token: number): void {
    this.#commandRaf = 0;
    if (token !== this.#commandToken || this.#state !== "command") return;
    const area = this.#area();
    const tab = getActiveTab();
    if (!area || !this.#anchor || !tab || tab.id !== this.#tabId) {
      this.#finishCommand(false, token);
      return;
    }
    const pointX = area.getBoundingClientRect().left + this.#anchor.viewportX;
    const pointY = area.getBoundingClientRect().top + this.#anchor.viewportY;
    if (!this.#rebaseAnchorAt(area, pointX, pointY)) {
      this.#finishCommand(false, token);
      return;
    }

    const elapsed = performance.now() - this.#commandStartTime;
    const t = Math.min(1, elapsed / COMMAND_DURATION_MS);
    const eased = 1 - Math.pow(1 - t, 3);
    const nextZoom = this.#commandStartZoom +
      (this.#commandTargetZoom - this.#commandStartZoom) * eased;
    updateTabPage(tab.id, tab.page, nextZoom);
    void tick().then(() => {
      if (token !== this.#commandToken || this.#state !== "command") return;
      const areaNow = this.#area();
      if (areaNow && this.#anchor) restoreAnchor(areaNow, this.#anchor);
      if (t >= 1) this.#finishCommand(false, token);
      else this.#commandRaf = requestAnimationFrame(() => this.#animateCommand(token));
    });
  }

  #finishCommand(commitTarget: boolean, token: number): void {
    if (token !== this.#commandToken || this.#state !== "command") return;
    if (this.#commandRaf) cancelAnimationFrame(this.#commandRaf);
    this.#commandRaf = 0;
    clearTimeout(this.#commandTimer);
    this.#commandTimer = 0;
    const tab = getActiveTab();
    if (
      commitTarget &&
      tab &&
      tab.id === this.#tabId &&
      Math.abs(tab.zoom - this.#commandTargetZoom) >= 0.01
    ) {
      const completionToken = ++this.#commandToken;
      updateTabPage(tab.id, tab.page, this.#commandTargetZoom);
      void tick().then(() => {
        if (completionToken !== this.#commandToken || this.#state !== "command") return;
        const area = this.#area();
        if (area && this.#anchor) restoreAnchor(area, this.#anchor);
        this.#completeCommand(completionToken);
      });
      return;
    }
    this.#completeCommand(token);
  }

  #completeCommand(token: number): void {
    if (token !== this.#commandToken || this.#state !== "command") return;
    clearTimeout(this.#commandTimer);
    this.#commandTimer = 0;
    this.#anchor = null;
    this.#tabId = null;
    this.#state = "idle";
    viewer.zooming = false;
    this.#markSettled();
  }

  #scheduleCommandRestore(): void {
    this.#invalidateCommand();
    const token = this.#commandToken;
    if (!this.#area() || !this.#anchor) {
      this.#state = "idle";
      viewer.zooming = false;
      this.#markSettled();
      return;
    }
    void tick().then(() => {
      if (token !== this.#commandToken || this.#state !== "command") return;
      const area = this.#area();
      if (area && this.#anchor) restoreAnchor(area, this.#anchor);
      this.#anchor = null;
      this.#tabId = null;
      this.#state = "idle";
      viewer.zooming = false;
      this.#markSettled();
    });
    this.#commandTimer = window.setTimeout(() => {
      if (token !== this.#commandToken || this.#state !== "command") return;
      this.#invalidateCommand();
      this.#anchor = null;
      this.#tabId = null;
      this.#state = "idle";
      viewer.zooming = false;
      this.#markSettled();
    }, COMMAND_TIMEOUT_MS);
  }

  #invalidateCommand(): void {
    this.#commandToken++;
    if (this.#commandRaf) cancelAnimationFrame(this.#commandRaf);
    this.#commandRaf = 0;
    if (this.#commandTimer) clearTimeout(this.#commandTimer);
    this.#commandTimer = 0;
  }

  #processWheelQueue(): void {
    this.#wheelRaf = 0;
    if (this.#wheelTickPending || this.#state !== "wheel") return;
    const area = this.#area();
    const tab = getActiveTab();
    if (
      !area ||
      !this.#anchor ||
      Math.abs(this.#wheelDelta) < 0.0005 ||
      !tab ||
      tab.status === "restoring" ||
      tab.id !== this.#tabId ||
      !this.#rebaseAnchorAt(area, this.#wheelCursorX, this.#wheelCursorY)
    ) {
      this.#finishWheel();
      return;
    }

    const delta = Math.abs(this.#wheelDelta) < 0.01
      ? this.#wheelDelta
      : this.#wheelDelta * 0.35;
    this.#wheelDelta -= delta;
    const nextZoom = clamp(tab.zoom * Math.exp(-delta), MIN_ZOOM, MAX_ZOOM);
    if (Math.abs(nextZoom - tab.zoom) < 0.01) {
      this.#wheelDelta = 0;
      this.#finishWheel();
      return;
    }

    updateTabPage(tab.id, tab.page, nextZoom);
    this.#wheelTickPending = true;
    const token = this.#wheelToken;
    void tick().then(() => {
      if (token !== this.#wheelToken) return;
      this.#wheelTickPending = false;
      if (this.#state !== "wheel" || !this.#anchor) return;
      const areaNow = this.#area();
      if (!areaNow || !this.#anchor) {
        this.#finishWheel();
        return;
      }
      restoreAnchor(areaNow, this.#anchor);
      if (Math.abs(this.#wheelDelta) < 0.0005) this.#finishWheel();
      else if (!this.#wheelRaf) {
        this.#wheelRaf = requestAnimationFrame(() => this.#processWheelQueue());
      }
    });
  }

  #rebaseAnchorAt(area: HTMLElement, clientX: number, clientY: number): boolean {
    if (!this.#anchor) return false;
    const slot = area.querySelector<HTMLElement>(
      `.page-slot[data-page="${this.#anchor.page}"]`,
    );
    if (!slot) return false;
    const slotRect = slot.getBoundingClientRect();
    if (slotRect.width <= 0 || slotRect.height <= 0) return false;
    const areaRect = area.getBoundingClientRect();
    this.#anchor = {
      page: this.#anchor.page,
      relX: clientX - slotRect.left,
      relY: clientY - slotRect.top,
      oldWidth: slotRect.width,
      oldHeight: slotRect.height,
      viewportX: clientX - areaRect.left,
      viewportY: clientY - areaRect.top,
    };
    return true;
  }

  #finishWheel(preserveAnchor = false): void {
    this.#wheelToken++;
    this.#wheelTickPending = false;
    if (this.#wheelRaf) cancelAnimationFrame(this.#wheelRaf);
    clearTimeout(this.#wheelIdleTimer);
    this.#wheelRaf = 0;
    if (preserveAnchor) {
      const area = this.#area();
      const tab = getActiveTab();
      if (area && this.#anchor && tab?.id === this.#tabId) {
        restoreAnchor(area, this.#anchor);
      }
    }
    this.#anchor = null;
    this.#tabId = null;
    this.#wheelDelta = 0;
    this.#state = "idle";
    viewer.zooming = false;
    this.#markSettled();
  }
}
