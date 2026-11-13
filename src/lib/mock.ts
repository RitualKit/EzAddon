// Fake backend for `npm run dev` in a plain browser. Never part of a release build.
import type { AddonDetail, AddonSummary, CategoryNode, InstalledAddon, Progress, SettingsView } from "./api";

const names = [
  ["Questie", "The ultimate questing helper: quests on your map and minimap."],
  ["Details! Damage Meter", "Computes damage, healing and more, for groups and raids."],
  ["Bagnon", "Shows all your bags in a single frame."],
  ["WeakAuras", "A powerful, flexible framework for displaying highly customizable graphics."],
  ["Auctionator", "Makes the auction house easier to use for buyers and sellers."],
  ["Deadly Boss Mods", "Timers, warnings and sounds for every boss encounter."],
  ["Leatrix Plus", "Dozens of small quality-of-life tweaks in one addon."],
  ["OmniCC", "Cooldown count text for everything."],
  ["Bartender4", "Full action bar replacement with lots of options."],
  ["pfQuest", "Quest database and map helper."],
  ["AtlasLoot", "Browse loot tables for dungeons, raids and crafting."],
  ["Plater Nameplates", "Customizable nameplates for every situation."],
];

const pic = (seed: number) =>
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" rx="10" fill="hsl(${(seed * 47) % 360},45%,35%)"/><text x="32" y="41" font-size="26" text-anchor="middle" fill="#fff" font-family="sans-serif">${names[seed % names.length][0][0]}</text></svg>`,
  );

const addons: AddonSummary[] = Array.from({ length: 75 }, (_, i) => ({
  id: 1000 + i,
  name: names[i % names.length][0] + (i >= names.length ? ` ${Math.floor(i / names.length) + 1}` : ""),
  summary: names[i % names.length][1],
  author: ["Aldor", "Thaurissan", "Mograine", "Kel'Thuzad"][i % 4],
  downloads: Math.round(9_800_000 / (i + 1)),
  thumbnail: pic(i),
  categories: [["Quests & Leveling"], ["Combat", "Damage Meters"], ["Bags & Inventory"]][i % 3],
  websiteUrl: "https://www.curseforge.com/wow/addons/example",
  latestFileId: 5000 + i,
  gameVersion: "1.60.1",
  dateModified: new Date(Date.now() - i * 86400000).toISOString(),
}));

const categories: CategoryNode[] = [
  { id: 1, name: "Action Bars", iconUrl: null, children: [] },
  { id: 2, name: "Bags & Inventory", iconUrl: null, children: [] },
  {
    id: 3,
    name: "Combat",
    iconUrl: null,
    children: [
      { id: 31, name: "Damage Meters", iconUrl: null, children: [] },
      { id: 32, name: "Boss Encounters", iconUrl: null, children: [] },
    ],
  },
  { id: 4, name: "Map & Minimap", iconUrl: null, children: [] },
  { id: 5, name: "Quests & Leveling", iconUrl: null, children: [] },
  {
    id: 6,
    name: "Unit Frames",
    iconUrl: null,
    children: [{ id: 61, name: "Party", iconUrl: null, children: [] }],
  },
];

let settings: SettingsView = {
  addonsPath: "C:\\Program Files (x86)\\World of Warcraft\\_forever_\\Interface\\AddOns",
  addonsPathValid: true,
  releaseChannel: 1,
  versionTypeOverride: null,
  hasApiKey: true,
};
let installed: InstalledAddon[] = [
  {
    modId: 1001, name: "Details! Damage Meter", fileId: 4000, fileName: "Details-1.60.1.zip",
    displayName: "Details 1.60.1", folders: ["Details"], installedAt: Date.now() / 1000 - 86400,
    thumbnail: pic(1), websiteUrl: null, isDependency: false,
  },
];
const listeners: ((p: Progress) => void)[] = [];
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

