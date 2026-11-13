<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, formatCount, SORTS, toAppError, type AddonSummary, type CategoryNode } from "../lib/api";
  import { showError } from "../lib/state.svelte";
  import CategoryTree from "./CategoryTree.svelte";
  import AddonRow from "./AddonRow.svelte";
  import Icon from "./Icon.svelte";

  // Raw state: lists are replaced wholesale, so deep proxies would only cost memory.
  let categories = $state.raw<CategoryNode[]>([]);
  let categoryId = $state<number | null>(null);
  let query = $state("");
  let sort = $state<number>(2);

  let results = $state.raw<AddonSummary[]>([]);
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
      if (reset) {
        results = page.addons;
      } else {
        const seen = new Set(results.map((r) => r.id));
        results = results.concat(page.addons.filter((a) => !seen.has(a.id)));
      }
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
  onDestroy(() => clearTimeout(debounce));
</script>

<div class="browse">
  <aside class="scroll">
    <div class="side-title row">
      <span class="grow">Categories</span>
      <button class="icon" title="Reload categories" onclick={() => loadCategories(true)}><Icon name="refresh" size={14} /></button>
    </div>
    <button class="all" class:active={categoryId === null} onclick={() => pick(null)}>
      <Icon name="grid" size={15} /> All Addons
    </button>
    <CategoryTree nodes={categories} selected={categoryId} onselect={pick} />
  </aside>

  <section>
    <div class="toolbar row">
      <div class="search grow">
        <span class="icon-in"><Icon name="search" size={16} /></span>
        <input
          type="search"
          placeholder="Search addons"
          bind:value={query}
          oninput={onQuery}
          onkeydown={(e) => e.key === "Enter" && (clearTimeout(debounce), load(true))}
        />
      </div>
      <label class="sort row">
        <span class="dim small">Sort by</span>
        <select bind:value={sort} onchange={() => load(true)}>
          {#each SORTS as s}<option value={s.id}>{s.label}</option>{/each}
        </select>
      </label>
    </div>

    <div class="heading row">
      <h2 class="grow ellipsis">{categoryName ?? "All Addons"}</h2>
      {#if !loading || results.length}<span class="dim small">{formatCount(total)} results</span>{/if}
    </div>

    <div class="results scroll" bind:this={scroller}>
      {#if error}
        <div class="empty">
          <p>{error}</p>
          <button class="outline" onclick={() => load(true)}>Try again</button>
        </div>
      {:else if !loading && results.length === 0}
        <div class="empty muted">No WoW Forever addons match your search.</div>
      {:else}
        <div class="list">
          {#each results as a (a.id)}<AddonRow addon={a} />{/each}
        </div>
        <div class="more">
          {#if loading}
            <span class="spinner muted"></span>
          {:else if hasMore}
            <button class="outline" onclick={() => load(false)}>Load more</button>
          {/if}
        </div>
      {/if}
    </div>
  </section>
</div>

<style>
  .browse { display: flex; height: 100%; }
  aside { width: 240px; flex: none; padding: 16px 10px; border-right: 1px solid var(--line); background: var(--bg-2); }
  .side-title { padding: 0 8px 8px; font-size: 12px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.8px; color: var(--dim); }
  .side-title .icon { padding: 4px; }
  .all {
    width: 100%; justify-content: flex-start; padding: 7px 10px; margin-bottom: 2px;
    background: transparent; color: var(--muted); font-weight: 500; gap: 9px;
  }
  .all:hover:not(:disabled) { background: var(--panel); color: var(--text); }
  .all.active { background: var(--panel-2); color: var(--text); font-weight: 600; }
  section { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .toolbar { padding: 16px 24px 4px; gap: 12px; }
  .search { position: relative; }
  .search input { width: 100%; padding-left: 36px; height: 38px; }
  .icon-in { position: absolute; left: 11px; top: 50%; transform: translateY(-50%); color: var(--dim); }
  .sort select { height: 38px; min-width: 150px; }
  .heading { padding: 14px 24px 10px; }
  .heading h2 { font-size: 17px; }
  .results { flex: 1; padding: 0 24px 20px; }
  .list { display: flex; flex-direction: column; gap: 6px; }
  .more { display: flex; justify-content: center; padding: 18px; min-height: 70px; }
  .empty { padding: 60px 16px; text-align: center; }
</style>
