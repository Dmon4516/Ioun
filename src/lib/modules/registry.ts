import type { DocumentKind } from "$lib/types";
import type { DocumentModule } from "./types";
import PdfView from "$components/Viewer/PdfView.svelte";

const pdfModule: DocumentModule = {
  kind: "pdf",
  View: PdfView,
};

const modules: Partial<Record<DocumentKind, DocumentModule>> = {
  pdf: pdfModule,
};

export function getModule(kind: DocumentKind): DocumentModule | null {
  return modules[kind] ?? null;
}
