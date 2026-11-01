<script lang="ts">
  import type { DocumentRecord, Tab } from "$lib/types";
  import PdfViewerCore from "./PdfViewerCore.svelte";

  let { doc: _doc, tab: _tab }: { doc: DocumentRecord; tab: Tab } = $props();
</script>

<PdfViewerCore />
