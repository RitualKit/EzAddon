//! Finding the WoW Forever AddOns folder.

use std::path::{Path, PathBuf};

/// The flavor folder WoW Forever uses inside the World of Warcraft folder.
pub const FOREVER_DIR: &str = "_forever_";

/// `<root>\_forever_\Interface\AddOns`
pub fn addons_dir_for_root(root: &Path) -> PathBuf {
    root.join(FOREVER_DIR).join("Interface").join("AddOns")
}

fn eq_ignore_case(a: &std::ffi::OsStr, b: &str) -> bool {
    a.to_string_lossy().eq_ignore_ascii_case(b)
}

/// Turns any folder the user picks into an AddOns path. Accepted inputs are the
/// WoW root, the `_forever_` folder, its `Interface` folder or the `AddOns`
/// folder itself.
pub fn normalize_user_path(p: &Path) -> PathBuf {
    let name = p.file_name();
    match name {
        Some(n) if eq_ignore_case(n, "AddOns") => p.to_path_buf(),
        Some(n) if eq_ignore_case(n, "Interface") => p.join("AddOns"),
        Some(n) if eq_ignore_case(n, FOREVER_DIR) => p.join("Interface").join("AddOns"),
        _ => {
            if p.join(FOREVER_DIR).is_dir() {
                addons_dir_for_root(p)
            } else {
                // Another flavor folder (e.g. `_retail_`) was picked: use its parent.
                let looks_like_flavor = name
                    .map(|n| {
                        let s = n.to_string_lossy();
                        s.len() > 2 && s.starts_with('_') && s.ends_with('_')
                    })
                    .unwrap_or(false);
                match (looks_like_flavor, p.parent()) {
                    (true, Some(parent)) => addons_dir_for_root(parent),
                    _ => addons_dir_for_root(p),
                }
            }
        }
    }
}

/// A path is usable if the game folder above `Interface` exists. AddOns is
/// created on the first install if it is missing.
pub fn is_valid_addons_dir(p: &Path) -> bool {
    p.is_dir()
        || p
            .parent()
            .and_then(|interface| interface.parent())
            .map(|flavor| flavor.is_dir())
            .unwrap_or(false)
}

/// Strips a trailing flavor folder (`_retail_`, `_classic_`…) from a registry
/// InstallPath, giving the WoW root.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn root_from_install_path(p: &Path) -> PathBuf {
    let trimmed = PathBuf::from(p.to_string_lossy().trim_end_matches(['\\', '/']));
    match trimmed.file_name().map(|n| n.to_string_lossy().to_string()) {
        Some(n) if n.len() > 2 && n.starts_with('_') && n.ends_with('_') => {
            trimmed.parent().map(Path::to_path_buf).unwrap_or(trimmed)
        }
        _ => trimmed,
    }
}

fn candidate_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();

    #[cfg(windows)]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        for key in [
            r"SOFTWARE\WOW6432Node\Blizzard Entertainment\World of Warcraft",
            r"SOFTWARE\Blizzard Entertainment\World of Warcraft",
        ] {
            if let Ok(k) = hklm.open_subkey(key) {
                if let Ok(p) = k.get_value::<String, _>("InstallPath") {
                    out.push(root_from_install_path(Path::new(&p)));
                }
            }
        }
        for drive in ["C", "D", "E", "F"] {
            out.push(PathBuf::from(format!(r"{drive}:\Program Files (x86)\World of Warcraft")));
            out.push(PathBuf::from(format!(r"{drive}:\Program Files\World of Warcraft")));
            out.push(PathBuf::from(format!(r"{drive}:\World of Warcraft")));
            out.push(PathBuf::from(format!(r"{drive}:\Games\World of Warcraft")));
        }
    }

    #[cfg(not(windows))]
    {
        out.push(PathBuf::from("/Applications/World of Warcraft"));
        if let Some(home) = std::env::var_os("HOME") {
            out.push(PathBuf::from(home).join("Games/World of Warcraft"));
        }
    }

    out
}

/// Looks for an installed WoW Forever and returns its AddOns path.
pub fn detect_addons_dir() -> Option<PathBuf> {
    candidate_roots()
        .into_iter()
        .find(|root| root.join(FOREVER_DIR).is_dir())
        .map(|root| addons_dir_for_root(&root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_every_accepted_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("World of Warcraft");
        let addons = addons_dir_for_root(&root);
        std::fs::create_dir_all(&addons).unwrap();
        std::fs::create_dir_all(root.join("_retail_")).unwrap();

        assert_eq!(normalize_user_path(&root), addons);
        assert_eq!(normalize_user_path(&root.join("_forever_")), addons);
        assert_eq!(normalize_user_path(&root.join("_forever_/Interface")), addons);
        assert_eq!(normalize_user_path(&addons), addons);
        assert_eq!(normalize_user_path(&root.join("_retail_")), addons);
        assert!(is_valid_addons_dir(&addons));
    }

    #[test]
    fn missing_addons_folder_is_still_valid_if_game_folder_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("_forever_")).unwrap();
        assert!(is_valid_addons_dir(&addons_dir_for_root(root)));
        assert!(!is_valid_addons_dir(&tmp.path().join("nope/Interface/AddOns")));
    }

    #[test]
    fn strips_flavor_from_install_path() {
        assert_eq!(
            root_from_install_path(Path::new("/games/World of Warcraft/_retail_/")),
            PathBuf::from("/games/World of Warcraft")
        );
        assert_eq!(
            root_from_install_path(Path::new("/games/World of Warcraft")),
            PathBuf::from("/games/World of Warcraft")
        );
    }
}
