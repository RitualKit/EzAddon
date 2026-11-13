<script lang="ts">
  import { formatCount, formatDate, type AddonSummary } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import InstallButton from "./InstallButton.svelte";
  import Icon from "./Icon.svelte";

  let { addon }: { addon: AddonSummary } = $props();
  const open = () => (app.detailId = addon.id);
</script>

<div class="row-item" role="button" tabindex="0" onclick={open} onkeydown={(e) => e.key === "Enter" && open()}>
  {#if addon.thumbnail}
    <img class="thumb" src={addon.thumbnail} alt="" loading="lazy" decoding="async" width="56" height="56" />
  {:else}
    <div class="thumb"></div>
  {/if}

  <div class="info grow">
    <div class="name-line">
      <span class="name ellipsis" title={addon.name}>{addon.name}</span>
      <span class="by ellipsis">by <span class="author">{addon.author || "unknown"}</span></span>
    </div>
    <div class="summary ellipsis" title={addon.summary}>{addon.summary}</div>
    <div class="metas">
      <span class="meta"><Icon name="download" size={13} /> {formatCount(addon.downloads)}</span>
      {#if addon.dateModified}<span class="meta"><Icon name="clock" size={13} /> {formatDate(addon.dateModified)}</span>{/if}
      {#if addon.gameVersion}<span class="meta"><Icon name="game" size={13} /> {addon.gameVersion}</span>{/if}
      {#each addon.categories.slice(0, 2) as c}<span class="tag">{c}</span>{/each}
    </div>
  </div>

  <div class="action">
    <InstallButton modId={addon.id} name={addon.name} available={addon.latestFileId !== null} />
  </div>
</div>

<style>
  .row-item {
    display: flex; align-items: center; gap: 14px; padding: 12px 14px;
    background: var(--panel); border-radius: var(--radius); cursor: pointer;
    border: 1px solid transparent; transition: background 0.1s, border-color 0.1s;
  }
  .row-item:hover, .row-item:focus-visible { background: #232329; border-color: #33333a; outline: none; }
  .name-line { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .name { font-weight: 700; font-size: 15px; flex: none; max-width: 60%; }
  .by { color: var(--dim); font-size: 12px; }
  .author { color: var(--muted); }
  .summary { color: var(--muted); font-size: 13px; margin: 2px 0 6px; }
  .metas { display: flex; align-items: center; gap: 14px; flex-wrap: wrap; row-gap: 4px; }
  .action { flex: none; }
</style>
