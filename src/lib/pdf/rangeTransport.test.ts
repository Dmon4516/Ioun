// @vitest-environment node
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { setInvokeMock } from "../../test/setup";

// pdf.js 4.x usa `Promise.withResolvers`, que no existe en el Node de este
// entorno (el webview objetivo sí lo tiene). Aquí solo hace falta para el test.
type WithResolvers = <T>() => {
  promise: Promise<T>;
  resolve: (value: T | PromiseLike<T>) => void;
  reject: (reason?: unknown) => void;
};
const promiseCtor = Promise as unknown as { withResolvers?: WithResolvers };
if (typeof promiseCtor.withResolvers !== "function") {
  promiseCtor.withResolvers = function withResolvers<T>() {
    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
      resolve = res;
      reject = rej;
    });
    return { promise, resolve, reject };
  };
}

const { getDocument, GlobalWorkerOptions } = await import("pdfjs-dist");
// En Node no hay `Worker`, así que pdf.js usa su worker "falso" e importa este
// archivo en el hilo principal.
GlobalWorkerOptions.workerSrc = pathToFileURL(
  path.join(process.cwd(), "node_modules/pdfjs-dist/build/pdf.worker.mjs"),
).href;
const { createRangeTransport } = await import("./rangeTransport");
const { PDFDataRangeTransport } = await import("pdfjs-dist");

/** PDF mínimo válido de una página, con los offsets del `xref` calculados. */
function buildPdf(paddingBytes: number): Uint8Array<ArrayBuffer> {
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R >>",
  ];
  const pad = " ".repeat(paddingBytes);
  objects.push(`<< /Length ${pad.length} >>\nstream\n${pad}\nendstream`);

  let body = "%PDF-1.4\n";
  const offsets: number[] = [];
  objects.forEach((object, index) => {
    offsets.push(Buffer.byteLength(body));
    body += `${index + 1} 0 obj\n${object}\nendobj\n`;
  });

  const xrefOffset = Buffer.byteLength(body);
  body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const offset of offsets) {
    body += `${String(offset).padStart(10, "0")} 00000 n \n`;
  }
  body += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
  return new TextEncoder().encode(body);
}

let dir = "";
let pdfPath = "";
let pdfBytes: Uint8Array<ArrayBuffer> = new Uint8Array(0);
let requests: { begin: number; end: number }[] = [];

/** Devuelve el mock de lectura de rangos a su comportamiento normal. */
function restoreRangeMock(): void {
  setInvokeMock("read_pdf_range", (args) => {
    const { begin, end } = args as { begin: number; end: number };
    const buffer = pdfBytes.subarray(begin, Math.min(end, pdfBytes.length));
    // La orden Rust devuelve bytes crudos, que llegan como ArrayBuffer.
    return buffer.buffer.slice(
      buffer.byteOffset,
      buffer.byteOffset + buffer.byteLength,
    ) as unknown as ArrayBuffer;
  });
}

beforeAll(() => {
  dir = mkdtempSync(path.join(tmpdir(), "lyra-range-"));
  pdfPath = path.join(dir, "grande.pdf");
  // 300 KB de relleno: el archivo es mucho mayor que el bloque que pide pdf.js,
  // que es justo lo que hace útil el transporte por rangos.
  pdfBytes = buildPdf(300 * 1024);
  writeFileSync(pdfPath, pdfBytes);

  setInvokeMock("read_pdf_range", (args) => {
    const { begin, end } = args as { begin: number; end: number };
    requests.push({ begin, end });
    const buffer = pdfBytes.subarray(begin, Math.min(end, pdfBytes.length));
    return buffer.buffer.slice(
      buffer.byteOffset,
      buffer.byteOffset + buffer.byteLength,
    ) as unknown as ArrayBuffer;
  });
});

