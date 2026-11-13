<script lang="ts">
  import DOMPurify from "dompurify";
  import { api, formatBytes, formatCount, formatDate, RELEASE_LABEL, toAppError, type AddonDetail } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import InstallButton from "./InstallButton.svelte";

  let { modId, onclose }: { modId: number; onclose: () => void } = $props();

  let detail = $state<AddonDetail | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = modId;
    detail = null;
    error = null;
    api.detail(id).then(
      (d) => { if (id === modId) detail = d; },
      (e) => { if (id === modId) error = toAppError(e).message; },
    );
  });

  // The description is author-written HTML: strip scripts, styles and anything
  // active, and keep links inert so they can be opened in the real browser.
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

  const installed = $derived(app.installed.find((a) => a.modId === modId));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="drawer" role="dialog" aria-modal="true" aria-label="Addon details">
  <button class="close ghost" aria-label="Close" onclick={onclose}>✕</button>

  {#if error}
    <div class="pad"><p>{error}</p></div>
  {:else if !detail}
    <div class="pad muted"><span class="spinner"></span>&nbsp; Loading…</div>
  {:else}
    {@const a = detail.addon}
    <div class="head pad row">
      {#if a.thumbnail}<img class="thumb big" src={a.thumbnail} alt="" />{/if}
      <div class="grow">
        <h2>{a.name}</h2>
        <div class="muted small">by {a.author} · ⬇ {formatCount(a.downloads)} downloads</div>
        <div class="cats">{#each a.categories as c}<span class="badge">{c}</span>{/each}</div>
      </div>
    </div>

    <div class="actions pad row">
      <InstallButton modId={a.id} name={a.name} available={detail.latestFile !== null && detail.allowDistribution} />
      {#if a.websiteUrl}
        <button onclick={() => a.websiteUrl && api.openUrl(a.websiteUrl)}>View on CurseForge</button>
      {/if}
      {#if installed}<span class="muted small">Installed: {installed.displayName || installed.fileName}</span>{/if}
    </div>

    <div class="body scroll">
      <div class="pad">
        {#if !detail.allowDistribution}
          <p class="warn">The author doesn't allow downloads from third-party apps. Use "View on CurseForge" to download it.</p>
        {/if}
        {#if detail.latestFile}
          {@const f = detail.latestFile}
          <div class="file">
            <div class="row">
              <strong class="grow ellipsis">{f.displayName || f.fileName}</strong>
              <span class="badge" class:good={f.releaseType === 1}>{RELEASE_LABEL[f.releaseType] ?? "File"}</span>
            </div>
            <div class="muted small">
              {[formatDate(f.fileDate), formatBytes(f.fileLength), f.gameVersions.length ? `WoW ${f.gameVersions.join(", ")}` : ""]
                .filter(Boolean)
                .join(" · ")}
            </div>
          </div>
        {:else}
          <p class="warn">No WoW Forever file has been uploaded for this addon yet.</p>
        {/if}

        {#if detail.dependencies.length}
          <h3>Dependencies</h3>
          <ul class="deps">
            {#each detail.dependencies as d}
              <li class="row">
                <span class="grow">{d.name}</span>
                <span class="badge">{d.required ? "Required, installed automatically" : "Optional"}</span>
                {#if d.installed}<span class="badge good">Installed</span>{/if}
              </li>
            {/each}
          </ul>
        {/if}

        {#if detail.screenshots.length}
          <div class="shots">
            {#each detail.screenshots.slice(0, 6) as s}
              {#if s.thumbnailUrl}<img src={s.thumbnailUrl} alt={s.title ?? ""} loading="lazy" />{/if}
            {/each}
          </div>
        {/if}

        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="desc" onclick={onDescClick}>{@html html}</div>
      </div>
    </div>
  {/if}
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: #0007; z-index: 20; }
  .drawer {
    position: fixed; top: 0; right: 0; bottom: 0; width: min(640px, 92vw); z-index: 21;
    background: var(--bg-2); border-left: 1px solid var(--line); display: flex; flex-direction: column;
    box-shadow: -12px 0 32px #0009; animation: slide 0.16s ease-out;
  }
  @keyframes slide { from { transform: translateX(24px); opacity: 0; } }
  .close { position: absolute; top: 10px; right: 10px; color: var(--muted); }
  .pad { padding: 16px 20px; }
  .head { padding-right: 52px; align-items: flex-start; gap: 14px; }
  .big { width: 72px; height: 72px; }
  .cats { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 8px; }
  .actions { padding-top: 0; flex-wrap: wrap; border-bottom: 1px solid var(--line); }
  .body { flex: 1; user-select: text; }
  .file { padding: 10px 12px; background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); margin-bottom: 14px; }
  .warn { color: var(--accent); margin-top: 0; }
  h3 { font-size: 14px; margin: 6px 0 8px; }
  .deps { list-style: none; padding: 0; margin: 0 0 14px; display: grid; gap: 6px; }
  .shots { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; margin-bottom: 14px; }
  .shots img { width: 100%; aspect-ratio: 16 / 10; object-fit: cover; border-radius: 6px; background: var(--panel); }
  .desc { overflow-wrap: anywhere; }
  .desc :global(img) { max-width: 100%; height: auto; }
  .desc :global(h1), .desc :global(h2), .desc :global(h3) { margin: 14px 0 6px; font-size: 15px; }
  .desc :global(table) { border-collapse: collapse; max-width: 100%; }
  .desc :global(td), .desc :global(th) { border: 1px solid var(--line); padding: 4px 6px; }
  .desc :global(pre), .desc :global(code) { background: var(--panel); border-radius: 4px; padding: 1px 4px; white-space: pre-wrap; }
</style>
