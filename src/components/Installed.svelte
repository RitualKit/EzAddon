<script lang="ts">
  import { onMount } from "svelte";
  import { api, formatDate } from "../lib/api";
  import { app, checkUpdates, refreshInstalled, uninstall, updateAll, showError } from "../lib/state.svelte";
  import InstallButton from "./InstallButton.svelte";

  let checking = $state(false);
  let updatingAll = $state(false);
  let filter = $state("");
  let confirmId = $state<number | null>(null);

  const list = $derived(
    app.installed.filter((a) => a.name.toLowerCase().includes(filter.trim().toLowerCase())),
  );
  const updateCount = $derived(Object.keys(app.updates).length);

  async function check() {
    checking = true;
    await checkUpdates();
    checking = false;
  }

  async function doUpdateAll() {
    updatingAll = true;
    await updateAll();
    updatingAll = false;
  }

  onMount(async () => {
    await refreshInstalled();
    if (app.installed.length) {
      checking = true;
      await checkUpdates(true);
      checking = false;
    }
  });
</script>

<div class="wrap">
  <div class="toolbar row">
    <input class="grow" type="search" placeholder="Filter installed addons…" bind:value={filter} />
    <button onclick={check} disabled={checking || !app.installed.length}>
      {#if checking}<span class="spinner"></span>{/if} Check for updates
    </button>
    <button class="primary" onclick={doUpdateAll} disabled={!updateCount || updatingAll}>
      {#if updatingAll}<span class="spinner"></span>{/if} Update all{updateCount ? ` (${updateCount})` : ""}
    </button>
    <button onclick={() => api.openAddonsFolder().catch(showError)} title="Open the AddOns folder">Open folder</button>
  </div>

  <div class="list scroll">
    {#if !app.installed.length}
      <div class="empty muted">
        Nothing installed with EzAddon yet. Find addons on the Browse tab.
      </div>
    {:else}
      {#each list as a (a.modId)}
        <div class="item row">
          {#if a.thumbnail}<img class="thumb" src={a.thumbnail} alt="" loading="lazy" />{:else}<div class="thumb"></div>{/if}
          <div class="grow">
            <div class="row">
              <button class="link ghost ellipsis" onclick={() => (app.detailId = a.modId)}>{a.name}</button>
              {#if a.isDependency}<span class="badge">dependency</span>{/if}
              {#if a.modId in app.updates}<span class="badge accent">update available</span>{/if}
            </div>
            <div class="muted small ellipsis" title={a.folders.join(", ")}>
              {a.displayName || a.fileName} · installed {formatDate(a.installedAt)} · {a.folders.join(", ")}
            </div>
          </div>
          {#if a.modId in app.updates}
            <InstallButton modId={a.modId} name={a.name} />
          {/if}
          {#if confirmId === a.modId}
            <button class="danger" onclick={() => { confirmId = null; uninstall(a.modId, a.name); }}>Confirm remove</button>
            <button class="ghost" onclick={() => (confirmId = null)}>Cancel</button>
          {:else}
            <button class="danger" disabled={!!app.busy[a.modId]} onclick={() => (confirmId = a.modId)}>Uninstall</button>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .wrap { height: 100%; display: flex; flex-direction: column; }
  .toolbar { padding: 12px 16px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .toolbar button { display: inline-flex; align-items: center; gap: 6px; }
  .list { flex: 1; padding: 8px 16px 16px; }
  .item { padding: 10px 12px; border: 1px solid var(--line); background: var(--panel); border-radius: var(--radius); margin-top: 8px; gap: 12px; }
  .link { padding: 0; font-weight: 600; font-size: 15px; text-align: left; max-width: 100%; }
  .link:hover { color: var(--accent); background: transparent !important; }
  .empty { padding: 60px 16px; text-align: center; }
</style>
