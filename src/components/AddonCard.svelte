<script lang="ts">
  import { formatCount, formatDate, type AddonSummary } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import InstallButton from "./InstallButton.svelte";

  let { addon }: { addon: AddonSummary } = $props();
</script>

<div
  class="card"
  role="button"
  tabindex="0"
  onclick={() => (app.detailId = addon.id)}
  onkeydown={(e) => e.key === "Enter" && (app.detailId = addon.id)}
>
  <div class="top row">
    {#if addon.thumbnail}
      <img class="thumb" src={addon.thumbnail} alt="" loading="lazy" decoding="async" width="48" height="48" />
    {:else}
      <div class="thumb"></div>
    {/if}
    <div class="grow">
      <div class="name ellipsis" title={addon.name}>{addon.name}</div>
      <div class="muted small ellipsis">by {addon.author || "unknown"}</div>
    </div>
  </div>
  <p class="summary">{addon.summary}</p>
  <div class="bottom row">
    <span class="muted small grow ellipsis">
      ⬇ {formatCount(addon.downloads)} · {formatDate(addon.dateModified)}
    </span>
    <InstallButton modId={addon.id} name={addon.name} available={addon.latestFileId !== null} />
  </div>
</div>

<style>
  .card {
    display: flex; flex-direction: column; gap: 8px; padding: 12px;
    background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius);
    cursor: pointer; min-width: 0; transition: border-color 0.12s;
  }
  .card:hover, .card:focus-visible { border-color: #57527a; outline: none; }
  .name { font-weight: 600; font-size: 15px; }
  .summary {
    margin: 0; color: var(--muted); font-size: 13px; flex: 1;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
    min-height: 2.9em;
  }
</style>
