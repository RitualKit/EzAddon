//! User settings (settings.json in the app config folder) and the API key.
//!
//! On Windows the API key lives in Windows Credential Manager. On other
//! platforms, which are only used for development, it is kept in settings.json.

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// The WoW Forever `Interface\AddOns` folder.
    pub addons_path: Option<PathBuf>,
    /// 1 = release only, 2 = also beta, 3 = also alpha.
    pub release_channel: u8,
    /// Overrides the CurseForge WoW Forever game-version type id.
    pub version_type_override: Option<u32>,
    /// Closing the window keeps EzAddon in the system tray instead of exiting.
    pub close_to_tray: bool,
    /// Check for addon updates in the background and show a notification.
    pub notify_updates: bool,
    #[cfg(not(windows))]
    pub api_key: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            addons_path: None,
            release_channel: 1,
            version_type_override: None,
            close_to_tray: true,
            notify_updates: true,
            #[cfg(not(windows))]
            api_key: None,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .map(|mut s| {
                s.release_channel = s.release_channel.clamp(1, 3);
                s
            })
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self)?)
    }
}

/// Writes to a temporary file, then renames it over the target, so a crash
/// can't leave a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ---------- API key storage ----------

#[cfg(windows)]
mod key_store {
    use super::*;
    use crate::error::AppError;
    const SERVICE: &str = "EzAddon";
    const USER: &str = "curseforge-api-key";

    fn entry() -> AppResult<keyring::Entry> {
        keyring::Entry::new(SERVICE, USER).map_err(|e| AppError::Other(e.to_string()))
    }

    pub fn get(_s: &Settings) -> Option<String> {
        entry().ok()?.get_password().ok().filter(|k| !k.is_empty())
    }

    pub fn set(_s: &mut Settings, key: Option<&str>) -> AppResult<()> {
        let e = entry()?;
        match key {
            Some(k) => e.set_password(k).map_err(|e| AppError::Other(e.to_string())),
            None => match e.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(err) => Err(AppError::Other(err.to_string())),
            },
        }
    }
}

#[cfg(not(windows))]
mod key_store {
    use super::*;

    pub fn get(s: &Settings) -> Option<String> {
        s.api_key.clone().filter(|k| !k.is_empty())
    }

    pub fn set(s: &mut Settings, key: Option<&str>) -> AppResult<()> {
        s.api_key = key.map(str::to_string);
        Ok(())
    }
}

pub fn get_api_key(s: &Settings) -> Option<String> {
    std::env::var("EZADDON_CF_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
        .or_else(|| key_store::get(s))
}

/// Stores the key, or removes it when `key` is `None`. The caller saves the
/// settings afterwards.
pub fn set_api_key(s: &mut Settings, key: Option<&str>) -> AppResult<()> {
    key_store::set(s, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_clamp() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cfg/settings.json");
        let s = Settings {
            addons_path: Some(PathBuf::from("/x/AddOns")),
            release_channel: 2,
            version_type_override: Some(5),
            ..Default::default()
        };
        s.save(&p).unwrap();
        let back = Settings::load(&p);
        assert_eq!(back.addons_path, s.addons_path);
        assert_eq!(back.release_channel, 2);
        assert_eq!(back.version_type_override, Some(5));

        std::fs::write(&p, br#"{"releaseChannel":9}"#).unwrap();
        assert_eq!(Settings::load(&p).release_channel, 3);
        std::fs::write(&p, b"garbage").unwrap();
        assert_eq!(Settings::load(&p).release_channel, 1);
    }
}
