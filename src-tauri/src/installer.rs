//! Planning, downloading and safely unpacking addons into the AddOns folder.

use crate::curseforge::{File, RELATION_REQUIRED};
use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use std::collections::HashSet;
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Refuse archives that would unpack to more than this (zip-bomb guard).
const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;

// ---------- Dependency planning ----------

/// Supplies the best WoW Forever file for a mod. The real implementation calls
/// CurseForge; the tests use a fixture.
#[allow(async_fn_in_trait)]
pub trait FileSource {
    async fn best_file(&self, mod_id: u32) -> AppResult<Option<File>>;
}

#[derive(Debug, Clone)]
pub struct Step {
    pub mod_id: u32,
    pub file: File,
}

#[derive(Debug, Default)]
pub struct Plan {
    /// Install order: dependencies first, the requested addon last.
    pub steps: Vec<Step>,
    /// Required dependencies that have no WoW Forever file.
    pub missing_dependencies: Vec<u32>,
}

/// Works out what to install for `root`. Required dependencies are followed
/// recursively. Those already installed are skipped and cycles are ignored.
pub async fn plan_install<S: FileSource>(
    src: &S,
    root: u32,
    installed: &HashSet<u32>,
) -> AppResult<Plan> {
    enum Frame {
        Enter(u32),
        Exit(u32, Box<File>),
    }
    let mut plan = Plan::default();
    let mut seen = HashSet::new();
    let mut stack = vec![Frame::Enter(root)];
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Enter(id) => {
                if !seen.insert(id) || (id != root && installed.contains(&id)) {
                    continue;
                }
                match src.best_file(id).await? {
                    None if id == root => return Err(AppError::NoForeverFile),
                    None => plan.missing_dependencies.push(id),
                    Some(file) => {
                        let deps: Vec<u32> = file
                            .dependencies
                            .iter()
                            .filter(|d| d.relation_type == RELATION_REQUIRED && d.mod_id != 0)
                            .map(|d| d.mod_id)
                            .collect();
                        stack.push(Frame::Exit(id, Box::new(file)));
                        for d in deps.into_iter().rev() {
                            stack.push(Frame::Enter(d));
                        }
                    }
                }
            }
            Frame::Exit(id, file) => plan.steps.push(Step { mod_id: id, file: *file }),
        }
    }
    Ok(plan)
}

// ---------- Download ----------

/// Streams `url` into `dest` and reports `(received, total)` as it goes.
pub async fn download(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    progress: impl FnMut(u64, Option<u64>),
) -> AppResult<()> {
    let r = download_inner(http, url, dest, progress).await;
    if r.is_err() {
        // Don't leave a partial file behind.
        let _ = tokio::fs::remove_file(dest).await;
    }
    r
}

async fn download_inner(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> AppResult<()> {
    if let Some(dir) = dest.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let resp = http.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!("download returned HTTP {}", resp.status())));
    }
    let total = resp.content_length();
    // Buffer writes: network chunks are often only a few KB.
    let mut out = tokio::io::BufWriter::with_capacity(256 * 1024, tokio::fs::File::create(dest).await?);
    let mut received = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        received += chunk.len() as u64;
        out.write_all(&chunk).await?;
        progress(received, total);
    }
    out.flush().await?;
    Ok(())
}

// ---------- Extraction ----------

/// A folder name that is a single plain path component.
pub fn is_safe_folder_name(name: &str) -> bool {
    let p = Path::new(name);
    !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && matches!(p.components().collect::<Vec<_>>().as_slice(), [Component::Normal(_)])
}

