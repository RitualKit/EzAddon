//! installed.json: what EzAddon installed and which folders each addon owns.
//! Entries are grouped by AddOns folder so switching folders keeps both lists.

use crate::error::AppResult;
use crate::settings::write_atomic;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAddon {
    pub mod_id: u32,
    pub name: String,
    pub file_id: u32,
    pub file_name: String,
    pub display_name: String,
    pub folders: Vec<String>,
    pub installed_at: u64,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub website_url: Option<String>,
    /// Installed automatically as another addon's required dependency.
    #[serde(default)]
    pub is_dependency: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    /// AddOns path → installed addons keyed by mod id.
    #[serde(default)]
    pub folders: BTreeMap<String, BTreeMap<u32, InstalledAddon>>,
}

fn key(addons: &Path) -> String {
    addons.to_string_lossy().to_lowercase()
}

impl Manifest {
    pub fn load(path: &Path) -> Manifest {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self)?)
    }

    pub fn entries(&self, addons: &Path) -> Vec<InstalledAddon> {
        let mut v: Vec<_> = self
            .folders
            .get(&key(addons))
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        v.sort_by_key(|a| a.name.to_lowercase());
        v
    }

    pub fn get(&self, addons: &Path, mod_id: u32) -> Option<&InstalledAddon> {
        self.folders.get(&key(addons))?.get(&mod_id)
    }

    pub fn put(&mut self, addons: &Path, entry: InstalledAddon) {
        self.folders.entry(key(addons)).or_default().insert(entry.mod_id, entry);
    }

    pub fn remove(&mut self, addons: &Path, mod_id: u32) -> Option<InstalledAddon> {
        self.folders.get_mut(&key(addons))?.remove(&mod_id)
    }

    /// True if an addon other than `except` owns this folder.
    pub fn folder_owned_by_other(&self, addons: &Path, folder: &str, except: u32) -> bool {
        self.folders
            .get(&key(addons))
            .map(|m| {
                m.values().any(|a| {
                    a.mod_id != except && a.folders.iter().any(|f| f.eq_ignore_ascii_case(folder))
                })
            })
            .unwrap_or(false)
    }

    /// Drops entries whose folders were all deleted outside EzAddon.
    /// Returns true if anything changed.
    pub fn prune_missing(&mut self, addons: &Path) -> bool {
        let Some(m) = self.folders.get_mut(&key(addons)) else {
            return false;
        };
        let before = m.len();
        m.retain(|_, a| a.folders.iter().any(|f| addons.join(f).is_dir()));
        before != m.len()
    }
}

pub fn manifest_path(data_dir: &Path) -> PathBuf {
    data_dir.join("installed.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u32, folders: &[&str]) -> InstalledAddon {
        InstalledAddon {
            mod_id: id,
            name: format!("Addon {id}"),
            file_id: 100 + id,
            file_name: "a.zip".into(),
            display_name: "a".into(),
            folders: folders.iter().map(|s| s.to_string()).collect(),
            installed_at: 1,
            thumbnail: None,
            website_url: None,
            is_dependency: false,
        }
    }

    #[test]
    fn round_trip_and_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let addons = dir.path().join("AddOns");
        std::fs::create_dir_all(addons.join("Lib")).unwrap();
        let mut m = Manifest::default();
        m.put(&addons, entry(1, &["Lib"]));
        m.put(&addons, entry(2, &["Lib", "Gone"]));
        m.put(&addons, entry(3, &["Gone"]));

        let p = manifest_path(dir.path());
        m.save(&p).unwrap();
        let mut back = Manifest::load(&p);
        assert_eq!(back.entries(&addons).len(), 3);
        assert_eq!(back.get(&addons, 2), m.get(&addons, 2));

        assert!(back.folder_owned_by_other(&addons, "lib", 1));
        assert!(back.folder_owned_by_other(&addons, "Gone", 2));
        assert!(!back.folder_owned_by_other(&addons, "Other", 1));

        assert!(back.prune_missing(&addons));
        let ids: Vec<u32> = back.entries(&addons).iter().map(|a| a.mod_id).collect();
        assert_eq!(ids, vec![1, 2]);
    }
}
