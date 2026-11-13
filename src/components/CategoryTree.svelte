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
          <button class="twisty ghost" aria-label={expanded ? "Collapse" : "Expand"} onclick={() => (open[n.id] = !expanded)}>
            {expanded ? "▾" : "▸"}
          </button>
        {:else}
          <span class="twisty"></span>
        {/if}
        <button class="name ghost ellipsis" onclick={() => onselect(n.id)} title={n.name}>
          {#if n.iconUrl}<img src={n.iconUrl} alt="" loading="lazy" width="16" height="16" />{/if}
          {n.name}
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
  .item { display: flex; align-items: center; border-radius: 7px; }
  .item:hover { background: var(--panel-2); }
  .item.active { background: var(--panel-2); box-shadow: inset 3px 0 0 var(--accent); }
  .item.active .name { color: var(--text); }
  .twisty { width: 20px; flex: none; padding: 0; color: var(--muted); font-size: 11px; text-align: center; }
  .name {
    flex: 1; text-align: left; padding: 5px 6px; color: var(--muted);
    display: flex; align-items: center; gap: 7px; background: transparent !important;
  }
  .name:hover { color: var(--text); }
  img { border-radius: 3px; flex: none; }
</style>
