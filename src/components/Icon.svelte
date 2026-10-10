<script lang="ts" module>
  export type IconName =
    | "search" | "folder" | "book" | "info" | "settings" | "close"
    | "arrow-up" | "arrow-down" | "minus" | "plus" | "grid" | "file"
    | "refresh" | "rotate-ccw" | "trash" | "chevron-down"
    | "fit" | "layers" | "sun" | "moon" | "monitor" | "comment"
    | "chevron-right" | "folder-open";

  const paths: Record<IconName, string> = {
    "search": '<circle cx="11" cy="11" r="6"></circle><path d="M20 20l-4.2-4.2"></path>',
    "folder": '<path d="M3.5 7.5a2 2 0 0 1 2-2h4.1l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"></path>',
    "folder-open": '<path d="M3.5 7.5a2 2 0 0 1 2-2h4.1l2 2h7a2 2 0 0 1 2 2v.5H8l-2 8.5H3.5v-11z"></path><path d="M8 10h13l-2 8.5H6z"></path>',
    "book": '<path d="M12 6.5C10.5 5 8.5 4.5 6 4.5H3v14h3c2.5 0 4.5.5 6 2 1.5-1.5 3.5-2 6-2h3v-14h-3c-2.5 0-4.5.5-6 2z"></path><path d="M12 6.5v13"></path>',
    "info": '<circle cx="12" cy="12" r="9"></circle><path d="M12 10.6v5"></path><path d="M12 7.2h.01"></path>',
    "settings": '<path d="M5 8h8M17 8h2M5 16h2M11 16h8"></path><circle cx="15" cy="8" r="2.2"></circle><circle cx="9" cy="16" r="2.2"></circle>',
    "close": '<path d="M6 6l12 12M18 6L6 18"></path>',
    "arrow-up": '<path d="M12 19V5"></path><path d="M6.5 10.5L12 5l5.5 5.5"></path>',
    "arrow-down": '<path d="M12 5v14"></path><path d="M6.5 13.5L12 19l5.5-5.5"></path>',
    "minus": '<path d="M7 12h10"></path>',
    "plus": '<path d="M12 7v10M7 12h10"></path>',
    "grid": '<rect x="3" y="3" width="7" height="7" rx="1.7"></rect><rect x="14" y="3" width="7" height="7" rx="1.7"></rect><rect x="3" y="14" width="7" height="7" rx="1.7"></rect><rect x="14" y="14" width="7" height="7" rx="1.7"></rect>',
    "file": '<path d="M6.5 3.5h7l4.5 4.5v11.5a2 2 0 0 1-2 2h-9.5a2 2 0 0 1-2-2v-14a2 2 0 0 1 2-2z"></path><path d="M13.5 3.5v4.5h4.5"></path>',
    "refresh": '<path d="M20 12a8 8 0 1 1-2.34-5.66"></path><path d="M20 5v4h-4"></path>',
    "rotate-ccw": '<path d="M3 12a9 9 0 1 0 3-6.7"></path><path d="M3 4v5h5"></path>',
    "trash": '<path d="M4 7h16"></path><path d="M9 7V5h6v2"></path><path d="M6.5 7l.8 12.5h9.4L17.5 7"></path><path d="M10 11v5.5M14 11v5.5"></path>',
    "chevron-down": '<path d="M6 9l6 6 6-6"></path>',
    "chevron-right": '<path d="M9 6l6 6-6 6"></path>',
    "fit": '<path d="M4 12h16"></path><path d="M7.5 8.5L4 12l3.5 3.5"></path><path d="M16.5 8.5L20 12l-3.5 3.5"></path>',
    "layers": '<path d="M12 3l9 5-9 5-9-5z"></path><path d="M3 13l9 5 9-5"></path>',
    "sun": '<circle cx="12" cy="12" r="4"></circle><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"></path>',
    "moon": '<path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z"></path>',
    "monitor": '<rect x="2" y="3" width="20" height="14" rx="2"></rect><path d="M8 21h8M12 17v4"></path>',
    "comment": '<path d="M4 5h16v11H8l-4 4z"></path>',
  };
</script>

<script lang="ts">
  let {
    name,
    size = 16,
    strokeWidth = 1.9,
  }: { name: IconName; size?: number; strokeWidth?: number } = $props();

  const html = $derived(paths[name] ?? "");
</script>

<svg
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  {@html html}
</svg>