const handlers: Record<string, (a: any) => unknown> = {
  get_settings: () => settings,
  save_settings: (a) => (settings = { ...settings, addonsPath: a.addonsPath, releaseChannel: a.releaseChannel, versionTypeOverride: a.versionTypeOverride, addonsPathValid: !!a.addonsPath }),
  set_api_key: (a) => (settings = { ...settings, hasApiKey: !!a.key }),
  detect_wow_path: () => "C:\\Program Files (x86)\\World of Warcraft\\_forever_\\Interface\\AddOns",
  open_addons_folder: () => null,
  get_categories: () => categories,
  search_addons: (a) => {
    let list = addons.filter((x) => x.name.toLowerCase().includes(String(a.query).toLowerCase()));
    if (a.categoryId) list = list.filter((_, i) => i % 3 === a.categoryId % 3);
    const page = list.slice(a.index, a.index + 30);
    return { addons: page, pagination: { index: a.index, pageSize: 30, resultCount: page.length, totalCount: list.length }, hasMore: a.index + 30 < list.length };
  },
  get_addon_detail: (a): AddonDetail => {
    const addon = addons.find((x) => x.id === a.modId)!;
    return {
      addon,
      screenshots: [{ title: "Screenshot", thumbnailUrl: pic(addon.id + 3), url: pic(addon.id + 3) }],
      descriptionHtml: `<h2>${addon.name}</h2><p>${addon.summary}</p><ul><li>Works with WoW Forever 1.60.1</li><li>Lightweight</li></ul><p><a href="https://www.curseforge.com">CurseForge page</a></p>`,
      latestFile: { id: addon.latestFileId!, displayName: `${addon.name} v1.60.1`, fileName: "addon.zip", fileDate: addon.dateModified!, releaseType: 1, fileLength: 734003, gameVersions: ["1.60.1"] },
      dependencies: addon.id % 2 ? [{ modId: 1006, name: "Ace3", required: true, installed: false }] : [],
      allowDistribution: true,
    };
  },
  install_addon: async (a) => {
    const addon = addons.find((x) => x.id === a.modId)!;
    for (let r = 0; r <= 10; r++) {
      listeners.forEach((l) => l({ modId: a.modId, stage: "downloading", name: addon.name, step: 1, steps: 1, received: r * 70000, total: 700000 }));
      await wait(80);
    }
    const entry: InstalledAddon = { modId: addon.id, name: addon.name, fileId: addon.latestFileId!, fileName: "addon.zip", displayName: addon.name, folders: [addon.name.replace(/\W/g, "")], installedAt: Date.now() / 1000, thumbnail: addon.thumbnail, websiteUrl: addon.websiteUrl, isDependency: false };
    installed = [...installed.filter((x) => x.modId !== addon.id), entry];
    listeners.forEach((l) => l({ modId: a.modId, stage: "done", name: "", step: 1, steps: 1, received: 0, total: null }));
    return { installed: [entry], warnings: [] };
  },
  uninstall_addon: (a) => (installed = installed.filter((x) => x.modId !== a.modId)),
  list_installed: () => installed,
  check_updates: () => installed.filter((x) => x.modId === 1001).map((x) => ({ modId: x.modId, latestFileId: 5001, latestFileName: "Details-1.60.2.zip" })),
};

export const mockBackend = {
  invoke: async <T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> => {
    await wait(150);
    const h = handlers[cmd];
    if (!h) throw { kind: "other", message: `mock: unknown command ${cmd}`, url: null };
    return (await h(args)) as T;
  },
  listenProgress: async (cb: (p: Progress) => void) => {
    listeners.push(cb);
    return () => {
      const i = listeners.indexOf(cb);
      if (i >= 0) listeners.splice(i, 1);
    };
  },
  openUrl: async (u: string) => {
    window.open(u, "_blank");
  },
  pickFolder: async () => "C:\\Games\\World of Warcraft",
};
