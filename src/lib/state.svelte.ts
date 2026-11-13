// App-wide reactive state, shared by every component.
import { api, toAppError, type AppError, type InstalledAddon, type Progress, type SettingsView } from "./api";

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  text: string;
  action?: { label: string; run: () => void };
}

export const app = $state({
  settings: null as SettingsView | null,
  installed: [] as InstalledAddon[],
  /** modId → installed addon, for O(1) lookups from every list row. */
  installedById: {} as Record<number, InstalledAddon>,
  /** modId → latest file id, for addons that have an update. */
  updates: {} as Record<number, number>,
  /** modId → live install progress. */
  busy: {} as Record<number, Progress>,
  toasts: [] as Toast[],
  /** Addon whose detail drawer is open. */
  detailId: null as number | null,
});

let toastId = 0;
export function toast(kind: Toast["kind"], text: string, action?: Toast["action"], ms = 5000) {
  const id = ++toastId;
  app.toasts.push({ id, kind, text, action });
  setTimeout(() => dismissToast(id), action ? ms * 2 : ms);
}
export function dismissToast(id: number) {
  const i = app.toasts.findIndex((t) => t.id === id);
  if (i >= 0) app.toasts.splice(i, 1);
}

export function showError(e: unknown) {
  const err: AppError = toAppError(e);
  if (err.kind === "distributionDisabled" && err.url) {
    const url = err.url;
    toast("error", err.message, { label: "Open on CurseForge", run: () => api.openUrl(url) });
  } else {
    toast("error", err.message);
  }
}

function setInstalled(list: InstalledAddon[]) {
  app.installed = list;
  app.installedById = Object.fromEntries(list.map((a) => [a.modId, a]));
}

export async function refreshInstalled() {
  try {
    setInstalled(await api.listInstalled());
  } catch (e) {
    showError(e);
  }
}

export async function checkUpdates(silent = false) {
  try {
    const list = await api.checkUpdates();
    app.updates = Object.fromEntries(list.map((u) => [u.modId, u.latestFileId]));
    if (!silent) toast("info", list.length ? `${list.length} update${list.length > 1 ? "s" : ""} available.` : "Everything is up to date.");
  } catch (e) {
    if (!silent) showError(e);
  }
}

export async function install(modId: number, name: string, quiet = false): Promise<boolean> {
  if (app.busy[modId]) return false;
  app.busy[modId] = { modId, stage: "resolving", name, step: 0, steps: 0, received: 0, total: null };
  try {
    const res = await api.install(modId);
    delete app.updates[modId];
    await refreshInstalled();
    const deps = res.installed.filter((a) => a.modId !== modId).length;
    if (!quiet) toast("success", `Installed ${name}${deps ? ` and ${deps} dependenc${deps > 1 ? "ies" : "y"}` : ""}.`);
    res.warnings.forEach((w) => toast("error", w));
    return true;
  } catch (e) {
    showError(e);
    return false;
  } finally {
    delete app.busy[modId];
  }
}

export async function uninstall(modId: number, name: string) {
  try {
    setInstalled(await api.uninstall(modId));
    delete app.updates[modId];
    toast("info", `Removed ${name}.`);
  } catch (e) {
    showError(e);
  }
}

export async function updateAll() {
  const ids = Object.keys(app.updates).map(Number);
  let ok = 0;
  for (const id of ids) {
    if (await install(id, app.installedById[id]?.name ?? `Addon #${id}`, true)) ok++;
  }
  toast("success", `Updated ${ok} of ${ids.length} addon${ids.length > 1 ? "s" : ""}.`);
}

let listening = false;
export async function init() {
  if (!listening) {
    listening = true;
    await api.onProgress((p) => {
      if (app.busy[p.modId] && p.stage !== "done") app.busy[p.modId] = p;
    });
    await api.onUpdates((list) => {
      app.updates = Object.fromEntries(list.map((u) => [u.modId, u.latestFileId]));
    });
  }
  app.settings = await api.getSettings();
  if (app.settings.addonsPathValid) await refreshInstalled();
}
