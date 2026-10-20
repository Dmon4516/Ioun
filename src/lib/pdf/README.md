# Capa PDF (pdf.js)

- Worker configurado **una sola vez** en `loader.ts`.
- Todo render pasa por `renderPage.ts`:
  - `getPageCached` mantiene un LRU de `PDFPageProxy` por documento
    (`MAX_PAGES_PER_DOC = 80`).
  - El LRU respeta `protectedPages.ts`: las páginas visibles o en render no se
    expulsan.
  - `cleanupPage` / `cleanupAllPages` liberan páginas con `page.cleanup()`.
- `loader.ts` mantiene un LRU de documentos abiertos, con el límite tomado de
  `settings.maxLoadedTabs` (`maxOpenDocs()`), y al abrir un PDF mide la proporción
  de su página 1 y la publica en `documentAspect.svelte.ts`. El visor (colocación
  de las páginas) y las miniaturas la comparten, así que ninguna espera a abrir una
  página.
- `rangeTransport.ts` sirve el PDF **por rangos**. pdf.js solo añade la cabecera
  `Range` cuando la URL es `http(s)`, así que con el esquema propio `lyra-pdf://`
  pedía el archivo entero. Es una **fábrica** (`createRangeTransport`), no una
  clase: la clase base de pdf.js se le pasa ya cargada porque pdf.js exige
  `instanceof` de su propia clase, y heredar de una importación estática
  arrastraría pdf.js al chunk inicial. `loader.ts` decide con `pdfFileLength`;
  si no puede, cae a la URL del protocolo.
- `documentState.svelte.ts` es un módulo hoja con los documentos ya pintados
  (`markDocumentLoaded`/`unmarkDocumentLoaded`). Vive aparte para que `loader.ts`
  lo limpie al cerrar y al expulsar del LRU sin crear un ciclo de imports con los
  stores.
- El estado de páginas del visor (`loadedPages`, `visiblePages` e
  `inFlightRenders`) usa claves `docId:page`. Al cerrar o hibernar una pestaña,
  `clearViewerPages` elimina las entradas de ese documento sin tocar las de
  otros.
- `viewer.svelte.ts` no importa el store de pestañas. La navegación programática
  que necesita actualizar ambos estados vive en
  `src/lib/stores/viewerCommands.svelte.ts`.
- `textLayer.svelte.ts` es el wrapper del `TextLayer` oficial de pdf.js
  (`PageTextLayer`). Está **integrado** en `PageSlot` detrás de
  `settings.textLayerEnabled` (encendido por defecto); ni el módulo ni el
  `pdf_viewer.css` entran en el arranque: se cargan al activarla.
- `textLayer.ts` (`extractText`) no lo usa nadie: la capa construye el texto con
  `page.streamTextContent()`. Se conserva porque es la vía natural para una
  búsqueda dentro del documento sin pintar la capa, pero hoy es código sin usar.
- Nunca importar `pdfjs-dist` fuera de esta carpeta.
