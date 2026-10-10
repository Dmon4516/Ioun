<script lang="ts">
  import type { OutlineNode } from "$lib/pdf/outline";
  import OutlineNodeComponent from "./OutlineNode.svelte";

  let {
    node,
    level = 1,
    onNavigate,
  }: {
    node: OutlineNode;
    level?: number;
    onNavigate: (page: number | null) => void;
  } = $props();

  const hasChildren = $derived(node.children.length > 0);
  let expanded = $state(true);
</script>

<div class="node">
  <button
    class="outline-row level-{level}"
    type="button"
    onclick={() => onNavigate(node.page)}
    disabled={node.page === null}
    title={node.page ? `Ir a página ${node.page}` : "Sin destino"}
  >
    {#if hasChildren}
      <span
        class="chev"
        class:open={expanded}
        role="presentation"
        onclick={(e) => {
          e.stopPropagation();
          expanded = !expanded;
        }}
      >›</span>
    {:else}
      <span class="chev empty"></span>
    {/if}
    <span class="title" class:bold={node.bold}>{node.title}</span>
    {#if node.page}
      <span class="page">pág. {node.page}</span>
    {/if}
  </button>

  {#if hasChildren && expanded}
    <div class="children">
      {#each node.children as child, i (i)}
        <OutlineNodeComponent node={child} level={level + 1} {onNavigate} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .node { display: grid; gap: 2px; }

  .outline-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: var(--radius-card);
    border: none;
    background: transparent;
    color: var(--text-primary);
    text-align: left;
    cursor: pointer;
    font-size: 13px;
    transition: background var(--animation-fast);
  }
  .outline-row:hover:not(:disabled) { background: var(--bg-active); }
  .outline-row:disabled { cursor: default; opacity: 0.6; }

  .chev {
    width: 12px;
    height: 12px;
    display: grid;
    place-items: center;
    color: var(--text-secondary);
    transition: transform var(--animation-fast);
    font-size: 16px;
    line-height: 1;
    flex: 0 0 auto;
  }
  .chev.open { transform: rotate(90deg); }
  .chev.empty { visibility: hidden; }

  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .title.bold { font-weight: 750; }

  .level-2 .title { font-weight: 550; opacity: 0.94; }
  .level-3 .title { font-weight: 470; opacity: 0.86; }

  .page { color: var(--text-secondary); font-size: 11px; flex: 0 0 auto; }

  .children {
    display: grid;
    gap: 2px;
    margin-left: 12px;
    padding-left: 10px;
    border-left: 1px solid var(--border);
    margin-top: 1px;
    margin-bottom: 1px;
  }
</style>
