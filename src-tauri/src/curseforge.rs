//! Minimal CurseForge Core API client, limited to World of Warcraft (gameId 1)
//! and the WoW Forever game-version type.

use crate::error::{AppError, AppResult};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const API_BASE: &str = "https://api.curseforge.com";
pub const WOW_GAME_ID: u32 = 1;
/// The WoW Forever `gameVersionTypeId` seen on curseforge.com. Used only when
/// the live lookup in [`resolve_forever_type_id`] finds nothing.
pub const FALLBACK_FOREVER_TYPE_ID: u32 = 88568;
pub const PAGE_SIZE: u32 = 30;
/// CurseForge rejects searches where index + pageSize exceeds this.
pub const MAX_SEARCH_WINDOW: u32 = 10_000;

/// `relationType` values from the CurseForge API.
pub const RELATION_REQUIRED: u8 = 3;

// ---------- API models (only the fields EzAddon uses) ----------
//
// CurseForge sometimes sends `null` for fields that are normally strings or
// lists. `nn` turns those into empty values so one odd addon can't make a
// whole page of results fail to load.

fn nn<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Clone, Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pagination {
    pub index: u32,
    pub page_size: u32,
    pub result_count: u32,
    pub total_count: u64,
}

/// A page of results. Items are parsed one by one, and any that don't match
/// the expected shape are skipped instead of failing the whole page.
#[derive(Debug, Clone)]
struct Paged<T> {
    data: Vec<T>,
    pagination: Pagination,
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Paged<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default, deserialize_with = "nn")]
            data: Vec<serde_json::Value>,
            #[serde(default, deserialize_with = "nn")]
            pagination: Pagination,
        }
        let raw = Raw::deserialize(d)?;
        let mut skipped = 0;
        let data = raw
            .data
            .into_iter()
            .filter_map(|v| match serde_json::from_value(v) {
                Ok(item) => Some(item),
                Err(e) => {
                    skipped += 1;
                    eprintln!("unexpected item shape: {e}");
                    None
                }
            })
            .collect();
        if skipped > 0 {
            eprintln!("skipped {skipped} item(s) in a CurseForge response");
        }
        Ok(Paged { data, pagination: raw.pagination })
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Category {
    pub id: u32,
    #[serde(deserialize_with = "nn")]
    pub name: String,
    #[serde(deserialize_with = "nn")]
    pub slug: String,
    pub icon_url: Option<String>,
    pub class_id: Option<u32>,
    pub parent_category_id: Option<u32>,
    pub is_class: Option<bool>,
    pub display_index: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Links {
    pub website_url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Asset {
    pub title: Option<String>,
    pub thumbnail_url: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Author {
    #[serde(deserialize_with = "nn")]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileIndex {
    #[serde(deserialize_with = "nn")]
    pub game_version: String,
    pub file_id: u32,
    #[serde(deserialize_with = "nn")]
    pub filename: String,
    pub release_type: u8,
    pub game_version_type_id: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Mod {
    pub id: u32,
    #[serde(deserialize_with = "nn")]
    pub name: String,
    #[serde(deserialize_with = "nn")]
    pub slug: String,
    #[serde(deserialize_with = "nn")]
    pub summary: String,
    pub download_count: f64,
    #[serde(deserialize_with = "nn")]
    pub links: Links,
    pub logo: Option<Asset>,
    #[serde(deserialize_with = "nn")]
    pub screenshots: Vec<Asset>,
    #[serde(deserialize_with = "nn")]
    pub authors: Vec<Author>,
    #[serde(deserialize_with = "nn")]
    pub categories: Vec<Category>,
    #[serde(deserialize_with = "nn")]
    pub latest_files_indexes: Vec<FileIndex>,
    pub date_modified: Option<String>,
    pub allow_mod_distribution: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SortableGameVersion {
    pub game_version_type_id: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Dependency {
    pub mod_id: u32,
    pub relation_type: u8,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct File {
    pub id: u32,
    pub mod_id: u32,
    #[serde(deserialize_with = "nn")]
    pub display_name: String,
    #[serde(deserialize_with = "nn")]
    pub file_name: String,
    pub release_type: u8,
    #[serde(deserialize_with = "nn")]
    pub file_date: String,
    pub file_length: u64,
    pub download_url: Option<String>,
    #[serde(deserialize_with = "nn")]
    pub game_versions: Vec<String>,
    #[serde(deserialize_with = "nn")]
    pub sortable_game_versions: Vec<SortableGameVersion>,
    #[serde(deserialize_with = "nn")]
    pub dependencies: Vec<Dependency>,
    pub is_available: Option<bool>,
    /// The addon folders in this file and their fingerprints (WoW only).
    #[serde(deserialize_with = "nn")]
    pub modules: Vec<FileModule>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileModule {
    #[serde(deserialize_with = "nn")]
    pub name: String,
    pub fingerprint: u64,
}

/// A file CurseForge matched from folder fingerprints, and the mod it belongs to.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FingerprintMatch {
    pub id: u32,
    pub file: File,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FingerprintMatches {
    #[serde(deserialize_with = "nn")]
    pub exact_matches: Vec<FingerprintMatch>,
    #[serde(deserialize_with = "nn")]
    pub partial_matches: Vec<FingerprintMatch>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameVersionType {
    pub id: u32,
    #[serde(deserialize_with = "nn")]
    pub name: String,
    #[serde(deserialize_with = "nn")]
    pub slug: String,
}

// ---------- Pure helpers (unit-tested) ----------

/// Finds the WoW Forever version type among CurseForge's WoW version types.
pub fn find_forever_type(types: &[GameVersionType]) -> Option<u32> {
    types
        .iter()
        .find(|t| {
            t.name.to_ascii_lowercase().contains("forever")
                || t.slug.to_ascii_lowercase().contains("forever")
        })
        .map(|t| t.id)
}

/// True if `file` is built for the given game-version type. Files without
/// version-type data are trusted, because the request was already filtered.
pub fn file_matches_type(file: &File, type_id: u32) -> bool {
    file.sortable_game_versions.is_empty()
        || file
            .sortable_game_versions
            .iter()
            .any(|v| v.game_version_type_id == Some(type_id))
}

/// Picks the newest usable file for the type. `max_release` is 1 (release
/// only), 2 (beta and release) or 3 (alpha too). If nothing fits the channel,
/// the newest file of any channel is returned.
pub fn pick_best_file(files: &[File], type_id: u32, max_release: u8) -> Option<&File> {
    let usable: Vec<&File> = files
        .iter()
        .filter(|f| f.is_available != Some(false) && file_matches_type(f, type_id))
        .collect();
    // ISO-8601 dates sort correctly as strings; the file id breaks ties.
    let newest = |a: &&File, b: &&File| (a.file_date.as_str(), a.id).cmp(&(b.file_date.as_str(), b.id));
    usable
        .iter()
        .copied()
        .filter(|f| f.release_type >= 1 && f.release_type <= max_release)
        .max_by(newest)
        .or_else(|| usable.iter().copied().max_by(newest))
}

/// Newest file id for the type from a mod's `latestFilesIndexes`.
pub fn latest_index_for_type(m: &Mod, type_id: u32, max_release: u8) -> Option<&FileIndex> {
    let of_type = || {
        m.latest_files_indexes
            .iter()
            .filter(move |i| i.game_version_type_id == Some(type_id))
    };
    of_type()
        .filter(|i| i.release_type >= 1 && i.release_type <= max_release)
        .max_by_key(|i| i.file_id)
        .or_else(|| of_type().max_by_key(|i| i.file_id))
}

// ---------- DTOs sent to the frontend ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonSummary {
    pub id: u32,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub downloads: u64,
    pub thumbnail: Option<String>,
    pub categories: Vec<String>,
    pub website_url: Option<String>,
    pub latest_file_id: Option<u32>,
    pub game_version: Option<String>,
    pub date_modified: Option<String>,
    /// False when the author only allows downloads from the CurseForge website.
    pub allow_distribution: bool,
}

impl AddonSummary {
    pub fn from_mod(m: &Mod, type_id: u32, max_release: u8) -> Self {
        let latest = latest_index_for_type(m, type_id, max_release);
        AddonSummary {
            id: m.id,
            name: m.name.clone(),
            summary: m.summary.clone(),
            author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
            downloads: m.download_count.max(0.0) as u64,
            thumbnail: m
                .logo
                .as_ref()
                .and_then(|l| l.thumbnail_url.clone().or_else(|| l.url.clone())),
            categories: m.categories.iter().map(|c| c.name.clone()).collect(),
            website_url: m.links.website_url.clone(),
            latest_file_id: latest.map(|i| i.file_id),
            game_version: latest.map(|i| i.game_version.clone()).filter(|v| !v.is_empty()),
            date_modified: m.date_modified.clone(),
            allow_distribution: m.allow_mod_distribution != Some(false),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub addons: Vec<AddonSummary>,
    pub pagination: Pagination,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryNode {
    pub id: u32,
    pub name: String,
    pub icon_url: Option<String>,
    pub children: Vec<CategoryNode>,
}

/// Builds the category tree. Class categories (`isClass`) are removed. A
/// category whose parent is the addons class, or isn't in the list, is a root.
pub fn build_category_tree(cats: &[Category]) -> (Option<u32>, Vec<CategoryNode>) {
    let addon_class = cats
        .iter()
        .filter(|c| c.is_class == Some(true))
        .find(|c| c.slug.contains("addon") || c.name.to_ascii_lowercase().contains("addon"))
        .or_else(|| cats.iter().find(|c| c.is_class == Some(true)))
        .map(|c| c.id);

    let normal: Vec<&Category> = cats
        .iter()
        .filter(|c| c.is_class != Some(true))
        .filter(|c| match (addon_class, c.class_id) {
            (Some(cls), Some(cid)) => cid == cls,
            _ => true,
        })
        .collect();
    let ids: std::collections::HashSet<u32> = normal.iter().map(|c| c.id).collect();

    fn children_of(parent: u32, all: &[&Category], depth: u8) -> Vec<CategoryNode> {
        if depth > 4 {
            return vec![];
        }
        let mut v: Vec<&&Category> = all
            .iter()
            .filter(|c| c.parent_category_id == Some(parent) && c.id != parent)
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v.into_iter()
            .map(|c| CategoryNode {
                id: c.id,
                name: c.name.clone(),
                icon_url: c.icon_url.clone(),
                children: children_of(c.id, all, depth + 1),
            })
            .collect()
    }

    let mut roots: Vec<&&Category> = normal
        .iter()
        .filter(|c| match c.parent_category_id {
            None | Some(0) => true,
            Some(p) => Some(p) == addon_class || !ids.contains(&p),
        })
        .collect();
    roots.sort_by(|a, b| a.name.cmp(&b.name));
    let tree = roots
        .into_iter()
        .map(|c| CategoryNode {
            id: c.id,
            name: c.name.clone(),
            icon_url: c.icon_url.clone(),
            children: children_of(c.id, &normal, 1),
        })
        .collect();
    (addon_class, tree)
}

// ---------- HTTP client ----------

// ---------- API key checks ----------

/// Cleans up a pasted key: removes whitespace (including inside the key),
/// zero-width characters, surrounding quotes and an accidental `x-api-key:`
/// prefix. Returns an empty string for an empty paste (meaning "remove").
pub fn sanitize_key(raw: &str) -> Result<String, String> {
    let compact: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}'))
        .collect();
    let mut key = compact.as_str();
    if key.len() >= 10 && key[..10].eq_ignore_ascii_case("x-api-key:") {
        key = &key[10..];
    }
    let key = key.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
    if let Some(bad) = key.chars().find(|c| !c.is_ascii_graphic()) {
        return Err(format!(
            "The key contains an unexpected character ({bad:?}). Copy it again from console.curseforge.com."
        ));
    }
    Ok(key.to_string())
}

/// The result of one test request made with a key.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub ok: bool,
    /// HTTP status, or `None` when CurseForge couldn't be reached at all.
    pub status: Option<u16>,
    pub endpoint: String,
    pub detail: String,
}

/// Two test requests: one any valid key may make, and the World of Warcraft
/// search the app actually needs. Together they tell a bad key apart from a
/// key that works but is refused for WoW, or a network problem.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyCheck {
    pub general: Probe,
    pub wow: Probe,
    pub hint: String,
}

impl KeyCheck {
    pub fn new(general: Probe, wow: Probe) -> Self {
        let hint = key_hint(&general, &wow);
        KeyCheck { general, wow, hint }
    }

    /// True if CurseForge itself answered with an error (not just unreachable).
    pub fn has_http_failure(&self) -> bool {
        [&self.general, &self.wow].iter().any(|p| !p.ok && p.status.is_some())
    }
}

fn key_hint(general: &Probe, wow: &Probe) -> String {
    let st = |p: &Probe| p.status.unwrap_or(0);
    let cdn = [general, wow]
        .iter()
        .any(|p| p.detail.to_ascii_lowercase().contains("cloudfront") || p.detail.to_ascii_lowercase().contains("cloudflare"));
    if general.ok && wow.ok {
        "The key works.".into()
    } else if st(general) == 429 || st(wow) == 429 {
        "CurseForge is rate-limiting requests. Wait a minute and test again.".into()
    } else if general.status.is_none() && wow.status.is_none() {
        "Couldn't reach CurseForge. Check your internet connection, firewall, antivirus or proxy.".into()
    } else if st(general) == 401 || st(wow) == 401 {
        "CurseForge doesn't recognize this key. Copy it again from console.curseforge.com, or generate a new one.".into()
    } else if general.ok {
        format!(
            "The key works, but CurseForge refuses World of Warcraft requests for it (HTTP {}). \
             Check your project in console.curseforge.com, or generate a new key.",
            st(wow)
        )
    } else if st(general) == 403 {
        let mut h = "CurseForge refused this key (HTTP 403). It may be revoked or not active yet: \
                     generate a new key in console.curseforge.com (some people also had to rename \
                     their console organization first)."
            .to_string();
        if cdn {
            h.push_str(" The refusal came from CurseForge's CDN, which can also mean your network or IP is being blocked or rate-limited; try again later or on another network.");
        }
        h
    } else {
        format!("CurseForge returned an error (HTTP {}). It may be having problems; try again later.", st(general).max(st(wow)))
    }
}

/// Short, single-line description of a failed response for error messages:
/// the start of the body plus headers that show who refused the request.
async fn describe_failure(resp: reqwest::Response) -> String {
    let mut parts = Vec::new();
    for h in ["server", "x-cache", "x-amzn-errortype", "cf-ray"] {
        if let Some(v) = resp.headers().get(h).and_then(|v| v.to_str().ok()) {
            parts.push(format!("{h}: {v}"));
        }
    }
    let body = resp.text().await.unwrap_or_default();
    let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if !body.is_empty() {
        let short: String = body.chars().take(200).collect();
        let ell = if body.chars().count() > 200 { "…" } else { "" };
        parts.insert(0, format!("response: {short}{ell}"));
    }
    parts.join("; ")
}

#[derive(Clone)]
pub struct CurseForge {
    http: reqwest::Client,
    key: String,
    base: String,
}

impl CurseForge {
    pub fn new(http: reqwest::Client, key: String) -> Self {
        CurseForge { http, key, base: API_BASE.to_string() }
    }

    #[cfg(test)]
    pub fn with_base(http: reqwest::Client, key: String, base: String) -> Self {
        CurseForge { http, key, base }
    }

    pub fn http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .user_agent(concat!("EzAddon/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            // Keep a couple of warm connections for fast browsing, and close
            // idle ones so nothing lingers in the background.
            .pool_max_idle_per_host(4)
            .pool_idle_timeout(Duration::from_secs(60))
            .tcp_nodelay(true)
            .build()
            .expect("HTTP client")
    }

    /// Passes a successful response through; turns a failed one into an
    /// error that keeps the status and what CurseForge said.
    async fn ensure_ok(resp: reqwest::Response, path: &str) -> AppResult<reqwest::Response> {
        let status = resp.status().as_u16();
        match status {
            200..=299 => Ok(resp),
            429 => Err(AppError::RateLimited),
            401 | 403 => Err(AppError::Rejected {
                status,
                endpoint: path.to_string(),
                detail: describe_failure(resp).await,
            }),
            code => {
                let detail = describe_failure(resp).await;
                let detail = if detail.is_empty() { String::new() } else { format!(" ({detail})") };
                Err(AppError::Api(format!("{path} returned HTTP {code}{detail}")))
            }
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> AppResult<T> {
        let resp = self
            .http
            .get(format!("{}{path}", self.base))
            .header("x-api-key", &self.key)
            .header("Accept", "application/json")
            .query(query)
            .send()
            .await?;
        let resp = Self::ensure_ok(resp, path).await?;
        Self::parse(resp, path).await
    }

    /// Reads a JSON body, reporting exactly what didn't match if parsing fails.
    async fn parse<T: DeserializeOwned>(resp: reqwest::Response, path: &str) -> AppResult<T> {
        let text = resp.text().await?;
        serde_json::from_str(&text)
            .map_err(|e| AppError::Api(format!("unexpected data from {path}: {e}")))
    }

    async fn probe(&self, path: &str, query: &[(&str, &str)]) -> Probe {
        let endpoint = if query.is_empty() {
            path.to_string()
        } else {
            let q: Vec<String> = query.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!("{path}?{}", q.join("&"))
        };
        let sent = self
            .http
            .get(format!("{}{path}", self.base))
            .header("x-api-key", &self.key)
            .header("Accept", "application/json")
            .query(query)
            .send()
            .await;
        match sent {
            Ok(resp) if resp.status().is_success() => {
                Probe { ok: true, status: Some(resp.status().as_u16()), endpoint, detail: String::new() }
            }
            Ok(resp) => {
                let status = resp.status().as_u16();
                Probe { ok: false, status: Some(status), endpoint, detail: describe_failure(resp).await }
            }
            Err(e) => Probe { ok: false, status: None, endpoint, detail: e.to_string() },
        }
    }

    /// Tests the key against a general endpoint and the WoW addon search.
    pub async fn diagnose_key(&self) -> KeyCheck {
        let wow_game = WOW_GAME_ID.to_string();
        let general_q = [("pageSize", "1")];
        let wow_q = [("gameId", wow_game.as_str()), ("pageSize", "1")];
        let (general, wow) = tokio::join!(
            self.probe("/v1/games", &general_q),
            self.probe("/v1/mods/search", &wow_q),
        );
        KeyCheck::new(general, wow)
    }

    pub async fn version_types(&self) -> AppResult<Vec<GameVersionType>> {
        let e: Envelope<Vec<GameVersionType>> = self
            .get(&format!("/v1/games/{WOW_GAME_ID}/version-types"), &[])
            .await?;
        Ok(e.data)
    }

    pub async fn categories(&self) -> AppResult<Vec<Category>> {
        let e: Envelope<Vec<Category>> = self
            .get("/v1/categories", &[("gameId", WOW_GAME_ID.to_string())])
            .await?;
        Ok(e.data)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn search(
        &self,
        type_id: u32,
        class_id: Option<u32>,
        category_id: Option<u32>,
        query: &str,
        sort_field: u8,
        index: u32,
    ) -> AppResult<(Vec<Mod>, Pagination)> {
        let sort_order = if sort_field == 4 { "asc" } else { "desc" };
        let mut q: Vec<(&str, String)> = vec![
            ("gameId", WOW_GAME_ID.to_string()),
            ("gameVersionTypeId", type_id.to_string()),
            ("sortField", sort_field.to_string()),
            ("sortOrder", sort_order.to_string()),
            ("index", index.to_string()),
            ("pageSize", PAGE_SIZE.to_string()),
        ];
        if let Some(c) = class_id {
            q.push(("classId", c.to_string()));
        }
        if let Some(c) = category_id {
            q.push(("categoryId", c.to_string()));
        }
        let query = query.trim();
        if !query.is_empty() {
            q.push(("searchFilter", query.to_string()));
        }
        let p: Paged<Mod> = self.get("/v1/mods/search", &q).await?;
        Ok((p.data, p.pagination))
    }

    pub async fn get_mod(&self, mod_id: u32) -> AppResult<Mod> {
        let e: Envelope<Mod> = self.get(&format!("/v1/mods/{mod_id}"), &[]).await?;
        Ok(e.data)
    }

    pub async fn get_mods(&self, ids: &[u32]) -> AppResult<Vec<Mod>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let resp = self
            .http
            .post(format!("{}/v1/mods", self.base))
            .header("x-api-key", &self.key)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "modIds": ids, "filterPcOnly": true }))
            .send()
            .await?;
        let resp = Self::ensure_ok(resp, "/v1/mods").await?;
        // Parsed like a page so one malformed addon is skipped, not fatal.
        let page: Paged<Mod> = Self::parse(resp, "/v1/mods").await?;
        Ok(page.data)
    }

    /// Looks up addon folders by fingerprint (POST /v1/fingerprints/{gameId}).
    pub async fn fingerprint_matches(&self, fingerprints: &[u32]) -> AppResult<FingerprintMatches> {
        let path = format!("/v1/fingerprints/{WOW_GAME_ID}");
        let resp = self
            .http
            .post(format!("{}{path}", self.base))
            .header("x-api-key", &self.key)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "fingerprints": fingerprints }))
            .send()
            .await?;
        let resp = Self::ensure_ok(resp, &path).await?;
        let e: Envelope<FingerprintMatches> = Self::parse(resp, &path).await?;
        Ok(e.data)
    }

    pub async fn description(&self, mod_id: u32) -> AppResult<String> {
        let e: Envelope<String> = self
            .get(&format!("/v1/mods/{mod_id}/description"), &[])
            .await?;
        Ok(e.data)
    }

    pub async fn files(&self, mod_id: u32, type_id: u32) -> AppResult<Vec<File>> {
        let p: Paged<File> = self
            .get(
                &format!("/v1/mods/{mod_id}/files"),
                &[
                    ("gameVersionTypeId", type_id.to_string()),
                    ("pageSize", "50".to_string()),
                ],
            )
            .await?;
        Ok(p.data)
    }

    /// Returns `None` when the author disallows third-party downloads.
    pub async fn download_url(&self, mod_id: u32, file_id: u32) -> AppResult<Option<String>> {
        let path = format!("/v1/mods/{mod_id}/files/{file_id}/download-url");
        let resp = self
            .http
            .get(format!("{}{path}", self.base))
            .header("x-api-key", &self.key)
            .send()
            .await?;
        // 403/404 here means the author disallows third-party downloads.
        if matches!(resp.status().as_u16(), 403 | 404) {
            return Ok(None);
        }
        let resp = Self::ensure_ok(resp, &path).await?;
        let e: Envelope<Option<String>> = Self::parse(resp, &path).await?;
        Ok(e.data.filter(|s| !s.is_empty()))
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn sanitizes_pasted_keys() {
        let k = "$2a$10$abcDEF123";
        assert_eq!(sanitize_key(k).unwrap(), k);
        assert_eq!(sanitize_key(&format!("  {k}\r\n")).unwrap(), k);
        assert_eq!(sanitize_key(&format!("\"{k}\"")).unwrap(), k);
        assert_eq!(sanitize_key(&format!("x-api-key: {k}")).unwrap(), k);
        assert_eq!(sanitize_key("$2a$10$ abc\u{200B}DEF123").unwrap(), k);
        assert_eq!(sanitize_key("   ").unwrap(), "");
        assert!(sanitize_key("abcé123").is_err());
    }

    fn probe(ok: bool, status: Option<u16>, detail: &str) -> Probe {
        Probe { ok, status, endpoint: "/x".into(), detail: detail.into() }
    }

    #[test]
    fn hints_match_the_failure() {
        let ok = probe(true, Some(200), "");
        let c = KeyCheck::new(ok.clone(), ok.clone());
        assert!(c.hint.contains("works") && !c.has_http_failure());
        let c = KeyCheck::new(ok.clone(), probe(false, Some(403), "response: Forbidden"));
        assert!(c.hint.contains("World of Warcraft") && c.has_http_failure());
        let c = KeyCheck::new(probe(false, Some(401), ""), probe(false, Some(401), ""));
        assert!(c.hint.contains("doesn't recognize"));
        let c = KeyCheck::new(probe(false, Some(403), "x-cache: Error from cloudfront"), probe(false, Some(403), ""));
        assert!(c.hint.contains("revoked") && c.hint.contains("CDN"));
        let c = KeyCheck::new(probe(false, Some(429), ""), ok.clone());
        assert!(c.hint.contains("rate-limiting"));
        let c = KeyCheck::new(probe(false, None, "dns error"), probe(false, None, "dns error"));
        assert!(c.hint.contains("Couldn't reach") && !c.has_http_failure());
    }

    /// Minimal fake CurseForge: "goodkey" may use /v1/games, WoW search is
    /// refused by the "CDN", and any other key gets 401.
    async fn fake_curseforge() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { return };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
                    let good = req.lines().any(|l| l.eq_ignore_ascii_case("x-api-key: goodkey"));
                    let (status, extra, body) = if !good {
                        ("401 Unauthorized", "", "{\"error\":\"Unauthorized\"}")
                    } else if path.starts_with("/v1/mods/search") {
                        ("403 Forbidden", "x-cache: Error from cloudfront\r\n", "{\"message\":\"Forbidden\"}")
                    } else {
                        ("200 OK", "", "{\"data\":[]}")
                    };
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        format!("http://{addr}")
    }

    fn local_client() -> reqwest::Client {
        reqwest::Client::builder().no_proxy().build().unwrap()
    }

    #[tokio::test]
    async fn diagnoses_wow_refusal_and_bad_key() {
        let base = fake_curseforge().await;

        let good = CurseForge::with_base(local_client(), "goodkey".into(), base.clone());
        let c = good.diagnose_key().await;
        assert!(c.general.ok, "{c:?}");
        assert!(!c.wow.ok);
        assert_eq!(c.wow.status, Some(403));
        assert!(c.wow.detail.contains("Forbidden") && c.wow.detail.contains("cloudfront"), "{}", c.wow.detail);
        assert!(c.hint.contains("World of Warcraft"));

        let bad = CurseForge::with_base(local_client(), "nope".into(), base.clone());
        let c = bad.diagnose_key().await;
        assert_eq!(c.general.status, Some(401));
        assert!(c.hint.contains("doesn't recognize"));

        // Regular calls keep the status and CurseForge's answer in the error.
        let err = bad.version_types().await.unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, AppError::Rejected { status: 401, .. }), "{msg}");
        assert!(msg.contains("HTTP 401") && msg.contains("Unauthorized"), "{msg}");
        let err = good.search(1, None, None, "", 2, 0).await.unwrap_err().to_string();
        assert!(err.contains("HTTP 403") && err.contains("cloudfront"), "{err}");
    }

    fn file(id: u32, date: &str, rt: u8, types: &[u32]) -> File {
        File {
            id,
            file_date: date.into(),
            release_type: rt,
            sortable_game_versions: types
                .iter()
                .map(|t| SortableGameVersion { game_version_type_id: Some(*t) })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn finds_forever_type_or_none() {
        let types = vec![
            GameVersionType { id: 517, name: "WoW Retail".into(), slug: "wow_retail".into() },
            GameVersionType { id: 88568, name: "WoW Forever".into(), slug: "wow-forever".into() },
        ];
        assert_eq!(find_forever_type(&types), Some(88568));
        assert_eq!(find_forever_type(&types[..1]), None);
    }

    #[test]
    fn picks_newest_release_for_type() {
        let files = vec![
            file(1, "2026-11-01T00:00:00Z", 1, &[88568]),
            file(2, "2026-11-05T00:00:00Z", 2, &[88568]),
            file(3, "2026-11-09T00:00:00Z", 1, &[517]),
            file(4, "2026-11-03T00:00:00Z", 1, &[88568, 517]),
        ];
        assert_eq!(pick_best_file(&files, 88568, 1).unwrap().id, 4);
        assert_eq!(pick_best_file(&files, 88568, 2).unwrap().id, 2);
        assert_eq!(pick_best_file(&files, 517, 1).unwrap().id, 3);
        assert!(pick_best_file(&files, 999, 3).is_none());
    }

    #[test]
    fn falls_back_to_any_channel() {
        let files = vec![file(9, "2026-11-01T00:00:00Z", 3, &[88568])];
        assert_eq!(pick_best_file(&files, 88568, 1).unwrap().id, 9);
    }

    #[test]
    fn skips_unavailable_files() {
        let mut f = file(5, "2026-11-10T00:00:00Z", 1, &[88568]);
        f.is_available = Some(false);
        let files = vec![f, file(4, "2026-11-01T00:00:00Z", 1, &[88568])];
        assert_eq!(pick_best_file(&files, 88568, 1).unwrap().id, 4);
    }

    #[test]
    fn category_tree_from_fixture() {
        let json = r#"[
          {"id":1,"name":"Addons","slug":"addons","isClass":true,"classId":null,"parentCategoryId":null},
          {"id":10,"name":"Bags & Inventory","slug":"bags","classId":1,"parentCategoryId":1},
          {"id":11,"name":"Unit Frames","slug":"unit-frames","classId":1,"parentCategoryId":1},
          {"id":12,"name":"Party","slug":"party","classId":1,"parentCategoryId":11},
          {"id":99,"name":"Other class","slug":"other","isClass":true},
          {"id":100,"name":"Not an addon","slug":"x","classId":99,"parentCategoryId":99}
        ]"#;
        let cats: Vec<Category> = serde_json::from_str(json).unwrap();
        let (cls, tree) = build_category_tree(&cats);
        assert_eq!(cls, Some(1));
        let names: Vec<_> = tree.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["Bags & Inventory", "Unit Frames"]);
        assert_eq!(tree[1].children[0].name, "Party");
    }

    #[test]
    fn category_tree_without_class() {
        let json = r#"[
          {"id":10,"name":"B","slug":"b","parentCategoryId":null},
          {"id":11,"name":"A","slug":"a","parentCategoryId":0},
          {"id":12,"name":"A1","slug":"a1","parentCategoryId":11}
        ]"#;
        let cats: Vec<Category> = serde_json::from_str(json).unwrap();
        let (cls, tree) = build_category_tree(&cats);
        assert_eq!(cls, None);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].name, "A");
        assert_eq!(tree[0].children.len(), 1);
    }

    #[test]
    fn latest_index_respects_type_and_channel() {
        let m: Mod = serde_json::from_str(
            r#"{"id":1,"latestFilesIndexes":[
              {"fileId":10,"filename":"a.zip","releaseType":1,"gameVersionTypeId":88568},
              {"fileId":12,"filename":"b.zip","releaseType":2,"gameVersionTypeId":88568},
              {"fileId":20,"filename":"c.zip","releaseType":1,"gameVersionTypeId":517}
            ]}"#,
        )
        .unwrap();
        assert_eq!(latest_index_for_type(&m, 88568, 1).unwrap().file_id, 10);
        assert_eq!(latest_index_for_type(&m, 88568, 2).unwrap().file_id, 12);
        assert!(latest_index_for_type(&m, 1, 3).is_none());
    }
}

#[cfg(test)]
mod live_fixtures {
    use super::*;

    /// Parses saved real search responses with the app's types:
    /// `EZADDON_FIXTURES=/path/to/dir cargo test parses_saved -- --ignored`
    #[test]
    #[ignore]
    fn parses_saved_search_responses() {
        let dir = std::env::var("EZADDON_FIXTURES").expect("set EZADDON_FIXTURES");
        let mut failures = 0;
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap();
            if let Err(err) = serde_json::from_str::<Paged<Mod>>(&text) {
                failures += 1;
                println!("{}: {err}", e.file_name().to_string_lossy());
            }
        }
        assert_eq!(failures, 0);
    }
}
