mod curseforge;
mod error;
mod webview_memory;
mod installer;
mod manifest;
mod settings;
mod tray;
mod wow;

use curseforge::{
    pick_best_file, KeyCheck, AddonSummary, Asset, CategoryNode, CurseForge, SearchPage,
    FALLBACK_FOREVER_TYPE_ID, MAX_SEARCH_WINDOW, PAGE_SIZE, RELATION_REQUIRED,
};
use error::{AppError, AppResult};
use manifest::{manifest_path, InstalledAddon, Manifest};
use serde::{Deserialize, Serialize};
use settings::Settings;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

const CATEGORY_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Addon pages kept in memory so going back and forth is instant.
const DETAIL_CACHE_SIZE: usize = 8;
const DETAIL_TTL: Duration = Duration::from_secs(5 * 60);

struct CategoryCache {
    at: Instant,
    class_id: Option<u32>,
    /// Shared so serving it to the UI or a search never copies the tree.
    tree: Arc<Vec<CategoryNode>>,
}

/// A small, bounded cache of recently viewed addon pages (oldest evicted first).
#[derive(Default)]
struct DetailCache {
    entries: VecDeque<(u32, u8, Instant, Arc<AddonDetail>)>,
}

impl DetailCache {
    fn get(&mut self, mod_id: u32, channel: u8) -> Option<Arc<AddonDetail>> {
        self.entries.retain(|e| e.2.elapsed() < DETAIL_TTL);
        self.entries
            .iter()
            .find(|e| e.0 == mod_id && e.1 == channel)
            .map(|e| e.3.clone())
    }