/// Unpacks `zip_path` into `staging`, which is emptied first, and returns the
/// top-level addon folders. Entries that would escape `staging` fail the whole
/// archive. If the zip holds loose files with a `.toc` at its root, they are
/// wrapped in a folder called `fallback_name`.
pub fn extract_zip(zip_path: &Path, staging: &Path, fallback_name: &str) -> AppResult<Vec<String>> {
    if staging.exists() {
        std::fs::remove_dir_all(staging)?;
    }
    std::fs::create_dir_all(staging)?;

    let mut archive = zip::ZipArchive::new(BufReader::new(std::fs::File::open(zip_path)?))?;
    let mut written = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(AppError::BadArchive(format!("unsafe path {:?}", entry.name())));
        };
        let first = rel.components().next();
        if matches!(first, Some(Component::Normal(n)) if n == "__MACOSX") {
            continue;
        }
        let out = staging.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut w = std::fs::File::create(&out)?;
        let budget = MAX_UNPACKED_BYTES.saturating_sub(written);
        let n = std::io::copy(&mut (&mut entry).take(budget + 1), &mut w)?;
        written += n;
        if written > MAX_UNPACKED_BYTES {
            return Err(AppError::BadArchive("archive is too large".into()));
        }
        w.flush()?;
    }

    let mut dirs = Vec::new();
    let mut loose = Vec::new();
    for e in std::fs::read_dir(staging)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        if e.file_type()?.is_dir() {
            dirs.push(name);
        } else {
            loose.push(name);
        }
    }

    let has_loose_toc = loose.iter().any(|n| n.to_ascii_lowercase().ends_with(".toc"));
    if dirs.is_empty() && has_loose_toc && is_safe_folder_name(fallback_name) {
        let wrap = staging.join(fallback_name);
        std::fs::create_dir_all(&wrap)?;
        for n in &loose {
            std::fs::rename(staging.join(n), wrap.join(n))?;
        }
        dirs.push(fallback_name.to_string());
    }
    dirs.retain(|d| is_safe_folder_name(d));
    if dirs.is_empty() {
        return Err(AppError::BadArchive("no addon folders found".into()));
    }
    dirs.sort();
    Ok(dirs)
}

// ---------- Moving folders into AddOns ----------

fn backup_path(addons: &Path, name: &str) -> PathBuf {
    addons.join(format!(".ezaddon-bak-{name}"))
}

/// Moves each folder from `staging` into `addons`, replacing what is there.
/// Old copies are kept as backups until every move has worked. If any move
/// fails, everything is put back as it was.
pub fn swap_in(addons: &Path, staging: &Path, folders: &[String]) -> AppResult<()> {
    std::fs::create_dir_all(addons)?;
    let mut done: Vec<(&str, Option<PathBuf>)> = Vec::new();

    let rollback = |done: &[(&str, Option<PathBuf>)]| {
        for (name, backup) in done.iter().rev() {
            let dest = addons.join(name);
            let _ = std::fs::remove_dir_all(&dest);
            if let Some(b) = backup {
                let _ = std::fs::rename(b, &dest);
            }
        }
    };

    for name in folders {
        if !is_safe_folder_name(name) {
            rollback(&done);
            return Err(AppError::BadArchive(format!("bad folder name {name:?}")));
        }
        let dest = addons.join(name);
        let backup = if dest.exists() {
            let b = backup_path(addons, name);
            if b.exists() {
                let _ = std::fs::remove_dir_all(&b);
            }
            if let Err(e) = std::fs::rename(&dest, &b) {
                rollback(&done);
                return Err(AppError::Io(format!(
                    "could not replace {name} (is the game or another program using it?): {e}"
                )));
            }
            Some(b)
        } else {
            None
        };
        if let Err(e) = std::fs::rename(staging.join(name), &dest) {
            if let Some(b) = &backup {
                let _ = std::fs::rename(b, &dest);
            }
            rollback(&done);
            return Err(AppError::Io(format!("could not install {name}: {e}")));
        }
        done.push((name, backup));
    }

    for (_, backup) in done {
        if let Some(b) = backup {
            let _ = std::fs::remove_dir_all(b);
        }
    }
    Ok(())
}

