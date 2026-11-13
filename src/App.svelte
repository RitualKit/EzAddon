<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./lib/api";
  import { app, checkUpdates, dismissToast, init, showError } from "./lib/state.svelte";
  import Browse from "./components/Browse.svelte";
  import Installed from "./components/Installed.svelte";
  import Settings from "./components/Settings.svelte";
  import Setup from "./components/Setup.svelte";
  import AddonDetail from "./components/AddonDetail.svelte";
  import Icon from "./components/Icon.svelte";

  type View = "installed" | "browse" | "settings";
  let view = $state<View>("browse");
  let ready = $state(false);

  const needsSetup = $derived(!!app.settings && (!app.settings.hasApiKey || !app.settings.addonsPathValid));
  const updateCount = $derived(Object.keys(app.updates).length);

  function go(v: View) {
    view = v;
    app.detailId = null;
  }

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
  <!-- Left icon rail -->
  <nav class="rail">
    <img class="logo" src="/icon.png" alt="EzAddon" width="34" height="34" />
    {#if ready && !needsSetup}
      <button class="rail-btn" class:active={view !== "settings"} title="World of Warcraft: Forever" onclick={() => go("browse")}>
        <span class="game-tile">WF</span>
      </button>
    {/if}
    <div class="grow"></div>
    {#if ready && !needsSetup}
      <button class="rail-btn" class:active={view === "settings"} title="Settings" onclick={() => go("settings")}>
        <Icon name="gear" size={20} />
      </button>
    {/if}
  </nav>

  <div class="main">
    {#if !ready}
      <div class="center muted"><span class="spinner"></span></div>
    {:else if needsSetup}
      <Setup />
    {:else if view === "settings"}
      <Settings />
    {:else}
      <!-- Game header, like a launcher's game page -->
      <header class="game-head">
        <div class="title row">
          <div class="game-art">WF</div>
          <div>
            <h1>World of Warcraft</h1>
            <div class="row small">
              <span class="tag accent">Forever</span>
              <span class="muted ellipsis" title={app.settings?.addonsPath ?? ""}>{app.settings?.addonsPath}</span>
            </div>
          </div>
          <div class="grow"></div>
          <button class="outline" onclick={() => api.openAddonsFolder().catch(showError)}>
            <Icon name="folder" size={16} /> Open Folder
          </button>
        </div>
        <div class="tabs">
          <button class="tab" class:active={view === "installed"} onclick={() => go("installed")}>
            My Addons <span class="count">{app.installed.length}</span>
            {#if updateCount}<span class="dot" title="{updateCount} update(s) available"></span>{/if}
          </button>
          <button class="tab" class:active={view === "browse"} onclick={() => go("browse")}>Browse</button>
        </div>
      </header>

      <div class="body">
        <!-- Browse stays mounted so results and scroll position survive tab switches. -->
        <div class="view" class:hidden={view !== "browse" || app.detailId !== null}><Browse /></div>
        {#if view === "installed"}<div class="view" class:hidden={app.detailId !== null}><Installed /></div>{/if}
        {#if app.detailId !== null}
          <div class="view"><AddonDetail modId={app.detailId} onclose={() => (app.detailId = null)} /></div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="toasts">
    {#each app.toasts as t (t.id)}
      <div class="toast {t.kind}">
        <span class="grow">{t.text}</span>
        {#if t.action}
          <button class="outline" onclick={() => { t.action?.run(); dismissToast(t.id); }}>{t.action.label}</button>
        {/if}
        <button class="icon" aria-label="Dismiss" onclick={() => dismissToast(t.id)}><Icon name="close" size={14} /></button>
      </div>
    {/each}
  </div>
</div>

<style>
  .shell { display: flex; height: 100%; }
  .rail {
    width: 64px; flex: none; background: var(--rail); border-right: 1px solid #1c1c21;
    display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 12px 0;
  }
  .logo { border-radius: 8px; margin-bottom: 8px; }
  .rail-btn {
    width: 44px; height: 44px; padding: 0; border-radius: 10px; background: transparent; color: var(--muted);
    position: relative;
  }
  .rail-btn:hover:not(:disabled) { background: var(--panel); color: var(--text); }
  .rail-btn.active { background: var(--panel-2); color: var(--text); }
  .rail-btn.active::before {
    content: ""; position: absolute; left: -10px; top: 10px; bottom: 10px; width: 3px;
    border-radius: 0 3px 3px 0; background: var(--accent);
  }
  .game-tile {
    width: 34px; height: 34px; border-radius: 8px; display: grid; place-items: center;
    font-size: 12px; font-weight: 800; letter-spacing: 0.5px; color: #fff;
    background: linear-gradient(135deg, #2f5fa8, #6a2fa8);
  }

  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .center { height: 100%; display: grid; place-items: center; }

  .game-head {
    padding: 18px 24px 0;
    background:
      radial-gradient(900px 160px at 0% 0%, #2a3a66 0%, transparent 70%),
      linear-gradient(180deg, #1a1a20, var(--bg));
    border-bottom: 1px solid var(--line);
  }
  .title { gap: 14px; }
  .game-art {
    width: 52px; height: 52px; border-radius: 8px; display: grid; place-items: center;
    font-weight: 800; font-size: 17px; color: #fff; background: linear-gradient(135deg, #2f5fa8, #6a2fa8);
    box-shadow: 0 4px 14px #0007;
  }
  h1 { font-size: 22px; line-height: 1.2; margin-bottom: 3px; }
  .title .muted { max-width: 46vw; }
  .tabs { display: flex; gap: 22px; margin-top: 14px; }
  .tab {
    background: transparent; border-radius: 0; padding: 10px 2px 11px; color: var(--muted);
    border-bottom: 2px solid transparent; font-size: 14px;
  }
  .tab:hover:not(:disabled) { background: transparent; color: var(--text); }
  .tab.active { color: var(--text); border-bottom-color: var(--accent); }
  .count { font-size: 11px; color: var(--muted); background: var(--panel-2); border-radius: 10px; padding: 0 7px; }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); }

  .body { flex: 1; min-height: 0; position: relative; }
  .view { position: absolute; inset: 0; }
  .hidden { visibility: hidden; pointer-events: none; }

  .toasts { position: fixed; right: 18px; bottom: 18px; display: flex; flex-direction: column; gap: 8px; z-index: 50; max-width: 440px; }
  .toast {
    display: flex; align-items: center; gap: 10px; padding: 10px 10px 10px 14px;
    background: var(--panel); border: 1px solid var(--line); border-left: 3px solid var(--info);
    border-radius: var(--radius); box-shadow: 0 10px 30px #000a; user-select: text;
  }
  .toast.success { border-left-color: var(--good); }
  .toast.error { border-left-color: var(--bad); }
</style>