afterAll(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe("LyraRangeTransport", () => {
  it("pide rangos en vez del archivo entero y pdf.js lo parsea", async () => {
    requests = [];
    const transport = createRangeTransport(PDFDataRangeTransport, pdfPath, pdfBytes.length);

    const pdf = await getDocument({
      range: transport,
      disableAutoFetch: true,
      rangeChunkSize: 64 * 1024,
    }).promise;

    expect(pdf.numPages).toBe(1);
    expect(requests.length).toBeGreaterThan(0);

    // Ningún bloque puede pedir el archivo entero de golpe...
    const bytesRead = requests.reduce((total, r) => total + (r.end - r.begin), 0);
    expect(bytesRead).toBeLessThan(pdfBytes.length);
    // ...y cada bloque tiene que caer dentro del archivo.
    for (const { begin, end } of requests) {
      expect(begin).toBeGreaterThanOrEqual(0);
      expect(end).toBeGreaterThan(begin);
      expect(end).toBeLessThanOrEqual(pdfBytes.length);
    }
    // El relleno de 300 KB no se lee: solo el encabezado, el xref y los objetos.
    const maxChunk = Math.max(...requests.map((r) => r.end - r.begin));
    expect(maxChunk).toBeLessThanOrEqual(64 * 1024);

    // pdf.js pide el principio (comprobación de cabecera) y el final (donde vive
    // el `startxref`) como rangos separados y deja el resto para más adelante.
    expect(requests.some((r) => r.begin === 0)).toBe(true);
    expect(requests.some((r) => r.end === pdfBytes.length)).toBe(true);

    await pdf.destroy();
  });

  it("sirve los bytes exactos que pide cada rango", async () => {
    requests = [];
    const transport = createRangeTransport(PDFDataRangeTransport, pdfPath, pdfBytes.length);
    const received: { begin: number; chunk: Uint8Array }[] = [];
    transport.addRangeListener((begin: number, chunk: Uint8Array) =>
      received.push({ begin, chunk }),
    );

    transport.requestDataRange(100, 200);
    await vi.waitFor(() => expect(received).toHaveLength(1));

    expect(requests).toEqual([{ begin: 100, end: 200 }]);
    expect(Array.from(received[0]!.chunk)).toEqual(Array.from(pdfBytes.subarray(100, 200)));
  });

  it("deja de pedir rangos tras `abort`", async () => {
    requests = [];
    const transport = createRangeTransport(PDFDataRangeTransport, pdfPath, pdfBytes.length);
    transport.abort();

    transport.requestDataRange(0, 1024);
    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(requests).toEqual([]);
  });

  it("rechaza la carga cuando un rango no se puede leer", async () => {
    // El escenario: el documento se quita de la biblioteca (o el disco se
    // desconecta) mientras se está leyendo. pdf.js deja al lector esperando su
    // bloque, así que sin avisar de alguna forma la carga se quedaba colgada.
    setInvokeMock("read_pdf_range", () => Promise.reject(new Error("disco desconectado")));

    try {
      const transport = createRangeTransport(PDFDataRangeTransport, pdfPath, pdfBytes.length);
      const task = getDocument({
        range: transport,
        disableAutoFetch: true,
        rangeChunkSize: 64 * 1024,
      });

      // Antes de avisar a pdf.js, esto no terminaba nunca: ni resolvía ni
      // rechazaba. Lo que importa es que la carga deje de estar colgada.
      const outcome = await Promise.race([
        task.promise.then(
          () => ({ kind: "resolved" as const }),
          (error: unknown) => ({ kind: "rejected" as const, error }),
        ),
        new Promise<{ kind: "timeout" }>((resolve) =>
          setTimeout(() => resolve({ kind: "timeout" }), 4000)),
      ]);

      expect(outcome.kind).toBe("rejected");
    } finally {
      restoreRangeMock();
    }
  }, 10_000);

  it("cierra también los rangos que lleguen después de un fallo", async () => {
    const transport = createRangeTransport(PDFDataRangeTransport, pdfPath, pdfBytes.length);
    const received: { begin: number; chunk: Uint8Array }[] = [];
    transport.addRangeListener((begin: number, chunk: Uint8Array) =>
      received.push({ begin, chunk }),
    );

    setInvokeMock("read_pdf_range", () => Promise.reject(new Error("fallo")));
    transport.requestDataRange(0, 100);
    await vi.waitFor(() => expect(received).toHaveLength(1));
    expect(received[0]!.chunk.byteLength).toBe(1);
    expect(requests).toEqual([]);

    // Sin esto, el siguiente bloque se quedaría esperando para siempre.
    transport.requestDataRange(100, 200);
    await vi.waitFor(() => expect(received).toHaveLength(2));
    expect(received[1]!.chunk.byteLength).toBe(1);

    restoreRangeMock();
  });
});
