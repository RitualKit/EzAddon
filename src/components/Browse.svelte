<script lang="ts">
  import { onMount } from "svelte";
  import { api, formatCount, SORTS, toAppError, type AddonSummary, type CategoryNode } from "../lib/api";
  import { showError } from "../lib/state.svelte";
  import CategoryTree from "./CategoryTree.svelte";
  import AddonCard from "./AddonCard.svelte";

  let categories = $state<CategoryNode[]>([]);
  let categoryId = $state<number | null>(null);
  let query = $state("");
  let sort = $state<number>(2);

  let results = $state<AddonSummary[]>([]);
  let total = $state(0);
  let hasMore = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let scroller: HTMLDivElement | undefined = $state();

  let seq = 0;
  let debounce: ReturnType<typeof setTimeout> | undefined;

  function findName(nodes: CategoryNode[], id: number | null): string | null {
    for (const n of nodes) {
      if (n.id === id) return n.name;
      const c = findName(n.children, id);
      if (c) return c;
    }
    return null;
  }
  const categoryName = $derived(findName(categories, categoryId));

  async function load(reset: boolean) {
    const my = ++seq;
    loading = true;
    error = null;
    try {
      const page = await api.search(query, categoryId, sort, reset ? 0 : results.length);
      if (my !== seq) return;
      results = reset ? page.addons : [...results, ...page.addons.filter((a) => !results.some((r) => r.id === a.id))];
      total = page.pagination.totalCount;
      hasMore = page.hasMore;
      if (reset) scroller?.scrollTo({ top: 0 });
    } catch (e) {
      if (my !== seq) return;
      error = toAppError(e).message;
      if (reset) results = [];
    } finally {
      if (my === seq) loading = false;
    }
  }

  function onQuery() {
    clearTimeout(debounce);
    debounce = setTimeout(() => load(true), 300);
  }

  function pick(id: number | null) {
    categoryId = id;
    load(true);
  }

  async function loadCategories(refresh = false) {
    try {
      categories = await api.getCategories(refresh);
    } catch (e) {
      showError(e);
    }
  }

  onMount(() => {
    loadCategories();
    load(true);
  });
</script>

<div class="browse">
  <aside class="scroll">
    <div class="side-title row">
      <span class="grow muted small">CATEGORIES</span>
      <button class="ghost small" title="Reload categories" onclick={() => loadCategories(true)}>↻</button>
    </div>
    <button class="all ghost" class:active={categoryId === null} onclick={() => pick(null)}>All addons</button>
    <CategoryTree nodes={categories} selected={categoryId} onselect={pick} />
  </aside>

  <section>
    <div class="toolbar row">
      <div class="search grow">
        <span class="icon muted">⌕</span>
        <input
          type="search"
          placeholder="Search WoW Forever addons…"
          bind:value={query}
          oninput={onQuery}
          onkeydown={(e) => e.key === "Enter" && (clearTimeout(debounce), load(true))}
        />
      </div>
      <select bind:value={sort} onchange={() => load(true)} aria-label="Sort by">
        {#each SORTS as s}<option value={s.id}>{s.label}</option>{/each}
      </select>
    </div>

    <div class="meta muted small">
      {#if categoryName}<strong>{categoryName}</strong> · {/if}
      {#if !loading || results.length}{formatCount(total)} addon{total === 1 ? "" : "s"}{/if}
    </div>

    <div class="results scroll" bind:this={scroller}>
      {#if error}
        <div class="empty">
          <p>{error}</p>
          <button onclick={() => load(true)}>Try again</button>
        </div>
      {:else if !loading && results.length === 0}
        <div class="empty muted">No WoW Forever addons match your search.</div>
      {:else}
        <div class="grid">
          {#each results as a (a.id)}<AddonCard addon={a} />{/each}
        </div>
        <div class="more">
          {#if loading}
            <span class="muted"><span class="spinner"></span>&nbsp; Loading…</span>
          {:else if hasMore}
            <button onclick={() => load(false)}>Load more</button>
          {/if}
        </div>
      {/if}
    </div>
  </section>
</div>

<style>
  .browse { display: flex; height: 100%; }
  aside { width: 230px; flex: none; padding: 12px 8px; border-right: 1px solid var(--line); background: var(--bg-2); }
  .side-title { padding: 0 8px 6px; letter-spacing: 0.6px; }
  .side-title button { padding: 0 6px; color: var(--muted); }
  .all { width: 100%; text-align: left; padding: 6px 10px; color: var(--muted); margin-bottom: 2px; }
  .all.active { color: var(--text); background: var(--panel-2); box-shadow: inset 3px 0 0 var(--accent); }
  section { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .toolbar { padding: 12px 16px 6px; }
  .search { position: relative; }
  .search input { width: 100%; padding-left: 30px; }
  .search .icon { position: absolute; left: 10px; top: 50%; transform: translateY(-52%); font-size: 16px; }
  .meta { padding: 2px 16px 8px; min-height: 26px; }
  .results { flex: 1; padding: 0 16px 16px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .more { display: flex; justify-content: center; padding: 16px; min-height: 64px; }
  .empty { padding: 60px 16px; text-align: center; }
</style>
