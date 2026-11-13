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

#[derive(Debug, Clone, Deserialize)]
struct Paged<T> {
    data: Vec<T>,
    #[serde(default)]
    pagination: Pagination,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Category {
    pub id: u32,
    pub name: String,
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
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileIndex {
    pub game_version: String,
    pub file_id: u32,
    pub filename: String,
    pub release_type: u8,
    pub game_version_type_id: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Mod {
    pub id: u32,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub download_count: f64,
    pub links: Links,
    pub logo: Option<Asset>,
    pub screenshots: Vec<Asset>,
    pub authors: Vec<Author>,
    pub categories: Vec<Category>,
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
    pub display_name: String,
    pub file_name: String,
    pub release_type: u8,
    pub file_date: String,
    pub file_length: u64,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
    pub sortable_game_versions: Vec<SortableGameVersion>,
    pub dependencies: Vec<Dependency>,
    pub is_available: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameVersionType {
    pub id: u32,
    pub name: String,
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

#[derive(Clone)]
pub struct CurseForge {
    http: reqwest::Client,
    key: String,
}

impl CurseForge {
    pub fn new(http: reqwest::Client, key: String) -> Self {
        CurseForge { http, key }
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

    fn check_status(status: reqwest::StatusCode, path: &str) -> AppResult<()> {
        match status.as_u16() {
            200..=299 => Ok(()),
            401 | 403 => Err(AppError::InvalidApiKey),
            code => Err(AppError::Api(format!("{path} returned HTTP {code}"))),
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> AppResult<T> {
        let resp = self
            .http
            .get(format!("{API_BASE}{path}"))
            .header("x-api-key", &self.key)
            .header("Accept", "application/json")
            .query(query)
            .send()
            .await?;
        Self::check_status(resp.status(), path)?;
        Ok(resp.json::<T>().await?)
    }

    /// Checks the key with a cheap request.
    pub async fn validate_key(&self) -> AppResult<()> {
        let _: serde_json::Value = self.get(&format!("/v1/games/{WOW_GAME_ID}"), &[]).await?;
        Ok(())
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
            .post(format!("{API_BASE}/v1/mods"))
            .header("x-api-key", &self.key)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "modIds": ids, "filterPcOnly": true }))
            .send()
            .await?;
        Self::check_status(resp.status(), "/v1/mods")?;
        let e: Envelope<Vec<Mod>> = resp.json().await?;
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
            .get(format!("{API_BASE}{path}"))
            .header("x-api-key", &self.key)
            .send()
            .await?;
        match resp.status().as_u16() {
            403 | 404 => return Ok(None),
            _ => Self::check_status(resp.status(), &path)?,
        }
        let e: Envelope<Option<String>> = resp.json().await?;
        Ok(e.data.filter(|s| !s.is_empty()))
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
