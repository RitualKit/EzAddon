<script lang="ts">
  import { api, toAppError, type KeyCheck, type Probe } from "../lib/api";
  import { app, refreshInstalled, toast } from "../lib/state.svelte";

  let { setup = false }: { setup?: boolean } = $props();

  // Form fields start from the saved settings.
  let key = $state("");
  let keyBusy = $state(false);
  let keyError = $state<string | null>(null);
  /** Result of the last key test, shown in a box below the field. */
  let check = $state.raw<KeyCheck | null>(null);
  /** True when the check failed and the key was not saved (offer "Save anyway"). */
  let unsaved = $state(false);
  let path = $state(app.settings?.addonsPath ?? "");
  let channel = $state(app.settings?.releaseChannel ?? 1);
  let override = $state(app.settings?.versionTypeOverride?.toString() ?? "");
  let closeToTray = $state(app.settings?.closeToTray ?? true);
  let notifyUpdates = $state(app.settings?.notifyUpdates ?? true);
  let pathError = $state<string | null>(null);

  async function saveKey(force = false) {
    keyBusy = true;
    keyError = null;
    check = null;
    unsaved = false;
    try {
      const res = await api.setApiKey(key, force);
      app.settings = res.settings;
      key = "";
      const ok = !res.check || (res.check.general.ok && res.check.wow.ok);
      if (!ok) check = res.check;
      toast(ok ? "success" : "info", !app.settings.hasApiKey ? "API key removed." : ok ? "API key saved." : "API key saved, but CurseForge reported a problem.");
    } catch (e) {
      const err = toAppError(e);
      if (err.kind === "keyCheckFailed" && err.check) {
        check = err.check;
        unsaved = true;
      } else {
        keyError = err.message;
      }
    } finally {
      keyBusy = false;
    }
  }

  let testing = $state(false);
  async function testKey() {
    keyBusy = testing = true;
    keyError = null;
    check = null;
    unsaved = false;
    try {
      check = await api.testApiKey();
    } catch (e) {
      keyError = toAppError(e).message;
    } finally {
      keyBusy = testing = false;
    }
  }

  function probeLine(label: string, p: Probe): string {
    const result = p.ok ? `OK (HTTP ${p.status})` : p.status ? `failed, HTTP ${p.status}` : "failed, no response";
    return `${label} [GET ${p.endpoint}]: ${result}${p.detail ? `, ${p.detail}` : ""}`;
  }

  async function copyCheck() {
    if (!check) return;
    const text = [
      "EzAddon API key check",
      probeLine("General API access", check.general),
      probeLine("World of Warcraft addons", check.wow),
      `Hint: ${check.hint}`,
    ].join("\n");
    try {
      await navigator.clipboard.writeText(text);
      toast("success", "Details copied.");
    } catch {
      toast("error", "Couldn't copy to the clipboard.");
    }
  }

  async function save(quiet = false) {
    pathError = null;
    const ov = override.trim() ? Number(override.trim()) : null;
    try {
      app.settings = await api.saveSettings(
        path.trim() || null,
        channel,
        ov !== null && Number.isFinite(ov) ? ov : null,
        closeToTray,
        notifyUpdates,
      );
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
    {#if !setup}<h1>Settings</h1>{/if}
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
        <button class="primary" disabled={keyBusy || !key.trim()} onclick={() => saveKey()}>
          {#if keyBusy && !testing}<span class="spinner"></span>{/if} Save key
        </button>
        {#if app.settings?.hasApiKey}
          <button class="outline" disabled={keyBusy} onclick={testKey}>
            {#if testing}<span class="spinner"></span>{/if} Test key
          </button>
        {/if}
        {#if app.settings?.hasApiKey && !setup}
          <button class="outline danger" disabled={keyBusy} onclick={() => { key = ""; saveKey(); }}>Remove</button>
        {/if}
      </div>
      {#if keyError}<p class="err">{keyError}</p>{/if}
      {#if check}
        {@const ok = check.general.ok && check.wow.ok}
        <div class="check" class:bad={!ok}>
          {#each [["General API access", check.general], ["World of Warcraft addons", check.wow]] as [label, p]}
            {@const pr = p as Probe}
            <div class="probe">
              <span class={pr.ok ? "pass" : "fail"}>{pr.ok ? "✓" : "✗"}</span>
              <div class="grow">
                <div>
                  <strong>{label}</strong>
                  <span class="muted small">{pr.ok ? `HTTP ${pr.status}` : pr.status ? `HTTP ${pr.status}` : "no response"}</span>
                </div>
                {#if pr.detail}<div class="dim small detail">{pr.detail}</div>{/if}
              </div>
            </div>
          {/each}
          <p class="hint">{check.hint}</p>
          <div class="row">
            {#if unsaved}
              <button class="outline" disabled={keyBusy} onclick={() => saveKey(true)}>Save anyway</button>
            {/if}
            {#if !ok}<button class="outline" onclick={copyCheck}>Copy details</button>{/if}
            <button class="ghost" onclick={() => (check = null)}>Dismiss</button>
          </div>
        </div>
      {/if}
      {#if app.settings?.hasApiKey && !check}<p class="ok small">✓ API key is set</p>{/if}
    </section>

    <section>
      <h2>WoW Forever folder</h2>
      <p class="muted">Addons are installed into <code>World of Warcraft\_forever_\Interface\AddOns</code>.</p>
      <div class="row">
        <input class="grow" placeholder="C:\Program Files (x86)\World of Warcraft" bind:value={path} onchange={() => save(true)} />
        <button class="outline" onclick={detect}>Detect</button>
        <button class="outline" onclick={browse}>Browse…</button>
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
        <h2>System tray</h2>
        <label class="toggle">
          <input type="checkbox" bind:checked={closeToTray} onchange={() => save()} />
          <span>
            <strong>Close to tray</strong>
            <span class="muted small">
              Closing the window keeps EzAddon in the system tray, using only a few MB. Quit from the tray menu.
            </span>
          </span>
        </label>
        <label class="toggle">
          <input type="checkbox" bind:checked={notifyUpdates} onchange={() => save()} />
          <span>
            <strong>Notify me about addon updates</strong>
            <span class="muted small">Checks every 6 hours in the background and shows a Windows notification.</span>
          </span>
        </label>
      </section>

      <section>
        <h2>Advanced</h2>
        <p class="muted small">
          CurseForge's WoW Forever game-version id. Leave it empty to look it up automatically.
          Change it only if search shows addons for the wrong game version.
        </p>
        <div class="row">
          <input placeholder="Automatic" inputmode="numeric" bind:value={override} style="width: 160px" />
          <button class="outline" onclick={() => save()}>Save</button>
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .settings { height: 100%; padding: 28px 24px; }
  h1 { font-size: 22px; margin-bottom: 4px; }
  .settings.setup { height: auto; padding: 0; }
  .inner { max-width: 720px; margin: 0 auto; display: grid; gap: 14px; }
  section { padding: 18px 20px; background: var(--panel); border-radius: var(--radius); }
  h2 { font-size: 15px; margin-bottom: 4px; }
  p { margin: 4px 0 10px; }
  .check { margin-top: 12px; padding: 12px 14px; border-radius: var(--radius); background: var(--bg-2); border: 1px solid var(--line); user-select: text; }
  .check.bad { border-color: #5a2a2a; }
  .probe { display: flex; gap: 10px; align-items: flex-start; padding: 4px 0; }
  .pass, .fail { font-weight: 800; width: 16px; flex: none; }
  .pass { color: var(--good); }
  .fail { color: var(--bad); }
  .detail { overflow-wrap: anywhere; margin-top: 2px; }
  .hint { margin: 8px 0 10px; }
  .err { color: var(--bad); margin: 8px 0 0; user-select: text; }
  .ok { color: var(--good); margin: 8px 0 0; overflow-wrap: anywhere; }
  .toggle { display: flex; gap: 12px; align-items: flex-start; padding: 8px 0; cursor: pointer; }
  .toggle input { width: 18px; height: 18px; margin: 2px 0 0; accent-color: var(--accent); flex: none; cursor: pointer; }
  .toggle > span { display: flex; flex-direction: column; gap: 2px; }
  code { background: var(--panel-2); padding: 1px 5px; border-radius: 4px; font-size: 12px; }
</style>
