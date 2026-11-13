// Typed wrappers around the Rust commands. In a plain browser during
// `npm run dev`, a mock backend is used so the UI can be worked on without Tauri.

import type { UnlistenFn } from "@tauri-apps/api/event";

export interface SettingsView {
  addonsPath: string | null;
  addonsPathValid: boolean;
  releaseChannel: number;
  versionTypeOverride: number | null;
  hasApiKey: boolean;
}

export interface CategoryNode {
  id: number;
  name: string;
  iconUrl: string | null;
  children: CategoryNode[];
}

export interface AddonSummary {
  id: number;
  name: string;
  summary: string;
  author: string;
  downloads: number;
  thumbnail: string | null;
  categories: string[];
  websiteUrl: string | null;
  latestFileId: number | null;
  gameVersion: string | null;
  dateModified: string | null;
}

export interface SearchPage {
  addons: AddonSummary[];
  pagination: { index: number; pageSize: number; resultCount: number; totalCount: number };
  hasMore: boolean;
}

export interface Asset {
  title: string | null;
  thumbnailUrl: string | null;
  url: string | null;
}

export interface FileInfo {
  id: number;
  displayName: string;
  fileName: string;
  fileDate: string;
  releaseType: number;
  fileLength: number;
  gameVersions: string[];
}

export interface DependencyInfo {
  modId: number;
  name: string;
  required: boolean;
  installed: boolean;
}

export interface AddonDetail {
  addon: AddonSummary;
  screenshots: Asset[];
  descriptionHtml: string;
  latestFile: FileInfo | null;
  dependencies: DependencyInfo[];
  allowDistribution: boolean;
}

export interface InstalledAddon {
  modId: number;
  name: string;
  fileId: number;
  fileName: string;
  displayName: string;
  folders: string[];
  installedAt: number;
  thumbnail: string | null;
  websiteUrl: string | null;
  isDependency: boolean;
}

export interface InstallResult {
  installed: InstalledAddon[];
  warnings: string[];
}

export interface UpdateInfo {
  modId: number;
  latestFileId: number;
  latestFileName: string;
}

export interface Progress {
  modId: number;
  stage: "resolving" | "downloading" | "extracting" | "done";
  name: string;
  step: number;
  steps: number;
  received: number;
  total: number | null;
}

export interface AppError {
  kind: string;
  message: string;
  url: string | null;
}

/** Sort options match CurseForge's ModsSearchSortField values. */
export const SORTS = [
  { id: 2, label: "Popularity" },
  { id: 6, label: "Total downloads" },
  { id: 3, label: "Last updated" },
  { id: 11, label: "Newest" },
  { id: 4, label: "Name" },
] as const;

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = (cb: (p: Progress) => void) => Promise<UnlistenFn>;

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let backend: Promise<{ invoke: Invoke; listenProgress: Listen; openUrl: (u: string) => Promise<void>; pickFolder: () => Promise<string | null> }>;

// `import.meta.env.DEV` is a build-time constant, so the mock is dropped from
// production builds.
if (import.meta.env.DEV && !isTauri) {
  backend = import("./mock").then((m) => m.mockBackend);
} else {
  backend = (async () => {
    const [{ invoke }, { listen }, opener, dialog] = await Promise.all([
      import("@tauri-apps/api/core"),
      import("@tauri-apps/api/event"),
      import("@tauri-apps/plugin-opener"),
      import("@tauri-apps/plugin-dialog"),
    ]);
    return {
      invoke: invoke as Invoke,
      listenProgress: ((cb) => listen<Progress>("install-progress", (e) => cb(e.payload))) as Listen,
      openUrl: (u: string) => opener.openUrl(u),
      pickFolder: async () => {
        const r = await dialog.open({ directory: true, multiple: false, title: "Choose your World of Warcraft folder" });
        return typeof r === "string" ? r : null;
      },
    };
  })();
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const b = await backend;
  try {
    return await b.invoke<T>(cmd, args);
  } catch (e) {
    throw toAppError(e);
  }
}

export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "kind" in e && "message" in e) return e as AppError;
  return { kind: "other", message: String(e), url: null };
}

export const api = {
  getSettings: () => call<SettingsView>("get_settings"),
  saveSettings: (addonsPath: string | null, releaseChannel: number, versionTypeOverride: number | null) =>
    call<SettingsView>("save_settings", { addonsPath, releaseChannel, versionTypeOverride }),
  setApiKey: (key: string) => call<SettingsView>("set_api_key", { key }),
  detectWowPath: () => call<string | null>("detect_wow_path"),
  openAddonsFolder: () => call<void>("open_addons_folder"),
  getCategories: (refresh = false) => call<CategoryNode[]>("get_categories", { refresh }),
  search: (query: string, categoryId: number | null, sortField: number, index: number) =>
    call<SearchPage>("search_addons", { query, categoryId, sortField, index }),
  detail: (modId: number) => call<AddonDetail>("get_addon_detail", { modId }),
  install: (modId: number) => call<InstallResult>("install_addon", { modId }),
  uninstall: (modId: number) => call<InstalledAddon[]>("uninstall_addon", { modId }),
  listInstalled: () => call<InstalledAddon[]>("list_installed"),
  checkUpdates: () => call<UpdateInfo[]>("check_updates"),
  onProgress: async (cb: (p: Progress) => void) => (await backend).listenProgress(cb),
  openUrl: async (url: string) => (await backend).openUrl(url),
  pickFolder: async () => (await backend).pickFolder(),
};

export function formatCount(n: number): string {
  if (n >= 1e9) return (n / 1e9).toFixed(1).replace(/\.0$/, "") + "B";
  if (n >= 1e6) return (n / 1e6).toFixed(1).replace(/\.0$/, "") + "M";
  if (n >= 1e3) return (n / 1e3).toFixed(1).replace(/\.0$/, "") + "K";
  return String(n);
}

export function formatDate(iso: string | null | number): string {
  if (iso === null || iso === "") return "";
  const d = typeof iso === "number" ? new Date(iso * 1000) : new Date(iso);
  return isNaN(d.getTime()) ? "" : d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export function formatBytes(n: number): string {
  if (n >= 1 << 20) return (n / (1 << 20)).toFixed(1) + " MB";
  if (n >= 1 << 10) return Math.round(n / (1 << 10)) + " KB";
  return n + " B";
}

export const RELEASE_LABEL: Record<number, string> = { 1: "Release", 2: "Beta", 3: "Alpha" };