    fn put(&mut self, mod_id: u32, channel: u8, d: Arc<AddonDetail>) {
        self.entries.retain(|e| e.0 != mod_id);
        if self.entries.len() >= DETAIL_CACHE_SIZE {
            self.entries.pop_front();
        }
        self.entries.push_back((mod_id, channel, Instant::now(), d));
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

pub struct AppState {
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    data_dir: PathBuf,
    cache_dir: PathBuf,
    http: reqwest::Client,
    forever_type: Mutex<Option<u32>>,
    categories: Mutex<Option<CategoryCache>>,
    details: Mutex<DetailCache>,
    /// Makes concurrent callers share one network lookup instead of each
    /// fetching the same version types or categories.
    lookup_lock: tokio::sync::Mutex<()>,
    /// Lets only one install, update or uninstall run at a time.
    install_lock: tokio::sync::Mutex<()>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl AppState {
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    fn client(&self) -> AppResult<CurseForge> {
        let key = settings::get_api_key(&self.settings.lock().unwrap()).ok_or(AppError::NoApiKey)?;
        Ok(CurseForge::new(self.http.clone(), key))
    }

    fn addons_path(&self) -> AppResult<PathBuf> {
        self.settings
            .lock()
            .unwrap()
            .addons_path
            .clone()
            .ok_or(AppError::NoAddonsPath)
    }

    fn manifest_file(&self) -> PathBuf {
        manifest_path(&self.data_dir)
    }

    /// The WoW Forever version type id: the override, then the cached value,
    /// then a live lookup, then the fallback.
    async fn forever_type(&self, cf: &CurseForge) -> AppResult<u32> {
        if let Some(id) = self.settings.lock().unwrap().version_type_override {
            return Ok(id);
        }
        if let Some(id) = *self.forever_type.lock().unwrap() {
            return Ok(id);
        }
        let _single = self.lookup_lock.lock().await;
        if let Some(id) = *self.forever_type.lock().unwrap() {
            return Ok(id);
        }
        match cf.version_types().await {
            Ok(types) => {
                let id = curseforge::find_forever_type(&types).unwrap_or(FALLBACK_FOREVER_TYPE_ID);
                *self.forever_type.lock().unwrap() = Some(id);
                Ok(id)
            }
            // Don't let a refused or failed lookup block searching: use the
            // known id (uncached, so it's retried later). If the key really is
            // refused, the search itself reports CurseForge's exact answer.
            Err(e) => {
                eprintln!("version-type lookup failed, using fallback: {e}");
                Ok(FALLBACK_FOREVER_TYPE_ID)
            }
        }
    }

    fn cached_categories(&self) -> Option<(Option<u32>, Arc<Vec<CategoryNode>>)> {
        self.categories
            .lock()
            .unwrap()
            .as_ref()
            .filter(|c| c.at.elapsed() < CATEGORY_TTL)
            .map(|c| (c.class_id, c.tree.clone()))
    }

    async fn categories(&self, cf: &CurseForge, refresh: bool) -> AppResult<(Option<u32>, Arc<Vec<CategoryNode>>)> {
        if !refresh {
            if let Some(c) = self.cached_categories() {
                return Ok(c);
            }
        }
        let _single = self.lookup_lock.lock().await;
        if !refresh {
            // Another caller may have filled the cache while we waited.
            if let Some(c) = self.cached_categories() {
                return Ok(c);
            }
            if let Some(disk) = load_category_disk_cache(&self.cache_dir) {
                let out = (disk.class_id, disk.tree.clone());
                *self.categories.lock().unwrap() = Some(disk);
                return Ok(out);
            }
        }
        let cats = cf.categories().await?;
        let (class_id, tree) = curseforge::build_category_tree(&cats);
        save_category_disk_cache(&self.cache_dir, class_id, &tree);
        let tree = Arc::new(tree);
        *self.categories.lock().unwrap() = Some(CategoryCache {
            at: Instant::now(),
            class_id,
            tree: tree.clone(),
        });
        Ok((class_id, tree))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiskCategories {
    fetched_at: u64,
    class_id: Option<u32>,
    tree: Vec<CategoryNode>,
}

fn load_category_disk_cache(cache_dir: &Path) -> Option<CategoryCache> {
    let bytes = std::fs::read(cache_dir.join("categories.json")).ok()?;
    let disk: DiskCategories = serde_json::from_slice(&bytes).ok()?;
    let age = now_secs().saturating_sub(disk.fetched_at);
    if age >= CATEGORY_TTL.as_secs() {
        return None;
    }
    Some(CategoryCache {
        at: Instant::now() - Duration::from_secs(age),
        class_id: disk.class_id,
        tree: Arc::new(disk.tree),
    })
}

fn save_category_disk_cache(cache_dir: &Path, class_id: Option<u32>, tree: &[CategoryNode]) {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DiskRef<'a> {
        fetched_at: u64,
        class_id: Option<u32>,
        tree: &'a [CategoryNode],
    }
    let disk = DiskRef { fetched_at: now_secs(), class_id, tree };
    if let Ok(bytes) = serde_json::to_vec(&disk) {
        let _ = settings::write_atomic(&cache_dir.join("categories.json"), &bytes);
    }
}

// ---------- Settings commands ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    addons_path: Option<String>,
    addons_path_valid: bool,
    release_channel: u8,
    version_type_override: Option<u32>,
    has_api_key: bool,
    close_to_tray: bool,
    notify_updates: bool,
}

fn settings_view(s: &Settings) -> SettingsView {
    SettingsView {
        addons_path: s.addons_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        addons_path_valid: s.addons_path.as_deref().map(wow::is_valid_addons_dir).unwrap_or(false),
        release_channel: s.release_channel,
        version_type_override: s.version_type_override,
        has_api_key: settings::get_api_key(s).is_some(),
        close_to_tray: s.close_to_tray,
        notify_updates: s.notify_updates,
    }
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> SettingsView {
    settings_view(&state.settings.lock().unwrap())
}

#[tauri::command]
fn save_settings(
    state: State<'_, AppState>,
    addons_path: Option<String>,
    release_channel: u8,
    version_type_override: Option<u32>,
    close_to_tray: Option<bool>,
    notify_updates: Option<bool>,
) -> AppResult<SettingsView> {
    let mut s = state.settings.lock().unwrap();
    if let Some(v) = close_to_tray {
        s.close_to_tray = v;
    }
    if let Some(v) = notify_updates {
        s.notify_updates = v;
    }
    s.addons_path = addons_path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .map(|p| wow::normalize_user_path(Path::new(&p)));
    if s.version_type_override != version_type_override || s.release_channel != release_channel.clamp(1, 3) {
        state.details.lock().unwrap().clear();
    }
    s.release_channel = release_channel.clamp(1, 3);
    if s.version_type_override != version_type_override {
        s.version_type_override = version_type_override;
        *state.forever_type.lock().unwrap() = None;
    }
    s.save(&state.settings_path)?;
    Ok(settings_view(&s))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KeySaveResult {
    settings: SettingsView,
    /// The key check, when one ran. It may contain failures if the key was
    /// saved anyway (`force`) or CurseForge couldn't be reached.
    check: Option<KeyCheck>,
}

/// Saves (or, when empty, removes) the API key. The key is tested first; if
/// CurseForge refuses it, nothing is saved unless `force` is set.
#[tauri::command]
async fn set_api_key(state: State<'_, AppState>, key: String, force: Option<bool>) -> AppResult<KeySaveResult> {
    let key = curseforge::sanitize_key(&key).map_err(AppError::Other)?;
    let mut check = None;
    if !key.is_empty() {
        let c = CurseForge::new(state.http.clone(), key.clone()).diagnose_key().await;
        // A network failure isn't the key's fault, so it doesn't block saving.
        if c.has_http_failure() && !force.unwrap_or(false) {
            return Err(AppError::KeyCheckFailed(Box::new(c)));
        }
        check = Some(c);
    }
    let mut s = state.settings.lock().unwrap();
    settings::set_api_key(&mut s, (!key.is_empty()).then_some(key.as_str()))?;
    s.save(&state.settings_path)?;
    *state.forever_type.lock().unwrap() = None;
    state.details.lock().unwrap().clear();
    Ok(KeySaveResult { settings: settings_view(&s), check })
}

/// Tests the saved key and reports what CurseForge answers.
#[tauri::command]
async fn test_api_key(state: State<'_, AppState>) -> AppResult<KeyCheck> {
    Ok(state.client()?.diagnose_key().await)
}

#[tauri::command]
fn detect_wow_path() -> Option<String> {
    wow::detect_addons_dir().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn open_addons_folder(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let p = state.addons_path()?;
    std::fs::create_dir_all(&p)?;
    app.opener()
        .open_path(p.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::Other(e.to_string()))
}

// ---------- Browsing commands ----------

#[tauri::command]
async fn get_categories(state: State<'_, AppState>, refresh: Option<bool>) -> AppResult<Arc<Vec<CategoryNode>>> {
    let cf = state.client()?;
    Ok(state.categories(&cf, refresh.unwrap_or(false)).await?.1)
}

#[tauri::command]
async fn search_addons(
    state: State<'_, AppState>,
    query: String,
    category_id: Option<u32>,
    sort_field: Option<u8>,
    index: Option<u32>,
) -> AppResult<SearchPage> {
    let cf = state.client()?;
    // Both are cached after the first call; on a cold start they run in parallel.
    let (type_id, cats) = tokio::join!(state.forever_type(&cf), state.categories(&cf, false));
    let type_id = type_id?;
    // A failed category lookup shouldn't block searching.
    let class_id = cats.ok().and_then(|c| c.0);
    let index = index.unwrap_or(0).min(MAX_SEARCH_WINDOW - PAGE_SIZE);
    let channel = state.settings().release_channel;
    let (mods, pagination) = cf
        .search(type_id, class_id, category_id, &query, sort_field.unwrap_or(2), index)
        .await?;
    let next = index + pagination.result_count.max(mods.len() as u32);
    let has_more = !mods.is_empty()
        && (next as u64) < pagination.total_count
        && next + PAGE_SIZE <= MAX_SEARCH_WINDOW;
    Ok(SearchPage {
        addons: mods
            .iter()
            .map(|m| AddonSummary::from_mod(m, type_id, channel))
            .collect(),
        pagination,
        has_more,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileInfo {
    id: u32,
    display_name: String,
    file_name: String,
    file_date: String,
    release_type: u8,
    file_length: u64,
    game_versions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyInfo {
    mod_id: u32,
    name: String,
    required: bool,
    installed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddonDetail {
    addon: AddonSummary,
    screenshots: Vec<Asset>,
    description_html: String,
    latest_file: Option<FileInfo>,
    dependencies: Vec<DependencyInfo>,
    allow_distribution: bool,
}

#[tauri::command]
async fn get_addon_detail(state: State<'_, AppState>, mod_id: u32) -> AppResult<Arc<AddonDetail>> {
    let channel = state.settings().release_channel;
    if let Some(hit) = state.details.lock().unwrap().get(mod_id, channel) {
        return Ok(hit);
    }
    let cf = state.client()?;
    let type_id = state.forever_type(&cf).await?;
    let (m, description, files) = tokio::join!(
        cf.get_mod(mod_id),
        cf.description(mod_id),
        cf.files(mod_id, type_id)
    );
    let m = m?;
    let files = files.unwrap_or_default();
    let best = pick_best_file(&files, type_id, channel);

    let deps: Vec<(u32, bool)> = best
        .map(|f| {
            f.dependencies
                .iter()
                .filter(|d| d.relation_type == RELATION_REQUIRED || d.relation_type == 2)
                .map(|d| (d.mod_id, d.relation_type == RELATION_REQUIRED))
                .collect()
        })
        .unwrap_or_default();
    let names: HashMap<u32, String> = cf
        .get_mods(&deps.iter().map(|d| d.0).collect::<Vec<_>>())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    let installed: HashSet<u32> = match state.addons_path() {
        Ok(p) => Manifest::load(&state.manifest_file())
            .entries(&p)
            .iter()
            .map(|a| a.mod_id)
            .collect(),
        Err(_) => HashSet::new(),
    };

    let detail = Arc::new(AddonDetail {
        addon: AddonSummary::from_mod(&m, type_id, channel),
        screenshots: m.screenshots.clone(),
        description_html: description.unwrap_or_default(),
        latest_file: best.map(|f| FileInfo {
            id: f.id,
            display_name: f.display_name.clone(),
            file_name: f.file_name.clone(),
            file_date: f.file_date.clone(),
            release_type: f.release_type,
            file_length: f.file_length,
            game_versions: f.game_versions.clone(),
        }),
        dependencies: deps
            .into_iter()
            .map(|(id, required)| DependencyInfo {
                mod_id: id,
                name: names.get(&id).cloned().unwrap_or_else(|| format!("Addon #{id}")),
                required,
                installed: installed.contains(&id),
            })
            .collect(),
        allow_distribution: m.allow_mod_distribution != Some(false),
    });
    state.details.lock().unwrap().put(mod_id, channel, detail.clone());
    Ok(detail)
}

// ---------- Install / update / uninstall ----------

struct LiveSource<'a> {
    cf: &'a CurseForge,
    type_id: u32,
    channel: u8,
}

impl installer::FileSource for LiveSource<'_> {
    async fn best_file(&self, mod_id: u32) -> AppResult<Option<curseforge::File>> {
        let files = self.cf.files(mod_id, self.type_id).await?;
        Ok(pick_best_file(&files, self.type_id, self.channel).cloned())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    mod_id: u32,
    stage: &'a str,
    name: &'a str,
    step: usize,
    steps: usize,
    received: u64,
    total: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallResult {
    installed: Vec<InstalledAddon>,
    warnings: Vec<String>,
}

#[tauri::command]
async fn install_addon(app: AppHandle, state: State<'_, AppState>, mod_id: u32) -> AppResult<InstallResult> {
    let _guard = state.install_lock.lock().await;
    let cf = state.client()?;
    let addons = state.addons_path()?;
    let type_id = state.forever_type(&cf).await?;
    let channel = state.settings().release_channel;
    let emit = |p: Progress| {
        let _ = app.emit("install-progress", p);
    };
    emit(Progress { mod_id, stage: "resolving", name: "", step: 0, steps: 0, received: 0, total: None });

    let mut manifest = Manifest::load(&state.manifest_file());
    let installed_ids: HashSet<u32> = manifest.entries(&addons).iter().map(|a| a.mod_id).collect();
    let src = LiveSource { cf: &cf, type_id, channel };
    let plan = installer::plan_install(&src, mod_id, &installed_ids).await?;

    // One batch request for names, thumbnails and website links.
    let ids: Vec<u32> = plan
        .steps
        .iter()
        .map(|s| s.mod_id)
        .chain(plan.missing_dependencies.iter().copied())
        .collect();
    let mods: HashMap<u32, curseforge::Mod> = cf
        .get_mods(&ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| (m.id, m))
        .collect();
    let name_of = |id: u32| mods.get(&id).map(|m| m.name.clone()).unwrap_or_else(|| format!("Addon #{id}"));

    let mut warnings: Vec<String> = plan
        .missing_dependencies
        .iter()
        .map(|id| format!("Required dependency {} has no WoW Forever version.", name_of(*id)))
        .collect();
    let mut done = Vec::new();
    let steps = plan.steps.len();

    for (i, step) in plan.steps.iter().enumerate() {
        let is_root = step.mod_id == mod_id;
        let name = name_of(step.mod_id);
        let website = mods.get(&step.mod_id).and_then(|m| m.links.website_url.clone());
        let result = async {
            let url = match step.file.download_url.clone().filter(|u| !u.is_empty()) {
                Some(u) => u,
                None => cf.download_url(step.mod_id, step.file.id).await?.ok_or_else(|| {
                    AppError::DistributionDisabled {
                        name: name.clone(),
                        url: website
                            .clone()
                            .unwrap_or_else(|| "https://www.curseforge.com/wow/addons".into()),
                    }
                })?,
            };

            let zip_path = state.cache_dir.join("downloads").join(format!("{}.zip", step.file.id));
            let mut last = Instant::now() - Duration::from_secs(1);
            installer::download(cf.http(), &url, &zip_path, |received, total| {
                if last.elapsed() >= Duration::from_millis(120) || Some(received) == total {
                    last = Instant::now();
                    emit(Progress { mod_id, stage: "downloading", name: &name, step: i + 1, steps, received, total });
                }
            })
            .await?;

            emit(Progress { mod_id, stage: "extracting", name: &name, step: i + 1, steps, received: 0, total: None });
            let staging = addons.join(format!(".ezaddon-staging-{}", step.mod_id));
            let fallback = step.file.file_name.trim_end_matches(".zip").to_string();
            let (a, s, z) = (addons.clone(), staging.clone(), zip_path.clone());
            let folders = tauri::async_runtime::spawn_blocking(move || {
                let r = installer::extract_zip(&z, &s, &fallback)
                    .and_then(|folders| installer::swap_in(&a, &s, &folders).map(|_| folders));
                let _ = std::fs::remove_dir_all(&s);
                let _ = std::fs::remove_file(&z);
                r
            })
            .await
            .map_err(|e| AppError::Other(e.to_string()))??;
            Ok::<_, AppError>(folders)
        }
        .await;

        let folders = match result {
            Ok(f) => f,
            Err(e) if is_root => return Err(e),
            Err(e) => {
                warnings.push(format!("Could not install dependency {name}: {e}"));
                continue;
            }
        };

        // Remove folders the previous version had that the new one doesn't,
        // unless another addon owns them.
        let previous = manifest.get(&addons, step.mod_id).cloned();
        if let Some(prev) = &previous {
            let stale: Vec<String> = prev
                .folders
                .iter()
                .filter(|f| !folders.iter().any(|n| n.eq_ignore_ascii_case(f)))
                .filter(|f| !manifest.folder_owned_by_other(&addons, f, step.mod_id))
                .cloned()
                .collect();
            let _ = installer::remove_folders(&addons, &stale);
        }

        let m = mods.get(&step.mod_id);
        let entry = InstalledAddon {
            mod_id: step.mod_id,
            name,
            file_id: step.file.id,
            file_name: step.file.file_name.clone(),
            display_name: step.file.display_name.clone(),
            folders,
            installed_at: now_secs(),
            thumbnail: m.and_then(|m| m.logo.as_ref()).and_then(|l| l.thumbnail_url.clone()),
            website_url: website,
            is_dependency: !is_root && previous.map(|p| p.is_dependency).unwrap_or(true),
        };
        manifest.put(&addons, entry.clone());
        manifest.save(&state.manifest_file())?;
        done.push(entry);
    }

    // Cached pages show dependency install state, so drop them.
    state.details.lock().unwrap().clear();
    emit(Progress { mod_id, stage: "done", name: "", step: steps, steps, received: 0, total: None });
    Ok(InstallResult { installed: done, warnings })
}

#[tauri::command]
async fn uninstall_addon(state: State<'_, AppState>, mod_id: u32) -> AppResult<Vec<InstalledAddon>> {
    let _guard = state.install_lock.lock().await;
    let addons = state.addons_path()?;
    let mut manifest = Manifest::load(&state.manifest_file());
    if let Some(entry) = manifest.get(&addons, mod_id).cloned() {
        let removable: Vec<String> = entry
            .folders
            .iter()
            .filter(|f| !manifest.folder_owned_by_other(&addons, f, mod_id))
            .cloned()
            .collect();
        installer::remove_folders(&addons, &removable)?;
        manifest.remove(&addons, mod_id);
        manifest.save(&state.manifest_file())?;
        state.details.lock().unwrap().clear();
    }
    Ok(manifest.entries(&addons))
}

#[tauri::command]
fn list_installed(state: State<'_, AppState>) -> AppResult<Vec<InstalledAddon>> {
    let Ok(addons) = state.addons_path() else {
        return Ok(vec![]);
    };
    let mut manifest = Manifest::load(&state.manifest_file());
    if manifest.prune_missing(&addons) {
        manifest.save(&state.manifest_file())?;
    }
    Ok(manifest.entries(&addons))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateInfo {
    mod_id: u32,
    latest_file_id: u32,
    latest_file_name: String,
}

#[tauri::command]
async fn check_updates(app: AppHandle, state: State<'_, AppState>) -> AppResult<Vec<UpdateInfo>> {
    let list = compute_updates(&state).await?;
    tray::show_update_count(&app, list.len());
    Ok(list)
}

/// Compares installed addons with CurseForge's latest WoW Forever files.
pub(crate) async fn compute_updates(state: &AppState) -> AppResult<Vec<UpdateInfo>> {
    let cf = state.client()?;
    let addons = state.addons_path()?;
    let type_id = state.forever_type(&cf).await?;
    let channel = state.settings().release_channel;
    let installed = Manifest::load(&state.manifest_file()).entries(&addons);
    let current: HashMap<u32, u32> = installed.iter().map(|a| (a.mod_id, a.file_id)).collect();
    let ids: Vec<u32> = current.keys().copied().collect();
    let mut out = Vec::new();
    for chunk in ids.chunks(100) {
        for m in cf.get_mods(chunk).await? {
            if let Some(idx) = curseforge::latest_index_for_type(&m, type_id, channel) {
                if current.get(&m.id).is_some_and(|f| *f != idx.file_id && idx.file_id > *f) {
                    out.push(UpdateInfo {
                        mod_id: m.id,
                        latest_file_id: idx.file_id,
                        latest_file_name: idx.filename.clone(),
                    });
                }
            }
        }
    }
    Ok(out)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second launch just reopens the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let paths = app.path();
            let config_dir = paths.app_config_dir()?;
            let data_dir = paths.app_data_dir()?;
            let cache_dir = paths.app_cache_dir()?;
            for d in [&config_dir, &data_dir, &cache_dir] {
                std::fs::create_dir_all(d)?;
            }
            let settings_path = config_dir.join("settings.json");
            let mut settings = Settings::load(&settings_path);
            if settings.addons_path.is_none() {
                settings.addons_path = wow::detect_addons_dir();
            }
            // Remove staging folders left behind if the app was closed mid-install.
            // (Backups are left alone: they may be the only copy of an addon.)
            if let Some(addons) = &settings.addons_path {
                if let Ok(entries) = std::fs::read_dir(addons) {
                    for e in entries.flatten() {
                        if e.file_name().to_string_lossy().starts_with(".ezaddon-staging-") {
                            let _ = std::fs::remove_dir_all(e.path());
                        }
                    }
                }
            }
            // Clear downloads left over from an interrupted install.
            let _ = std::fs::remove_dir_all(cache_dir.join("downloads"));
            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
                data_dir,
                cache_dir,
                http: CurseForge::http_client(),
                forever_type: Mutex::new(None),
                categories: Mutex::new(None),
                details: Mutex::new(DetailCache::default()),
                lookup_lock: tokio::sync::Mutex::new(()),
                install_lock: tokio::sync::Mutex::new(()),
            });

            // Warm the version-type and category caches while the window loads,
            // so the first search needs a single round trip.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                if let Ok(cf) = state.client() {
                    let _ = tokio::join!(state.forever_type(&cf), state.categories(&cf, false));
                }
            });

            // Without a tray (e.g. no shell running), closing the window just exits.
            if let Err(e) = tray::setup(app) {
                eprintln!("system tray unavailable: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Let WebView2 release memory while the window is minimized.
            if let tauri::WindowEvent::Resized(_) = event {
                let minimized = window.is_minimized().unwrap_or(false);
                webview_memory::set_low(window, minimized);
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            set_api_key,
            test_api_key,
            detect_wow_path,
            open_addons_folder,
            get_categories,
            search_addons,
            get_addon_detail,
            install_addon,
            uninstall_addon,
            list_installed,
            check_updates,
        ])
        .build(tauri::generate_context!())
        .expect("error while building EzAddon");

    app.run(|app, event| {
        // Closing the last window asks the app to exit (with no exit code).
        // In tray mode the window, and the WebView2 processes behind it, are
        // gone but EzAddon keeps running in the tray. Quit from the tray menu
        // calls exit(0), which carries a code and is let through.
        if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
            if tray::is_available() && app.state::<AppState>().settings().close_to_tray {
                api.prevent_exit();
            }
        }
    });
}
