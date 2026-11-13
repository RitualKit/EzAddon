<script lang="ts">
  import { onMount } from "svelte";
  import { app, checkUpdates, dismissToast, init, showError } from "./lib/state.svelte";
  import Browse from "./components/Browse.svelte";
  import Installed from "./components/Installed.svelte";
  import Settings from "./components/Settings.svelte";
  import Setup from "./components/Setup.svelte";
  import AddonDetail from "./components/AddonDetail.svelte";

  type Tab = "browse" | "installed" | "settings";
  let tab = $state<Tab>("browse");
  let ready = $state(false);

  const needsSetup = $derived(!!app.settings && (!app.settings.hasApiKey || !app.settings.addonsPathValid));
  const updateCount = $derived(Object.keys(app.updates).length);

  onMount(async () => {
    try {
      await init();
      if (app.settings?.hasApiKey && app.settings.addonsPathValid && app.installed.length) checkUpdates(true);
    } catch (e) {
      showError(e);
    } finally {
      ready = true;
    }
  });
</script>

<div class="shell">
  <header>
    <div class="brand row">
      <img src="/icon.png" alt="" width="26" height="26" />
      <h1>EzAddon</h1>
      <span class="badge accent">WoW Forever</span>
    </div>
    {#if ready && !needsSetup}
      <nav class="row">
        <button class:active={tab === "browse"} class="ghost" onclick={() => (tab = "browse")}>Browse</button>
        <button class:active={tab === "installed"} class="ghost" onclick={() => (tab = "installed")}>
          Installed <span class="badge">{app.installed.length}</span>
          {#if updateCount}<span class="badge accent">{updateCount} update{updateCount > 1 ? "s" : ""}</span>{/if}
        </button>
        <button class:active={tab === "settings"} class="ghost" onclick={() => (tab = "settings")}>Settings</button>
      </nav>
    {/if}
  </header>

  <main>
    {#if !ready}
      <div class="center muted"><span class="spinner"></span>&nbsp; Loading…</div>
    {:else if needsSetup}
      <Setup />
    {:else}
      <!-- Browse stays mounted so search results and scroll position survive tab switches. -->
      <div class="view" class:hidden={tab !== "browse"}><Browse /></div>
      {#if tab === "installed"}<div class="view"><Installed /></div>{/if}
      {#if tab === "settings"}<div class="view"><Settings /></div>{/if}
    {/if}
  </main>

  {#if app.detailId !== null}
    <AddonDetail modId={app.detailId} onclose={() => (app.detailId = null)} />
  {/if}

  <div class="toasts">
    {#each app.toasts as t (t.id)}
      <div class="toast {t.kind}">
        <span class="grow">{t.text}</span>
        {#if t.action}
          <button onclick={() => { t.action?.run(); dismissToast(t.id); }}>{t.action.label}</button>
        {/if}
        <button class="ghost" aria-label="Dismiss" onclick={() => dismissToast(t.id)}>✕</button>
      </div>
    {/each}
  </div>
</div>

<style>
  .shell { display: flex; flex-direction: column; height: 100%; }
  header {
    display: flex; align-items: center; justify-content: space-between; gap: 16px;
    padding: 10px 16px; background: var(--bg-2); border-bottom: 1px solid var(--line);
  }
  .brand h1 { font-size: 17px; letter-spacing: 0.2px; }
  nav button { color: var(--muted); display: inline-flex; gap: 6px; align-items: center; }
  nav button.active { color: var(--text); background: var(--panel-2); }
  main { flex: 1; min-height: 0; position: relative; }
  .view { position: absolute; inset: 0; }
  .hidden { visibility: hidden; pointer-events: none; }
  .center { height: 100%; display: grid; place-items: center; }
  .toasts { position: fixed; right: 16px; bottom: 16px; display: flex; flex-direction: column; gap: 8px; z-index: 50; max-width: 440px; }
  .toast {
    display: flex; align-items: center; gap: 8px; padding: 10px 12px;
    background: var(--panel); border: 1px solid var(--line); border-left: 4px solid var(--info);
    border-radius: var(--radius); box-shadow: 0 8px 24px #0008; user-select: text;
  }
  .toast.success { border-left-color: var(--good); }
  .toast.error { border-left-color: var(--bad); }
  .toast button.ghost { padding: 2px 6px; color: var(--muted); }
</style>
