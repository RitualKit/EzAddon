//! Finds addons installed outside EzAddon and matches them to CurseForge.
//!
//! Each addon folder gets a fingerprint computed the same way CurseForge does
//! for WoW: MurmurHash2 (seed 1) of every file the folder's `.toc` files load
//! (directly or through XML includes), with whitespace bytes removed; those
//! hashes are sorted, joined as decimal text and hashed again. CurseForge's
//! fingerprint endpoint then says which mod and exact file each folder is.
//! Verified against the module fingerprints CurseForge publishes for real
//! WoW Forever files (including `_Forever.toc` variants).
//!
//! Folders that don't match but declare `## X-Curse-Project-ID` in their TOC
//! are adopted with an unknown version, so they still get update checks.

use crate::curseforge::{latest_index_for_type, CurseForge, File};
use crate::error::{AppError, AppResult};
use crate::manifest::{InstalledAddon, Manifest};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// MurmurHash2, 32-bit.
pub fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = seed ^ data.len() as u32;
    let (chunks, rest) = data.as_chunks::<4>();
    for c in chunks {
        let mut k = u32::from_le_bytes(*c);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
    }
    if rest.len() == 3 {
        h ^= (rest[2] as u32) << 16;
    }
    if rest.len() >= 2 {
        h ^= (rest[1] as u32) << 8;
    }
    if !rest.is_empty() {
        h ^= rest[0] as u32;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

/// Hash of a file with tabs, newlines, carriage returns and spaces removed.
fn file_hash(path: &Path) -> Option<u32> {
    let bytes = std::fs::read(path).ok()?;
    let normalized: Vec<u8> = bytes.into_iter().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
    Some(murmur2(&normalized, 1))
}

/// Resolves a path from a TOC/XML include, ignoring letter case (authors
/// often write `Libs\foo.lua` for a folder called `libs`).
fn resolve_ci(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut cur = base.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|p| !p.is_empty()) {
        let entries = std::fs::read_dir(&cur).ok()?;
        let hit = entries
            .flatten()
            .map(|e| e.file_name())
            .find(|n| n.to_string_lossy().eq_ignore_ascii_case(part))?;
        cur.push(hit);
    }
    Some(cur)
}

/// `Name.toc`, or `Name_Suffix.toc` / `Name-Suffix.toc` for flavor variants.
fn is_addon_toc(folder: &str, file: &str) -> bool {
    let (folder, file) = (folder.to_ascii_lowercase(), file.to_ascii_lowercase());
    let Some(stem) = file.strip_suffix(".toc") else { return false };
    if stem == folder {
        return true;
    }
    match stem.strip_prefix(&folder) {
        Some(rest) if rest.len() > 1 && (rest.starts_with('_') || rest.starts_with('-')) => {
            rest[1..].chars().all(|c| c.is_ascii_alphanumeric())
        }
        _ => false,
    }
}

/// Files a TOC loads: non-comment lines naming a `.lua` or `.xml` file.
fn toc_includes(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .filter(|l| {
            let lower = l.to_ascii_lowercase();
            (lower.ends_with(".lua") || lower.ends_with(".xml")) && !l.contains("..")
        })
        .map(str::to_string)
        .collect()
}

/// Files an XML loads: `<Include file="..."/>` and `<Script file="..."/>`.
fn xml_includes(text: &str) -> Vec<String> {
    // Drop comments first so commented-out includes don't count.
    let mut clean = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        clean.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    clean.push_str(rest);

    let lower = clean.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find('<') {
        let at = pos + i + 1;
        pos = at;
        let tag_len = if lower[at..].starts_with("include") {
            7
        } else if lower[at..].starts_with("script") {
            6
        } else {
            continue;
        };
        let mut j = at + tag_len;
        let ws = lower[j..].len() - lower[j..].trim_start().len();
        if ws == 0 || !lower[j + ws..].starts_with("file=") {
            continue;
        }
        j += ws + 5;
        if !clean[j..].starts_with(['"', '\'']) {
            continue;
        }
        j += 1;
        let Some(end) = clean[j..].find(['"', '\'']) else { continue };
        let value = &clean[j..j + end];
        let after = clean[j + end + 1..].trim_start();
        if after.starts_with("/>") && !value.is_empty() && !value.contains("..") {
            out.push(value.to_string());
        }
    }
    out
}

