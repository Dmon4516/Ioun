export const HEX_RE = /^#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

export function safeColor(c: string): string {
  return HEX_RE.test(c) ? c : "#888888";
}

export function hexWithAlpha(hex: string, alpha: string): string {
  if (!HEX_RE.test(hex)) return hex;
  const value = hex.slice(1);
  if (value.length === 3 || value.length === 4) {
    return `#${value.slice(0, 3).split("").map((part) => part + part).join("")}${alpha}`;
  }
  if (value.length === 6) return `${hex}${alpha}`;
  return hex;
}

export function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

export function clampInt(v: unknown, min: number, max: number, fallback: number): number {
  if (typeof v !== "number" || !Number.isFinite(v)) return fallback;
  return Math.min(max, Math.max(min, Math.trunc(v)));
}
