<script lang="ts">
  import { onMount } from "svelte";
  import { formatDate } from "../lib/api";
  import { app, checkUpdates, refreshInstalled, scanAddons, uninstall, updateAll } from "../lib/state.svelte";
  import InstallButton from "./InstallButton.svelte";
  import Icon from "./Icon.svelte";

  let checking = $state(false);
  let updatingAll = $state(false);
  let filter = $state("");
  let confirmId = $state<number | null>(null);
  let scanning = $state(false);
  let showUnmatched = $state(false);

  const list = $derived(
    app.installed.filter((a) => a.name.toLowerCase().includes(filter.trim().toLowerCase())),
  );
  // "Update All" only counts updates EzAddon can install itself.
  const updateCount = $derived(Object.keys(app.updates).filter((id) => !app.updateBlocked[Number(id)]).length);

  async function scan() {
    scanning = true;
    showUnmatched = false;
    await scanAddons();
    scanning = false;
  }

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
    <div class="search grow">
      <span class="icon-in"><Icon name="search" size={16} /></span>
      <input type="search" placeholder="Search my addons" bind:value={filter} />
    </div>
    <button class="outline" onclick={check} disabled={checking || !app.installed.length}>
      {#if checking}<span class="spinner"></span>{:else}<Icon name="refresh" size={15} />{/if} Check for Updates
    </button>
    <button class="outline" onclick={scan} disabled={scanning} title="Find addons installed outside EzAddon and add the ones found on CurseForge">
      {#if scanning}<span class="spinner"></span>{:else}<Icon name="search" size={15} />{/if} Scan for Addons
    </button>
    <button class="primary" onclick={doUpdateAll} disabled={!updateCount || updatingAll}>
      {#if updatingAll}<span class="spinner"></span>{/if} Update All{updateCount ? ` (${updateCount})` : ""}
    </button>
  </div>

  <div class="table scroll">
    {#if app.lastScan && (app.lastScan.added.length || app.lastScan.unmatched.length)}
      {@const r = app.lastScan}
      <div class="scan-result">
        <div class="row">
          <span class="grow">
            Scanned {r.scanned} addon folder{r.scanned === 1 ? "" : "s"}:
            <strong>{r.added.length} added</strong>{#if r.unmatched.length}, {r.unmatched.length} not found on CurseForge{/if}.
          </span>
          {#if r.unmatched.length}
            <button class="ghost small" onclick={() => (showUnmatched = !showUnmatched)}>{showUnmatched ? "Hide" : "Show"} not found</button>
          {/if}
          <button class="icon" title="Dismiss" onclick={() => (app.lastScan = null)}><Icon name="close" size={14} /></button>
        </div>
        {#if showUnmatched}
          <p class="dim small unmatched">{r.unmatched.join(", ")}</p>
          <p class="dim small">These folders aren't on CurseForge, were changed by hand, or are parts of another addon. EzAddon leaves them alone.</p>
        {/if}
      </div>
    {/if}
    {#if !app.installed.length}
      <div class="empty">
        <Icon name="puzzle" size={40} stroke={1.5} />
        <p>EzAddon isn't tracking any addons yet.</p>
        <p class="dim small">Already have addons installed? Scan for them to get update checks.</p>
        <button class="primary scan-cta" onclick={scan} disabled={scanning}>
          {#if scanning}<span class="spinner"></span>{:else}<Icon name="search" size={15} />{/if} Scan for Addons
        </button>
      </div>
    {:else}
      <div class="th">
        <span>Addon</span>
        <span>Status</span>
        <span>Installed</span>
        <span></span>
      </div>
      {#each list as a (a.modId)}
        <div class="tr">
          <button class="addon" onclick={() => (app.detailId = a.modId)}>
            {#if a.thumbnail}<img class="thumb" src={a.thumbnail} alt="" loading="lazy" />{:else}<div class="thumb"></div>{/if}
            <span class="text">
              <span class="name ellipsis">{a.name}</span>
              <span class="ver dim ellipsis" title={a.folders.join(", ")}>{a.displayName || a.fileName}</span>
            </span>
          </button>
          <span>
            {#if a.modId in app.updates && app.updateBlocked[a.modId]}
              <span class="status upd" title="The author only allows downloads from the CurseForge website">Update on CurseForge</span>
            {:else if a.modId in app.updates}
              <span class="status upd">{a.fileId ? "Update available" : "Version unknown, update to track it"}</span>
            {:else}
              <span class="status ok"><Icon name="check" size={13} /> Up to date</span>
            {/if}
            {#if a.isDependency}<span class="tag">Dependency</span>{/if}
          </span>
          <span class="dim small">{formatDate(a.installedAt)}</span>
          <span class="actions">
            {#if a.modId in app.updates}<InstallButton modId={a.modId} name={a.name} websiteUrl={a.websiteUrl} />{/if}
            {#if confirmId === a.modId}
              <button class="danger outline" onclick={() => { confirmId = null; uninstall(a.modId, a.name); }}>Remove?</button>
              <button class="icon" title="Cancel" onclick={() => (confirmId = null)}><Icon name="close" size={15} /></button>
            {:else}
              <button class="icon danger" title="Uninstall" disabled={!!app.busy[a.modId]} onclick={() => (confirmId = a.modId)}>
                <Icon name="trash" size={16} />
              </button>
            {/if}
          </span>
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .wrap { height: 100%; display: flex; flex-direction: column; }
  .toolbar { padding: 16px 24px 12px; gap: 10px; }
  .search { position: relative; max-width: 420px; }
  .search input { width: 100%; padding-left: 36px; height: 38px; }
  .icon-in { position: absolute; left: 11px; top: 50%; transform: translateY(-50%); color: var(--dim); }
  .toolbar button { height: 38px; }
  .toolbar .search + button { margin-left: auto; }
  .table { flex: 1; padding: 0 24px 20px; }
  .th, .tr { display: grid; grid-template-columns: minmax(0, 1fr) 210px 120px 170px; gap: 14px; align-items: center; }
  .th {
    padding: 8px 14px; font-size: 12px; font-weight: 700; color: var(--dim); text-transform: uppercase; letter-spacing: 0.6px;
    position: sticky; top: 0; background: var(--bg); z-index: 1;
  }
  .tr {
    padding: 10px 14px; background: var(--panel); border-radius: var(--radius); margin-bottom: 6px;
    content-visibility: auto; contain-intrinsic-size: auto 64px;
  }
  .tr:hover { background: #232329; }
  .addon { justify-content: flex-start; gap: 12px; background: transparent !important; padding: 0; min-width: 0; text-align: left; }
  .addon .thumb { width: 44px; height: 44px; }
  .text { display: flex; flex-direction: column; min-width: 0; }
  .name { font-weight: 700; }
  .addon:hover .name { color: var(--accent); }
  .ver { font-size: 12px; font-weight: 400; }
  .status { display: inline-flex; align-items: center; gap: 5px; font-size: 13px; font-weight: 600; margin-right: 6px; }
  .status.ok { color: var(--good); }
  .status.upd { color: var(--accent); }
  .actions { display: flex; justify-content: flex-end; align-items: center; gap: 6px; }
  .scan-result { margin: 4px 0 10px; padding: 10px 14px; border-radius: var(--radius); background: var(--panel); border-left: 3px solid var(--accent); }
  .unmatched { margin: 8px 0 4px; overflow-wrap: anywhere; user-select: text; }
  .scan-cta { margin-top: 16px; }
  .empty { display: flex; flex-direction: column; align-items: center; padding: 70px 16px; color: var(--dim); text-align: center; }
  .empty p { margin: 8px 0 0; color: var(--muted); }
  .empty p.dim { color: var(--dim); margin-top: 2px; }
</style>
