import type { PDFPageProxy } from "pdfjs-dist";

export interface TextItem {
  str: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export async function extractText(page: PDFPageProxy): Promise<TextItem[]> {
  const content = await page.getTextContent();
  const items: TextItem[] = [];
  for (const item of content.items) {
    if (!("str" in item)) continue;
    const t = item.transform;
    items.push({
      str: item.str,
      x: t[4] ?? 0,
      y: t[5] ?? 0,
      width: item.width,
      height: item.height,
    });
  }
  return items;
}
