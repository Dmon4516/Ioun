import type { PDFDataRangeTransport } from "pdfjs-dist";
import { readPdfRange } from "$lib/tauri/commands";

/**
 * Transporte por rangos sobre el disco.
 *
 * pdf.js solo añade la cabecera `Range` cuando la URL es `http(s)`
 * (`isHttp` en su `NetworkManager`), así que con el esquema propio
 * `lyra-pdf://` pedía siempre el archivo completo: un PDF de 300 MB se
 * materializaba entero en Rust y en el webview. Con este transporte pdf.js pide
 * solo los bloques que necesita (`requestDataRange`) y cada bloque se lee del
 * disco por IPC.
 *
 * Es de acceso aleatorio puro: no hay lectura progresiva, así que se declara
 * `progressiveDone` en el constructor y todo lo demás llega por `getRangeReader`.
 *
 * ## Por qué una fábrica y no una clase exportada
 *
 * pdf.js exige que el transporte sea `instanceof` **de su propia** clase
 * (`src.range instanceof PDFDataRangeTransport` en `getDocument`), así que tiene
 * que heredar de la clase del módulo que use el cargador. Heredar de la clase
 * importada aquí obligaría a importar `pdfjs-dist` de forma estática, y eso
 * arrastra pdf.js (334 KB) al chunk inicial, cuando `loader.ts` lo carga aparte
 * con `import()` a propósito. Con la fábrica, la clase base se pasa ya cargada.
 */
export function createRangeTransport(
  base: typeof PDFDataRangeTransport,
  path: string,
  length: number,
): PDFDataRangeTransport {
  class RangeTransport extends base {
    #path = path;
    #aborted = false;
    #failed = false;

    constructor() {
      // `initialData` vacío: el primer bloque también se pide por rango, así que
      // no se duplica la lectura del encabezado.
      super(length, null, true);
    }

    override requestDataRange(begin: number, end: number): void {
      if (this.#aborted) return;
      // Una lectura ya falló: lo que pida a partir de ahora se cierra igual en
      // vez de quedarse esperando para siempre. Se entrega en una microtarea
      // porque pdf.js registra el lector justo **después** de llamar aquí, y
      // entregar el bloque antes haría saltar su comprobación interna.
      if (this.#failed) {
        queueMicrotask(() => this.onDataRange(begin, failureChunk()));
        return;
      }
      void this.#read(begin, end);
    }

    override abort(): void {
      this.#aborted = true;
    }

    /** Expuesto para las pruebas. */
    get aborted(): boolean {
      return this.#aborted;
    }

    async #read(begin: number, end: number): Promise<void> {
      let chunk: Uint8Array;
      try {
        const buffer = await readPdfRange(this.#path, begin, end);
        if (this.#aborted) return;
        // Llega como `ArrayBuffer` porque la orden Rust devuelve bytes crudos.
        chunk = new Uint8Array(buffer);
      } catch (error) {
        console.error(`[pdf] No se pudo leer el rango ${begin}-${end} de ${this.#path}`, error);
        this.#failRead(begin);
        return;
      }
      this.onDataRange(begin, chunk);
      this.onDataProgress(Math.min(begin + chunk.byteLength, this.length), this.length);
    }

    /**
     * Avisa a pdf.js de que un bloque no se pudo leer.
     *
     * Medido con pdf.js 4.10 (ver `rangeTransport.test.ts`): `PDFDataRangeTransport`
     * no tiene ningún gancho de error —solo `onDataRange`/`onDataProgress`, y ni
     * llamar a `abort()` ni destruir la `PDFDocumentLoadingTask` desbloquean a un
     * lector que espera su bloque—, y un bloque **vacío** tampoco: pdf.js lo trata
     * como «todavía no hay datos» y sigue esperando. Lo único que hace terminar la
     * espera con un error real (que `getDocument().promise` rechaza) es entregarle
     * al rango bytes que no puede analizar.
     */
    #failRead(begin: number): void {
      this.#failed = true;
      this.onDataRange(begin, failureChunk());
    }
  }

  return new RangeTransport();
}

/** Indica a pdf.js que el bloque no se pudo leer. Ver `#failRead`. */
function failureChunk(): Uint8Array {
  return new Uint8Array(1);
}
