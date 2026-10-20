/**
 * Genera un ID con el mismo formato que el backend (`prefix-` + 32 hex chars).
 * Solo los datos transitorios (pestañas) se generan en el frontend; los
 * persistentes los genera Rust.
 */
export function uid(prefix = "id"): string {
  const bytes = new Uint8Array(16);
  if (typeof crypto !== "undefined" && "getRandomValues" in crypto) {
    crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(Math.random() * 256);
  }
  const hex = Array.from(bytes, (b) => (b ?? 0).toString(16).padStart(2, "0")).join("");
  return `${prefix}-${hex}`;
}
