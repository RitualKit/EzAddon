# EzAddon

A small, fast Windows app for finding and installing **World of Warcraft: Forever** addons from CurseForge.

- Browse every WoW Forever addon on CurseForge by category, or search by name.
- Sort by popularity, downloads, last updated, newest or name.
- Install with one click. Required dependencies are installed automatically.
- See what you've installed, check for updates, update one addon or all of them, and uninstall.
- It only shows WoW Forever. Retail, Classic and other games are hidden.

It's built with [Tauri](https://tauri.app) and Svelte, so it uses Windows' built-in WebView2 instead of bundling a browser. The installer is a few MB and the app uses far less memory than Electron-based managers. It does nothing in the background while it's idle.

## Getting started

1. Download `EzAddon_x.y.z_x64-setup.exe` from the [Releases](../../releases) page or from the latest **Build** workflow run's artifacts, then run it. It installs for your user only, so no admin rights are needed.
2. **CurseForge API key:**
   - Sign in at [console.curseforge.com](https://console.curseforge.com/) and create a free API key.
   - Paste it into EzAddon.
   - The key is stored in Windows Credential Manager and is only sent to `api.curseforge.com`.
3. **WoW folder:** click **Detect**, or use **Browse…** to pick your `World of Warcraft` folder.
   - Addons go into `World of Warcraft\_forever_\Interface\AddOns`.
   - You can also pick the `_forever_` folder or the `AddOns` folder directly.

## How installs work

1. EzAddon picks the newest WoW Forever file for the addon, following your release channel (Release, Beta or Alpha).
2. It downloads the zip and unpacks it into a temporary folder inside `AddOns`. Archive entries that would write outside that folder are rejected.
3. It moves the addon folders into place. Existing copies are kept as backups until the move succeeds, and restored if it fails.
4. It records what it installed in `%APPDATA%\com.ritualkit.ezaddon\installed.json`. Uninstalling removes only the folders EzAddon put there, and never a folder another installed addon still uses.

**Third-party downloads:** some authors don't allow downloads from third-party apps. For those addons, EzAddon shows an **Open on CurseForge** button instead of installing.

**Addons installed outside EzAddon:** v1 doesn't track addons you installed manually or with another manager, and leaves them alone.

**Game version:** EzAddon finds CurseForge's WoW Forever game-version id automatically. If CurseForge ever changes it, you can override it in **Settings → Advanced**.

## Development

You need Node 22+, the Rust toolchain (stable, 1.90+), and on Windows the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```bash
npm install
```

Run the full app with hot reload:

```bash
npm run tauri dev
```

Run the UI only, in a browser, against a mock backend:

```bash
npm run dev
```

Run the checks:

```bash
npm run check
```

```bash
cd src-tauri && cargo test && cargo clippy --all-targets
```

Build the installers (on Windows) into `src-tauri/target/release/bundle/`:

```bash
npm run tauri build
```

For local testing you can set the `EZADDON_CF_API_KEY` environment variable instead of saving a key in the app.

CI (`.github/workflows/build.yml`) runs the checks on Linux and builds the NSIS `.exe` and `.msi` installers on Windows for every push. Pushing a `v*` tag attaches the installers to a GitHub release.

### Layout

| Path | What it is |
|---|---|
| `src-tauri/src/curseforge.rs` | CurseForge API client, file selection, category tree |
| `src-tauri/src/installer.rs` | dependency planning, download, safe unzip, atomic folder swap |
| `src-tauri/src/manifest.rs` | the installed-addons manifest |
| `src-tauri/src/wow.rs` | WoW Forever folder detection |
| `src-tauri/src/settings.rs` | settings and API key storage |
| `src-tauri/src/lib.rs` | Tauri commands |
| `src/` | Svelte 5 UI. `src/lib/mock.ts` is the browser-only mock backend |

EzAddon is not affiliated with Blizzard Entertainment or CurseForge/Overwolf.
