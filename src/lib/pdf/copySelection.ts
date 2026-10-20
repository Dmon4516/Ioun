import { settings } from "$lib/stores/settings.svelte";

/**
 * Normaliza el texto copiado desde la capa de texto.
 *
 * pdf.js crea un `span` por línea visual, así que copiar un párrafo entrega un
 * salto de línea entre cada línea. Aquí se unen las líneas que pertenecen al
 * mismo párrafo con un espacio, y se mantiene el salto solo cuando la línea
 * anterior cierra frase (`.`, `!`, `?`, `:`, `;`, guion de diálogo) o la
 * siguiente empieza en mayúscula. El guion de separación silábica se elimina.
 */
export function normalizeCopiedText(raw: string, ignoreWatermark: boolean): string {
  const lines = raw.split(/\r?\n/);
  const filtered = ignoreWatermark ? dropWatermarkLines(lines) : lines;
  return joinParagraphLines(filtered);
}

/**
 * Quita las líneas que se repiten idénticas varias veces, que es la firma de una
 * marca de agua (aparece una vez por página). Es una heurística conservadora: se
 * exige al menos dos apariciones, longitud ≤ 80 y sin dígitos, para no tocar
 * encabezados numerados ni texto legítimo.
 */
function dropWatermarkLines(lines: string[]): string[] {
  const counts = new Map<string, number>();
  for (const line of lines) {
    const key = line.trim();
    if (!key || key.length > 80 || /\d/.test(key)) continue;
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return lines.filter((line) => {
    const key = line.trim();
    return !key || !counts.has(key) || (counts.get(key) ?? 0) < 2;
  });
}

function joinParagraphLines(lines: string[]): string {
  const paragraphs: string[] = [];
  let current = "";
  let previousEndsOpen = false;

  for (const rawLine of lines) {
    const line = rawLine.replace(/\s+$/, "");
    if (line === "") {
      if (current !== "") paragraphs.push(current);
      current = "";
      previousEndsOpen = false;
      continue;
    }
    if (current === "") {
      current = line;
    } else if (current.endsWith("-")) {
      // Separación silábica al final de línea: unir sin espacio.
      current = current.slice(0, -1) + line;
    } else if (previousEndsOpen && /^[a-záéíóúñü0-9("'¿¡]/.test(line)) {
      // La frase anterior no cierra y esta sigue en minúscula: misma frase.
      current += " " + line;
    } else {
      // Cierre de frase o inicio en mayúscula: párrafo nuevo (salto simple).
      current += "\n" + line;
    }
    previousEndsOpen = !/[.!?:;¿¡"—]\s*$/.test(line);
  }
  if (current !== "") paragraphs.push(current);
  return paragraphs.join("\n\n");
}

function selectionIsInTextLayer(): boolean {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return false;
  const node = selection.anchorNode;
  const element = node instanceof Element ? node : node?.parentElement;
  return element?.closest(".page-text-layer") != null;
}

function onCopy(event: ClipboardEvent): void {
  if (!selectionIsInTextLayer()) return;
  const raw = window.getSelection()?.toString() ?? "";
  if (!raw) return;
  const normalized = normalizeCopiedText(raw, settings.ignoreWatermarkOnCopy);
  if (normalized === raw) return;
  event.preventDefault();
  event.clipboardData?.setData("text/plain", normalized);
}

let installed = false;

/** Instala el único manejador de copiado del visor. Idempotente. */
export function installPdfCopyNormalization(): () => void {
  if (installed) return () => {};
  installed = true;
  document.addEventListener("copy", onCopy);
  return () => {
    installed = false;
    document.removeEventListener("copy", onCopy);
  };
}
