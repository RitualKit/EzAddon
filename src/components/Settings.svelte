<script lang="ts">
  import { api, toAppError } from "../lib/api";
  import { app, refreshInstalled, toast } from "../lib/state.svelte";

  let { setup = false }: { setup?: boolean } = $props();

  // Form fields start from the saved settings.
  let key = $state("");
  let keyBusy = $state(false);
  let keyError = $state<string | null>(null);
  let path = $state(app.settings?.addonsPath ?? "");
  let channel = $state(app.settings?.releaseChannel ?? 1);
  let override = $state(app.settings?.versionTypeOverride?.toString() ?? "");
  let pathError = $state<string | null>(null);

  async function saveKey() {
    keyBusy = true;
    keyError = null;
    try {
      app.settings = await api.setApiKey(key);
      key = "";
      toast("success", app.settings.hasApiKey ? "API key saved." : "API key removed.");
    } catch (e) {
      keyError = toAppError(e).message;
    } finally {
      keyBusy = false;
    }
  }

  async function save(quiet = false) {
    pathError = null;
    const ov = override.trim() ? Number(override.trim()) : null;
    try {
      app.settings = await api.saveSettings(path.trim() || null, channel, ov !== null && Number.isFinite(ov) ? ov : null);
      path = app.settings.addonsPath ?? "";
      if (path && !app.settings.addonsPathValid) {
        pathError = "No WoW Forever install was found there. Choose your World of Warcraft folder (the one that contains _forever_).";
      } else {
        await refreshInstalled();
        if (!quiet) toast("success", "Settings saved.");
      }
    } catch (e) {
      pathError = toAppError(e).message;
    }
  }

  async function detect() {
    const p = await api.detectWowPath();
    if (p) {
      path = p;
      await save(true);
    } else {
      pathError = "WoW Forever wasn't found in the usual places. Use Browse… to pick your World of Warcraft folder.";
    }
  }

  async function browse() {
    const p = await api.pickFolder();
    if (p) {
      path = p;
      await save(true);
    }
  }
</script>

<div class="settings scroll" class:setup>
  <div class="inner">
    <section>
      <h2>CurseForge API key</h2>
      <p class="muted">
        EzAddon talks to CurseForge with your own free API key. Create one at
        <a href="https://console.curseforge.com/" onclick={(e) => { e.preventDefault(); api.openUrl("https://console.curseforge.com/"); }}>console.curseforge.com</a>
        (sign in, then open <em>API keys</em>) and paste it here. It is kept in Windows Credential Manager.
      </p>
      <div class="row">
        <input
          class="grow"
          type="password"
          autocomplete="off"
          spellcheck="false"
          placeholder={app.settings?.hasApiKey ? "•••••••• (saved, paste a new key to replace it)" : "Paste your API key"}
          bind:value={key}
          onkeydown={(e) => e.key === "Enter" && key.trim() && saveKey()}
        />
        <button class="primary" disabled={keyBusy || !key.trim()} onclick={saveKey}>
          {#if keyBusy}<span class="spinner"></span>{/if} Save key
        </button>
        {#if app.settings?.hasApiKey && !setup}
          <button class="danger" disabled={keyBusy} onclick={() => { key = ""; saveKey(); }}>Remove</button>
        {/if}
      </div>
      {#if keyError}<p class="err">{keyError}</p>{/if}
      {#if app.settings?.hasApiKey}<p class="ok small">✓ API key is set</p>{/if}
    </section>

    <section>
      <h2>WoW Forever folder</h2>
      <p class="muted">Addons are installed into <code>World of Warcraft\_forever_\Interface\AddOns</code>.</p>
      <div class="row">
        <input class="grow" placeholder="C:\Program Files (x86)\World of Warcraft" bind:value={path} onchange={() => save(true)} />
        <button onclick={detect}>Detect</button>
        <button onclick={browse}>Browse…</button>
      </div>
      {#if pathError}<p class="err">{pathError}</p>{/if}
      {#if app.settings?.addonsPathValid}<p class="ok small">✓ {app.settings.addonsPath}</p>{/if}
    </section>

    {#if !setup}
      <section>
        <h2>Release channel</h2>
        <p class="muted">Which addon builds to install and offer as updates.</p>
        <select bind:value={channel} onchange={() => save()}>
          <option value={1}>Release only (recommended)</option>
          <option value={2}>Release and beta</option>
          <option value={3}>Release, beta and alpha</option>
        </select>
      </section>

      <section>
        <h2>Advanced</h2>
        <p class="muted small">
          CurseForge's WoW Forever game-version id. Leave it empty to look it up automatically.
          Change it only if search shows addons for the wrong game version.
        </p>
        <div class="row">
          <input placeholder="Automatic" inputmode="numeric" bind:value={override} style="width: 160px" />
          <button onclick={() => save()}>Save</button>
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .settings { height: 100%; padding: 20px 16px; }
  .settings.setup { height: auto; padding: 0; }
  .inner { max-width: 720px; margin: 0 auto; display: grid; gap: 14px; }
  section { padding: 16px; background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); }
  h2 { font-size: 15px; margin-bottom: 4px; }
  p { margin: 4px 0 10px; }
  .err { color: var(--bad); margin: 8px 0 0; user-select: text; }
  .ok { color: var(--good); margin: 8px 0 0; overflow-wrap: anywhere; }
  code { background: var(--bg-2); padding: 1px 5px; border-radius: 4px; font-size: 12px; }
  button { display: inline-flex; align-items: center; gap: 6px; }
</style>
