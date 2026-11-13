<script lang="ts">
  import { app, install } from "../lib/state.svelte";

  let { modId, name, available = true }: { modId: number; name: string; available?: boolean } = $props();

  const installed = $derived(app.installed.find((a) => a.modId === modId));
  const hasUpdate = $derived(modId in app.updates);
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
  <button class="primary" disabled title={busy.name ? `${busy.name} (${busy.step}/${busy.steps})` : ""}>
    <span class="spinner"></span> {label()}
  </button>
{:else if installed && hasUpdate}
  <button class="primary" onclick={(e) => { e.stopPropagation(); install(modId, name); }}>Update</button>
{:else if installed}
  <button disabled class="done">✓ Installed</button>
{:else if !available}
  <button disabled title="No WoW Forever version is available">Unavailable</button>
{:else}
  <button class="primary" onclick={(e) => { e.stopPropagation(); install(modId, name); }}>Install</button>
{/if}

<style>
  button { min-width: 96px; display: inline-flex; align-items: center; justify-content: center; gap: 6px; }
  .done { color: var(--good); opacity: 1; }
</style>
