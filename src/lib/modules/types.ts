import type { Component } from "svelte";
import type { DocumentRecord, Tab } from "$lib/types";

export interface DocumentModule {
  kind: DocumentRecord["kind"];
  View: Component<{ doc: DocumentRecord; tab: Tab }>;
}
