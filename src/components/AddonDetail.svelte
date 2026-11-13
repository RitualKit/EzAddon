<script lang="ts" module>
  import DOMPurify from "dompurify";

  // Images in descriptions load lazily and decode off the main thread.
  DOMPurify.addHook("afterSanitizeAttributes", (node) => {
    if (node.tagName === "IMG") {
      node.setAttribute("loading", "lazy");
      node.setAttribute("decoding", "async");
    }
  });
</script>

<script lang="ts">
  import { api, formatBytes, formatCount, formatDate, RELEASE_LABEL, toAppError, type AddonDetail } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import InstallButton from "./InstallButton.svelte";
  import Icon from "./Icon.svelte";

  let { modId, onclose }: { modId: number; onclose: () => void } = $props();

  // Raw state: the detail is replaced, never mutated, so it needn't be deeply reactive.
  let detail = $state.raw<AddonDetail | null>(null);
  let error = $state<string | null>(null);
  let tab = $state<"description" | "files" | "images">("description");

  $effect(() => {
    const id = modId;
    detail = null;
    error = null;
    tab = "description";
    api.detail(id).then(
      (d) => { if (id === modId) detail = d; },
      (e) => { if (id === modId) error = toAppError(e).message; },
    );
  });

  // The description is author-written HTML: strip scripts, styles and anything
  // active, and keep links inert so they open in the real browser.
  const html = $derived(
    detail
      ? DOMPurify.sanitize(detail.descriptionHtml, {
          FORBID_TAGS: ["style", "iframe", "form", "input", "video", "audio", "object", "embed"],
          FORBID_ATTR: ["style", "class", "id"],
        })
      : "",
  );

  function onDescClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href");
    if (href && /^https?:\/\//i.test(href)) api.openUrl(href);
  }

  const installed = $derived(app.installedById[modId]);
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="page">
  <div class="topbar">
    <button class="ghost back" onclick={onclose}><Icon name="back" size={16} /> Back</button>
  </div>

  {#if error}
    <div class="pad"><p>{error}</p><button class="outline" onclick={onclose}>Back</button></div>
  {:else if !detail}
    <div class="pad muted"><span class="spinner"></span></div>
  {:else}
    {@const a = detail.addon}
    <div class="hero">
      {#if a.thumbnail}<img class="logo" src={a.thumbnail} alt="" />{:else}<div class="logo"></div>{/if}
      <div class="grow">
        <h1>{a.name}</h1>
        <div class="by">by <span>{a.author}</span></div>
        <div class="metas">
          <span class="meta"><Icon name="download" size={13} /> {formatCount(a.downloads)} downloads</span>
          {#if a.dateModified}<span class="meta"><Icon name="clock" size={13} /> Updated {formatDate(a.dateModified)}</span>{/if}
          {#if a.gameVersion}<span class="meta"><Icon name="game" size={13} /> {a.gameVersion}</span>{/if}
        </div>
        <div class="cats">{#each a.categories as c}<span class="tag">{c}</span>{/each}</div>
      </div>
      <div class="cta">
        <InstallButton
          modId={a.id}
          name={a.name}
          available={detail.latestFile !== null}
          allowDistribution={detail.allowDistribution}
          websiteUrl={a.websiteUrl}
          large
        />
        {#if a.websiteUrl}
          <button class="outline" onclick={() => a.websiteUrl && api.openUrl(a.websiteUrl)}>
            <Icon name="external" size={14} /> CurseForge page
          </button>
        {/if}
        {#if installed}<div class="dim small">Installed: {installed.displayName || installed.fileName}</div>{/if}
      </div>
    </div>

    <div class="tabs">
      <button class="tab" class:active={tab === "description"} onclick={() => (tab = "description")}>Description</button>
      <button class="tab" class:active={tab === "files"} onclick={() => (tab = "files")}>Files</button>
      {#if detail.screenshots.length}
        <button class="tab" class:active={tab === "images"} onclick={() => (tab = "images")}>Images <span class="dim">{detail.screenshots.length}</span></button>
      {/if}
    </div>

    <div class="content scroll">
      <div class="inner">
        {#if !detail.allowDistribution}
          <p class="warn">
            The author only allows downloads from the CurseForge website. Download it there and extract it into your
            AddOns folder, then use <strong>Scan for addons</strong> in My Addons so EzAddon tracks it and tells you about
            updates.
          </p>
        {/if}

        {#if tab === "description"}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="desc" onclick={onDescClick}>{@html html}</div>
        {:else if tab === "files"}
          {#if detail.latestFile}
            {@const f = detail.latestFile}
            <div class="file-table">
              <div class="th"><span>Type</span><span>Name</span><span>Uploaded</span><span>Size</span><span>Game Version</span></div>
              <div class="tr">
                <span><span class="rt rt{f.releaseType}">{(RELEASE_LABEL[f.releaseType] ?? "?")[0]}</span></span>
                <span class="ellipsis" title={f.fileName}>{f.displayName || f.fileName}</span>
                <span>{formatDate(f.fileDate)}</span>
                <span>{formatBytes(f.fileLength)}</span>
                <span>{f.gameVersions.join(", ") || "Forever"}</span>
              </div>
            </div>
            <p class="dim small">This is the file EzAddon installs for WoW Forever with your release channel setting.</p>
          {:else}
            <p class="warn">No WoW Forever file has been uploaded for this addon yet.</p>
          {/if}

          {#if detail.dependencies.length}
            <h3>Dependencies</h3>
            <div class="deps">
              {#each detail.dependencies as d}
                <div class="dep row">
                  <Icon name="puzzle" size={15} />
                  <span class="grow">{d.name}</span>
                  <span class="tag">{d.required ? "Required, installed automatically" : "Optional"}</span>
                  {#if d.installed}<span class="tag good">Installed</span>{/if}
                </div>
              {/each}
            </div>
          {/if}
        {:else}
          <div class="shots">
            {#each detail.screenshots as s}
              {#if s.thumbnailUrl}
                <button class="shot" onclick={() => s.url && api.openUrl(s.url)} title={s.title ?? ""}>
                  <img src={s.thumbnailUrl} alt={s.title ?? ""} loading="lazy" />
                </button>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .page { height: 100%; display: flex; flex-direction: column; background: var(--bg); }
  .topbar { padding: 10px 16px 0; }
  .back { color: var(--muted); padding: 6px 10px; }
  .back:hover:not(:disabled) { color: var(--text); }
  .pad { padding: 24px; }
  .hero { display: flex; gap: 18px; padding: 10px 28px 18px; align-items: flex-start; }
  .logo { width: 96px; height: 96px; border-radius: 6px; object-fit: cover; background: var(--panel-2); flex: none; box-shadow: 0 6px 18px #0008; }
  h1 { font-size: 24px; line-height: 1.2; }
  .by { color: var(--dim); font-size: 13px; margin: 2px 0 8px; }
  .by span { color: var(--muted); }
  .metas { display: flex; gap: 16px; flex-wrap: wrap; }
  .cats { display: flex; gap: 6px; flex-wrap: wrap; margin-top: 10px; }
  .cta { display: flex; flex-direction: column; gap: 8px; align-items: stretch; min-width: 170px; }
  .tabs { display: flex; gap: 22px; padding: 0 28px; border-bottom: 1px solid var(--line); }
  .tab { background: transparent; border-radius: 0; padding: 10px 2px 11px; color: var(--muted); border-bottom: 2px solid transparent; }
  .tab:hover:not(:disabled) { background: transparent; color: var(--text); }
  .tab.active { color: var(--text); border-bottom-color: var(--accent); }
  .content { flex: 1; user-select: text; }
  .inner { padding: 20px 28px 32px; max-width: 980px; }
  .warn { color: var(--accent); margin-top: 0; }
  h3 { font-size: 15px; margin: 22px 0 10px; }
  .file-table { background: var(--panel); border-radius: var(--radius); overflow: hidden; margin-bottom: 10px; }
  .th, .tr { display: grid; grid-template-columns: 60px 1fr 130px 90px 130px; gap: 12px; padding: 10px 14px; align-items: center; }
  .th { font-size: 12px; font-weight: 700; color: var(--dim); text-transform: uppercase; letter-spacing: 0.5px; background: var(--panel-2); }
  .rt { display: inline-grid; place-items: center; width: 24px; height: 24px; border-radius: 4px; font-weight: 800; font-size: 12px; color: #fff; }
  .rt1 { background: #2e9c5e; } .rt2 { background: #3b76d6; } .rt3 { background: #b9472f; }
  .deps { display: grid; gap: 6px; }
  .dep { padding: 10px 12px; background: var(--panel); border-radius: var(--radius); color: var(--muted); }
  .dep .grow { color: var(--text); }
  .shots { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 10px; }
  .shot { padding: 0; background: var(--panel); border-radius: var(--radius); overflow: hidden; }
  .shot img { width: 100%; aspect-ratio: 16 / 10; object-fit: cover; display: block; }
  .desc { overflow-wrap: anywhere; color: #d6d6db; }
  .desc :global(img) { max-width: 100%; height: auto; }
  .desc :global(h1), .desc :global(h2), .desc :global(h3) { margin: 18px 0 8px; font-size: 16px; color: var(--text); }
  .desc :global(table) { border-collapse: collapse; max-width: 100%; }
  .desc :global(td), .desc :global(th) { border: 1px solid var(--line); padding: 4px 6px; }
  .desc :global(pre), .desc :global(code) { background: var(--panel); border-radius: 4px; padding: 1px 4px; white-space: pre-wrap; }
</style>