/// Every file that counts towards the folder fingerprint.
fn matching_files(folder: &Path) -> Vec<PathBuf> {
    let name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let Ok(entries) = std::fs::read_dir(folder) else { return vec![] };
    let mut tocs = Vec::new();
    let mut out: Vec<PathBuf> = Vec::new();
    for e in entries.flatten() {
        if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let file = e.file_name().to_string_lossy().to_string();
        if is_addon_toc(&name, &file) {
            tocs.push(e.path());
        } else if file.eq_ignore_ascii_case("bindings.xml") {
            out.push(e.path());
        }
    }
    tocs.sort();
    let mut seen: HashSet<PathBuf> = out.iter().cloned().collect();
    let mut stack: Vec<PathBuf> = tocs.into_iter().rev().collect();
    while let Some(p) = stack.pop() {
        if !seen.insert(p.clone()) || !p.is_file() {
            continue;
        }
        out.push(p.clone());
        let lower = p.to_string_lossy().to_ascii_lowercase();
        let includes = if lower.ends_with(".toc") || lower.ends_with(".xml") {
            let text = String::from_utf8_lossy(&std::fs::read(&p).unwrap_or_default()).to_string();
            if lower.ends_with(".toc") { toc_includes(&text) } else { xml_includes(&text) }
        } else {
            vec![]
        };
        let dir = p.parent().unwrap_or(folder).to_path_buf();
        for inc in includes.into_iter().rev() {
            if let Some(resolved) = resolve_ci(&dir, inc.trim()) {
                stack.push(resolved);
            }
        }
    }
    out
}

/// CurseForge-compatible fingerprint of an addon folder, or `None` if the
/// folder has no TOC (so isn't an addon).
pub fn folder_fingerprint(folder: &Path) -> Option<u32> {
    let files = matching_files(folder);
    if !files.iter().any(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("toc"))) {
        return None;
    }
    let mut hashes: Vec<u32> = files.iter().filter_map(|f| file_hash(f)).collect();
    hashes.sort_unstable();
    let joined: String = hashes.iter().map(|h| h.to_string()).collect();
    Some(murmur2(joined.as_bytes(), 1))
}

/// `## X-Curse-Project-ID: 12345` from any of the folder's TOC files.
pub fn toc_project_id(folder: &Path) -> Option<u32> {
    let name = folder.file_name()?.to_string_lossy().to_string();
    for e in std::fs::read_dir(folder).ok()?.flatten() {
        let file = e.file_name().to_string_lossy().to_string();
        if !is_addon_toc(&name, &file) {
            continue;
        }
        let text = String::from_utf8_lossy(&std::fs::read(e.path()).ok()?).to_string();
        for line in text.lines() {
            let line = line.trim_start_matches('#').trim();
            if let Some((k, v)) = line.split_once(':') {
                if k.trim().eq_ignore_ascii_case("X-Curse-Project-ID") {
                    if let Ok(id) = v.trim().parse() {
                        return Some(id);
                    }
                }
            }
        }
    }
    None
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    /// Addons newly added to EzAddon.
    pub added: Vec<InstalledAddon>,
    /// Folders that couldn't be matched to CurseForge.
    pub unmatched: Vec<String>,
    /// Folders skipped because EzAddon already manages them.
    pub already_managed: usize,
    /// Addon folders looked at.
    pub scanned: usize,
}