/// Deletes addon folders. Names that aren't a single plain folder are skipped.
pub fn remove_folders(addons: &Path, folders: &[String]) -> AppResult<()> {
    for f in folders.iter().filter(|f| is_safe_folder_name(f)) {
        let p = addons.join(f);
        if p.is_dir() {
            std::fs::remove_dir_all(&p)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curseforge::Dependency;
    use std::collections::HashMap;

    struct Fixture(HashMap<u32, Vec<u32>>);

    impl FileSource for Fixture {
        async fn best_file(&self, mod_id: u32) -> AppResult<Option<File>> {
            Ok(self.0.get(&mod_id).map(|deps| File {
                id: mod_id * 10,
                mod_id,
                dependencies: deps
                    .iter()
                    .map(|d| Dependency { mod_id: *d, relation_type: RELATION_REQUIRED })
                    .chain(std::iter::once(Dependency { mod_id: 777, relation_type: 2 }))
                    .collect(),
                ..Default::default()
            }))
        }
    }

    fn ids(p: &Plan) -> Vec<u32> {
        p.steps.iter().map(|s| s.mod_id).collect()
    }

    #[tokio::test]
    async fn plans_dependencies_first_and_handles_cycles() {
        // 1 -> [2, 3], 2 -> [4], 4 -> [1] (cycle), 3 -> [], 5 has no file
        let f = Fixture(HashMap::from([
            (1, vec![2, 3, 5]),
            (2, vec![4]),
            (3, vec![]),
            (4, vec![1]),
        ]));
        let plan = plan_install(&f, 1, &HashSet::new()).await.unwrap();
        assert_eq!(ids(&plan), vec![4, 2, 3, 1]);
        assert_eq!(plan.missing_dependencies, vec![5]);
    }

    #[tokio::test]
    async fn skips_installed_dependencies_but_not_root() {
        let f = Fixture(HashMap::from([(1, vec![2]), (2, vec![])]));
        let plan = plan_install(&f, 1, &HashSet::from([1, 2])).await.unwrap();
        assert_eq!(ids(&plan), vec![1]);
    }

    #[tokio::test]
    async fn root_without_file_is_an_error() {
        let f = Fixture(HashMap::new());
        assert!(matches!(
            plan_install(&f, 1, &HashSet::new()).await,
            Err(AppError::NoForeverFile)
        ));
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            if name.ends_with('/') {
                z.add_directory(name.trim_end_matches('/'), opts).unwrap();
            } else {
                z.start_file(*name, opts).unwrap();
                z.write_all(data).unwrap();
            }
        }
        z.finish().unwrap();
    }

    #[test]
    fn extracts_top_level_folders() {
        let d = tempfile::tempdir().unwrap();
        let zip = d.path().join("a.zip");
        make_zip(
            &zip,
            &[
                ("Questie/", b""),
                ("Questie/Questie.toc", b"## Interface: 16001"),
                ("Questie/Modules/x.lua", b"print(1)"),
                ("QuestieLib/QuestieLib.toc", b""),
                ("__MACOSX/._junk", b""),
            ],
        );
        let staging = d.path().join("stage");
        let dirs = extract_zip(&zip, &staging, "Questie").unwrap();
        assert_eq!(dirs, vec!["Questie", "QuestieLib"]);
        assert!(staging.join("Questie/Modules/x.lua").is_file());
        assert!(!staging.join("__MACOSX").exists());
    }

    #[test]
    fn rejects_path_traversal() {
        let d = tempfile::tempdir().unwrap();
        let zip = d.path().join("evil.zip");
        make_zip(&zip, &[("Good/Good.toc", b""), ("../../evil.txt", b"x")]);
        let staging = d.path().join("inner/stage");
        let err = extract_zip(&zip, &staging, "Good").unwrap_err();
        assert!(matches!(err, AppError::BadArchive(_)));
        assert!(!d.path().join("evil.txt").exists());
    }

    #[test]
    fn wraps_loose_toc_in_folder() {
        let d = tempfile::tempdir().unwrap();
        let zip = d.path().join("loose.zip");
        make_zip(&zip, &[("Tiny.toc", b""), ("Tiny.lua", b"")]);
        let staging = d.path().join("stage");
        let dirs = extract_zip(&zip, &staging, "Tiny").unwrap();
        assert_eq!(dirs, vec!["Tiny"]);
        assert!(staging.join("Tiny/Tiny.toc").is_file());
    }

    #[test]
    fn swap_replaces_and_rolls_back() {
        let d = tempfile::tempdir().unwrap();
        let addons = d.path().join("AddOns");
        let staging = d.path().join("stage");
        std::fs::create_dir_all(addons.join("A")).unwrap();
        std::fs::write(addons.join("A/old.txt"), "old").unwrap();
        std::fs::create_dir_all(staging.join("A")).unwrap();
        std::fs::write(staging.join("A/new.txt"), "new").unwrap();

        // "B" is missing from staging, so the second move fails and A is restored.
        let err = swap_in(&addons, &staging, &["A".into(), "B".into()]);
        assert!(err.is_err());
        assert!(addons.join("A/old.txt").is_file());
        assert!(!addons.join(".ezaddon-bak-A").exists());

        std::fs::create_dir_all(staging.join("A")).unwrap();
        std::fs::write(staging.join("A/new.txt"), "new").unwrap();
        swap_in(&addons, &staging, &["A".into()]).unwrap();
        assert!(addons.join("A/new.txt").is_file());
        assert!(!addons.join("A/old.txt").exists());
        assert!(!addons.join(".ezaddon-bak-A").exists());
    }

    #[test]
    fn folder_name_safety() {
        assert!(is_safe_folder_name("Questie"));
        assert!(is_safe_folder_name("Details_Streamer"));
        for bad in ["", ".", "..", "a/b", "a\\b", "C:", "../x"] {
            assert!(!is_safe_folder_name(bad), "{bad}");
        }
    }

    #[test]
    fn remove_folders_skips_unsafe_names() {
        let d = tempfile::tempdir().unwrap();
        let addons = d.path().join("AddOns");
        std::fs::create_dir_all(addons.join("X")).unwrap();
        std::fs::create_dir_all(d.path().join("Keep")).unwrap();
        remove_folders(&addons, &["X".into(), "../Keep".into()]).unwrap();
        assert!(!addons.join("X").exists());
        assert!(d.path().join("Keep").exists());
    }
}

/// Live install against the real API and CDN, run by hand:
/// `cargo test live_install -- --ignored --nocapture`
#[cfg(test)]
mod live {
    use super::*;
    use crate::curseforge::{pick_best_file, CurseForge};

    #[tokio::test]
    #[ignore]
    async fn live_install() {
        let cf = CurseForge::new(CurseForge::http_client(), "proxy-injected".into());
        // Simulationcraft allows third-party downloads; Bagnon doesn't.
        let files = cf.files(82745, 88568).await.unwrap();
        let file = pick_best_file(&files, 88568, 1).expect("a Forever file").clone();
        let url = match file.download_url.clone() {
            Some(u) => u,
            None => cf.download_url(82745, file.id).await.unwrap().expect("downloadable"),
        };
        assert_eq!(cf.download_url(1592, 9546609).await.unwrap(), None, "Bagnon should be website-only");

        let dir = tempfile::tempdir().unwrap();
        let addons = dir.path().join("_forever_/Interface/AddOns");
        let zip = dir.path().join("dl.zip");
        let mut last = (0, None);
        download(cf.http(), &url, &zip, |r, t| last = (r, t)).await.unwrap();
        println!("downloaded {} of {:?} bytes from {url}", last.0, last.1);
        let staging = addons.join(".ezaddon-staging-82745");
        let folders = extract_zip(&zip, &staging, "Simulationcraft").unwrap();
        swap_in(&addons, &staging, &folders).unwrap();
        let _ = std::fs::remove_dir_all(&staging);
        println!("installed folders {folders:?}");
        assert!(addons.join("Simulationcraft").join("Simulationcraft.toc").is_file());
        // The installed folder fingerprints to what CurseForge lists for the file.
        let fp = crate::scanner::folder_fingerprint(&addons.join("Simulationcraft")).unwrap();
        assert!(file.modules.iter().any(|m| m.fingerprint == u64::from(fp)), "fingerprint {fp} not in {:?}", file.modules);
    }
}
