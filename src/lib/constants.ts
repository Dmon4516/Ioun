export const PAGE_GAP = 26;
export const VIEWER_PADDING_TOP = 74;

/** Paso de desplazamiento del visor: algo menos de una pantalla (botones
 * arriba/abajo de la barra y Shift+flecha). */
export const SCROLL_STEP_PAGE = { fraction: 0.85, minPx: 80 };
/** Paso corto, para moverse "un poco" con las flechas del teclado. */
export const SCROLL_STEP_FINE = { fraction: 0.15, minPx: 40 };
export const MIN_ZOOM = 40;
export const MAX_ZOOM = 999;
export const DEFAULT_ASPECT_RATIO = 760 / 1000;

export const WHEEL_IDLE_MS = 180;
export const COMMAND_DURATION_MS = 160;
export const SETTLE_LOCK_MS = 160;
