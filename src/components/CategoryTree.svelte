<script lang="ts">
  import type { CategoryNode } from "../lib/api";
  import Self from "./CategoryTree.svelte";

  let {
    nodes,
    selected,
    onselect,
    depth = 0,
  }: { nodes: CategoryNode[]; selected: number | null; onselect: (id: number | null) => void; depth?: number } = $props();

  let open = $state<Record<number, boolean>>({});

  function contains(n: CategoryNode, id: number | null): boolean {
    return n.children.some((c) => c.id === id || contains(c, id));
  }
</script>

<ul>
  {#each nodes as n (n.id)}
    {@const expanded = open[n.id] ?? contains(n, selected)}
    <li>
      <div class="item" class:active={selected === n.id} style:padding-left="{8 + depth * 14}px">
        {#if n.children.length}
          <button class="twisty" aria-label={expanded ? "Collapse" : "Expand"} onclick={() => (open[n.id] = !expanded)}>
            {expanded ? "▾" : "▸"}
          </button>
        {:else}
          <span class="twisty"></span>
        {/if}
        <button class="name" onclick={() => onselect(n.id)} title={n.name}>
          {#if n.iconUrl}<img src={n.iconUrl} alt="" loading="lazy" width="16" height="16" />{/if}
          <span class="ellipsis">{n.name}</span>
        </button>
      </div>
      {#if n.children.length && expanded}
        <Self nodes={n.children} {selected} {onselect} depth={depth + 1} />
      {/if}
    </li>
  {/each}
</ul>

<style>
  ul { list-style: none; margin: 0; padding: 0; }
  .item { display: flex; align-items: center; border-radius: 4px; margin-bottom: 1px; }
  .item:hover { background: var(--panel); }
  .item:hover .name { color: var(--text); }
  .item.active { background: var(--panel-2); }
  .item.active .name { color: var(--text); font-weight: 600; }
  .twisty { width: 18px; flex: none; padding: 0; color: var(--dim); font-size: 10px; text-align: center; background: transparent !important; font-weight: 400; }
  .name {
    flex: 1; justify-content: flex-start; padding: 6px 8px 6px 2px; color: var(--muted); font-weight: 500;
    gap: 9px; background: transparent !important; min-width: 0;
  }
  img { border-radius: 3px; flex: none; }
</style>
