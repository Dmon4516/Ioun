// Capa de texto de pdf.js. **Integrada** en `PageSlot` y encendida por defecto
// (`settings.textLayerEnabled`); `TextLayerPrototype.svelte` se conserva como
// banco de medidas, accesible en desarrollo con `?proto=text`.
//
// Requisitos para que la medición sea comparable con el canvas:
//  - El contenedor debe llevar la clase `textLayer` y tener cargadas las
//    reglas de `pdfjs-dist/web/pdf_viewer.css`.
//  - `--scale-factor` debe coincidir con la escala usada al pintar el canvas.
//
// La versión de pdfjs-dist debe estar fijada al medir: `TextLayer` y
// `setLayerDimensions` no son API estable entre versiones mayores. En
// pdfjs-dist 4.x, `TextLayer` se exporta desde la entrada principal.

import { TextLayer, setLayerDimensions } from "pdfjs-dist";
import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";
import { getPageCached } from "./renderPage";

export class PageTextLayer {
  #layer: TextLayer | null = null;
  // pdf.js exige un `HTMLDivElement` en `setLayerDimensions`.
  #container: HTMLDivElement | null = null;
  #generation = 0;

  async render(
    pdf: PDFDocumentProxy,
    docId: string,
    pageNumber: number,
    scale: number,
    container: HTMLDivElement,
  ): Promise<void> {
    const token = ++this.#generation;

    const page: PDFPageProxy = await getPageCached(pdf, pageNumber, docId);
    if (token !== this.#generation) return;

    container.textContent = "";
    this.#container = container;

    const viewport = page.getViewport({ scale });
    container.style.setProperty("--scale-factor", String(scale));
    // Igual que el visor de pdf.js: el tamaño se deriva de `--scale-factor`,
    // así que rotaciones y redondeos los resuelve pdf.js, no nosotros.
    setLayerDimensions(container, viewport);

    const layer = new TextLayer({
      textContentSource: page.streamTextContent(),
      container,
      viewport,
    });
    this.#layer = layer;

    try {
      await layer.render();
    } catch (error) {
      if (token !== this.#generation) return;
      console.error("[text-layer] Error al construir la capa de texto", error);
      this.#layer = null;
      throw error;
    }
  }

  /** Aborta el render en curso y vacía el contenedor. */
  cancel(): void {
    this.#generation++;
    try {
      this.#layer?.cancel();
    } catch {
      // Ya cancelado o el documento se cerró antes.
    }
    this.#layer = null;
    if (this.#container) this.#container.textContent = "";
  }

  /**
   * Suelta la capa. Debe llamarse antes de `pdf.destroy()`: el render en curso
   * no debe tocar un documento ya liberado.
   */
  destroy(): void {
    this.cancel();
    this.#container = null;
  }
}