struct Folder {
    name: String,
    fingerprint: Option<u32>,
    project_id: Option<u32>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Scans `addons` and adds every addon CurseForge recognizes to `manifest`.
pub async fn scan(cf: &CurseForge, addons: &Path, manifest: &mut Manifest, type_id: u32, channel: u8) -> AppResult<ScanReport> {
    let managed: HashSet<String> = manifest
        .entries(addons)
        .iter()
        .flat_map(|a| a.folders.iter().map(|f| f.to_ascii_lowercase()))
        .collect();

    // Reading and hashing files is blocking work.
    let dir = addons.to_path_buf();
    let managed_c = managed.clone();
    let (folders, already_managed) = tauri::async_runtime::spawn_blocking(move || {
        let mut folders = Vec::new();
        let mut already = 0;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name.starts_with("Blizzard_") {
                    continue;
                }
                if managed_c.contains(&name.to_ascii_lowercase()) {
                    already += 1;
                    continue;
                }
                let path = e.path();
                let fingerprint = folder_fingerprint(&path);
                if fingerprint.is_none() {
                    continue; // no TOC: not an addon
                }
                folders.push(Folder { name, fingerprint, project_id: toc_project_id(&path) });
            }
        }
        folders.sort_by_key(|f| f.name.to_lowercase());
        (folders, already)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?;

    // 1. Exact identification by fingerprint.
    let mut claimed: HashMap<String, u32> = HashMap::new(); // folder → mod id
    let mut files: HashMap<u32, File> = HashMap::new(); // mod id → matched file
    let fps: Vec<u32> = folders.iter().filter_map(|f| f.fingerprint).collect();
    for chunk in fps.chunks(500) {
        let m = cf.fingerprint_matches(chunk).await?;
        for hit in m.exact_matches.into_iter().chain(m.partial_matches) {
            for module in &hit.file.modules {
                for f in &folders {
                    if f.fingerprint.map(u64::from) == Some(module.fingerprint) && !claimed.contains_key(&f.name) {
                        claimed.insert(f.name.clone(), hit.id);
                        files.entry(hit.id).or_insert_with(|| hit.file.clone());
                    }
                }
            }
        }
    }

    // 2. Fallback: the project id the addon's TOC declares (version unknown).
    for f in &folders {
        if let (false, Some(pid)) = (claimed.contains_key(&f.name), f.project_id) {
            claimed.insert(f.name.clone(), pid);
        }
    }

    let mut by_mod: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (folder, id) in &claimed {
        by_mod.entry(*id).or_default().push(folder.clone());
    }
    let ids: Vec<u32> = by_mod.keys().copied().collect();
    let mut mods = HashMap::new();
    for chunk in ids.chunks(100) {
        for m in cf.get_mods(chunk).await? {
            mods.insert(m.id, m);
        }
    }

    let mut added = Vec::new();
    let mut unmatched: Vec<String> = folders.iter().filter(|f| !claimed.contains_key(&f.name)).map(|f| f.name.clone()).collect();
    for (id, mut folder_names) in by_mod {
        let Some(m) = mods.get(&id) else {
            unmatched.extend(folder_names);
            continue;
        };
        let file = files.get(&id);
        // Version-unknown adoptions only make sense if a Forever build exists.
        if file.is_none() && latest_index_for_type(m, type_id, channel).is_none() {
            unmatched.extend(folder_names);
            continue;
        }
        folder_names.sort();
        let entry = InstalledAddon {
            mod_id: id,
            name: m.name.clone(),
            file_id: file.map(|f| f.id).unwrap_or(0),
            file_name: file.map(|f| f.file_name.clone()).unwrap_or_default(),
            display_name: file.map(|f| f.display_name.clone()).unwrap_or_else(|| "Unknown version".into()),
            folders: folder_names,
            installed_at: now_secs(),
            thumbnail: m.logo.as_ref().and_then(|l| l.thumbnail_url.clone()),
            website_url: m.links.website_url.clone(),
            is_dependency: false,
        };
        manifest.put(addons, entry.clone());
        added.push(entry);
    }
    unmatched.sort_by_key(|n| n.to_lowercase());
    added.sort_by_key(|a| a.name.to_lowercase());
    Ok(ScanReport { scanned: folders.len() + already_managed, already_managed, added, unmatched })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    /// Builds a small addon; the expected value comes from the reference
    /// implementation that matched 31/31 real CurseForge module fingerprints.
    fn sample_addon(root: &Path) -> PathBuf {
        let a = root.join("MyAddon");
        write(&a.join("MyAddon.toc"), "## Interface: 16001\n## Title: My Addon\n# comment.lua\nCore.lua\nlibs\\Libs.xml\n");
        write(&a.join("MyAddon_Forever.toc"), "## Interface: 16001\nCore.lua\nForever.lua\n");
        write(&a.join("Core.lua"), "local x = 1\r\nprint( x )\n");
        write(&a.join("Forever.lua"), "-- forever only\n");
        write(&a.join("Libs/Libs.xml"), "<Ui>\n<!-- <Script file=\"Old.lua\"/> -->\n<Script file=\"LibA.lua\"/>\n<Include file='Sub\\Sub.xml' />\n</Ui>\n");
        write(&a.join("Libs/LibA.lua"), "return 42\n");
        write(&a.join("Libs/sub/Sub.xml"), "<Ui><Script file=\"s.lua\"/></Ui>");
        write(&a.join("Libs/sub/s.lua"), "s=1");
        write(&a.join("Bindings.xml"), "<Bindings/>");
        write(&a.join("Unused.lua"), "not loaded");
        write(&a.join("Libs/Old.lua"), "commented out");
        a
    }

    #[test]
    fn murmur2_reference_values() {
        assert_eq!(murmur2(b"", 1), 1_540_447_798);
        assert_eq!(murmur2(b"a", 1), 626_045_324);
        assert_eq!(murmur2(b"abcd", 1), 3_376_380_438);
        assert_eq!(murmur2(b"hello world", 1), 2_213_174_766);
    }

    #[test]
    fn folder_fingerprint_matches_reference() {
        let d = tempfile::tempdir().unwrap();
        let a = sample_addon(d.path());
        let mut names: Vec<String> = matching_files(&a)
            .iter()
            .map(|p| p.strip_prefix(&a).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["Bindings.xml", "Core.lua", "Forever.lua", "Libs/LibA.lua", "Libs/Libs.xml", "Libs/sub/Sub.xml", "Libs/sub/s.lua", "MyAddon.toc", "MyAddon_Forever.toc"]
        );
        assert_eq!(folder_fingerprint(&a), Some(REFERENCE_FINGERPRINT));
    }

    #[test]
    fn toc_name_rules_and_project_id() {
        assert!(is_addon_toc("Plater", "Plater.toc"));
        assert!(is_addon_toc("Plater", "plater_Forever.toc"));
        assert!(is_addon_toc("Plater", "Plater-Mainline.toc"));
        assert!(!is_addon_toc("Plater", "PlaterOptions.toc"));
        assert!(!is_addon_toc("Plater", "Plater_.toc"));

        let d = tempfile::tempdir().unwrap();
        write(&d.path().join("X/X.toc"), "## Title: X\n## X-Curse-Project-ID: 12345\n");
        assert_eq!(toc_project_id(&d.path().join("X")), Some(12345));
        write(&d.path().join("Y/Y.toc"), "## Title: Y\n");
        assert_eq!(toc_project_id(&d.path().join("Y")), None);
        assert_eq!(folder_fingerprint(&d.path().join("Z")), None);
    }

    const REFERENCE_FINGERPRINT: u32 = 2_656_195_407;
}

/// Live check against the real API, run by hand:
/// `EZADDON_LIVE_ADDONS=/path/to/AddOns cargo test live_scan -- --ignored --nocapture`
#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn live_scan() {
        let dir = PathBuf::from(std::env::var("EZADDON_LIVE_ADDONS").expect("set EZADDON_LIVE_ADDONS"));
        let key = std::env::var("EZADDON_CF_API_KEY").unwrap_or_else(|_| "proxy-injected".into());
        let cf = CurseForge::new(CurseForge::http_client(), key);
        let mut manifest = Manifest::default();
        let report = scan(&cf, &dir, &mut manifest, 88568, 1).await.unwrap();
        for a in &report.added {
            println!("ADDED {} (mod {}, file {} {:?}) folders {:?}", a.name, a.mod_id, a.file_id, a.display_name, a.folders);
        }
        println!("UNMATCHED {:?}; scanned {}", report.unmatched, report.scanned);
        assert!(!report.added.is_empty());
    }
}
