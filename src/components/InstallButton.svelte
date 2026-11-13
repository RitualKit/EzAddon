<script lang="ts">
  import { api } from "../lib/api";
  import { app, install } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let {
    modId,
    name,
    available = true,
    large = false,
    allowDistribution = true,
    websiteUrl = null,
  }: {
    modId: number;
    name: string;
    available?: boolean;
    large?: boolean;
    /** False when the author only allows downloads from the CurseForge website. */
    allowDistribution?: boolean;
    websiteUrl?: string | null;
  } = $props();

  const installed = $derived(app.installedById[modId]);
  const hasUpdate = $derived(modId in app.updates);
  // Blocked if the search result says so, or the update check found it out.
  const blocked = $derived(!allowDistribution || !!app.updateBlocked[modId]);
  const url = $derived(websiteUrl ?? installed?.websiteUrl ?? null);
  const BLOCKED_TIP = "The author only allows downloads from the CurseForge website. Download it there, then use Scan for addons in My Addons to track it.";

  function openSite(e: MouseEvent) {
    e.stopPropagation();
    if (url) api.openUrl(url);
  }
  const busy = $derived(app.busy[modId]);
  const pct = $derived(busy?.total ? Math.round((busy.received / busy.total) * 100) : null);

  function label(): string {
    if (!busy) return "";
    if (busy.stage === "downloading") return pct !== null ? `${pct}%` : "Downloading";
    if (busy.stage === "extracting") return "Installing";
    return "Preparing";
  }
</script>

{#if busy}
  <button class="primary busy" class:large disabled title={busy.name ? `${busy.name} (${busy.step}/${busy.steps})` : ""}>
    {#if pct !== null}<span class="bar" style:width="{pct}%"></span>{/if}
    <span class="lbl"><span class="spinner"></span> {label()}</span>
  </button>
{:else if installed && hasUpdate && blocked}
  <button class="outline site" class:large title={BLOCKED_TIP} disabled={!url} onclick={openSite}>
    <Icon name="external" size={14} /> Update on CurseForge
  </button>
{:else if installed && hasUpdate}
  <button class="primary" class:large onclick={(e) => { e.stopPropagation(); install(modId, name); }}>
    <Icon name="refresh" size={14} /> Update
  </button>
{:else if installed}
  <button class="done" class:large disabled><Icon name="check" size={15} /> Installed</button>
{:else if !available}
  <button class:large disabled title="No WoW Forever version is available">Unavailable</button>
{:else if blocked}
  <button class="outline site" class:large title={BLOCKED_TIP} disabled={!url} onclick={openSite}>
    <Icon name="external" size={14} /> CurseForge
  </button>
{:else}
  <button class="primary" class:large onclick={(e) => { e.stopPropagation(); install(modId, name); }}>
    <Icon name="download" size={14} /> Install
  </button>
{/if}

<style>
  button { min-width: 112px; height: 34px; position: relative; overflow: hidden; }
  button.large { height: 40px; width: 100%; font-size: 15px; }
  .done { color: var(--good); background: #16281f; opacity: 1; }
  .site { color: var(--muted); }
  .site:hover:not(:disabled) { color: var(--text); }
  .busy { opacity: 1; background: #8a3a20; }
  .bar { position: absolute; left: 0; top: 0; bottom: 0; background: var(--accent); transition: width 0.15s; }
  .lbl { position: relative; display: inline-flex; align-items: center; gap: 6px; }
</style>
