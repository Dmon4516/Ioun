import type { PDFDocumentProxy } from "pdfjs-dist";

export interface OutlineNode {
  title: string;
  page: number | null;
  children: OutlineNode[];
  bold: boolean;
}

async function resolveDest(
  pdf: PDFDocumentProxy,
  dest: string | unknown[] | null,
): Promise<number | null> {
  if (!dest) return null;

  let explicit: unknown[] | null = null;
  if (typeof dest === "string") {
    try {
      explicit = (await pdf.getDestination(dest)) as unknown[] | null;
    } catch {
      return null;
    }
  } else if (Array.isArray(dest)) {
    explicit = dest;
  }
  if (!explicit || explicit.length === 0) return null;

  const ref = explicit[0];
  try {
    if (typeof ref === "object" && ref !== null) {
      const idx = await pdf.getPageIndex(ref as Parameters<PDFDocumentProxy["getPageIndex"]>[0]);
      return idx + 1;
    }
    if (typeof ref === "number") return ref + 1;
  } catch {
    return null;
  }
  return null;
}

export async function buildOutline(pdf: PDFDocumentProxy): Promise<OutlineNode[]> {
  const raw = await pdf.getOutline();
  if (!raw) return [];

  async function walk(items: typeof raw): Promise<OutlineNode[]> {
    const out: OutlineNode[] = [];
    for (const item of items ?? []) {
      const page = await resolveDest(pdf, item.dest);
      const children = item.items ? await walk(item.items) : [];
      out.push({
        title: item.title || "Sin título",
        page,
        children,
        bold: !!item.bold,
      });
    }
    return out;
  }

  return walk(raw);
}
