use super::{Progress, github::error, models::*};
use crate::foundation::{AppResult, GameClientSelection};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_EXPANDED_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}

pub(super) fn no_links(path: &Path) -> AppResult<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(error("clientUnsafePath"));
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return Err(error("clientUnsafePath"));
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub(super) fn member_path(name: &str) -> AppResult<PathBuf> {
    let normalized = name.replace('\\', "/");
    let trimmed = normalized.strip_suffix('/').unwrap_or(&normalized);
    let mut result = PathBuf::new();
    if trimmed.is_empty() || trimmed.len() > 1024 {
        return Err(error("clientUnsafeArchive"));
    }
    for part in trimmed.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
            || ["COM¹", "COM²", "COM³", "LPT¹", "LPT²", "LPT³"].contains(&stem.as_str());
        if part.is_empty()
            || matches!(part, "." | "..")
            || reserved
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c))
        {
            return Err(error("clientUnsafeArchive"));
        }
        result.push(part);
    }
    Ok(result)
}

struct OwnedDirectory {
    path: PathBuf,
    parent: PathBuf,
    keep: bool,
}

impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        // Only the new UUID directory allocated by this operation is ever removed.
        if !self.keep
            && self.path.parent() == Some(self.parent.as_path())
            && no_links(&self.path).is_ok()
        {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(super) fn install<R: Read>(
    mut reader: R,
    release: &ClientRelease,
    asset: &ReleaseAsset,
    root: &str,
    progress: &Progress,
    persist: impl FnOnce(&ClientInstallation) -> AppResult<()>,
) -> AppResult<ClientInstallation> {
    if !asset.installable() || asset.size == 0 || asset.size > MAX_ASSET_BYTES {
        return Err(error("clientPackageFormat"));
    }
    let root = Path::new(root.trim());
    if !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(error("clientUnsafePath"));
    }
    #[cfg(windows)]
    if !matches!(root.components().next(), Some(std::path::Component::Prefix(prefix))
        if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
    {
        return Err(error("clientUnsafePath"));
    }
    no_links(root)?;
    let parent = root.join("PlatverseOutput");
    no_links(&parent)?;
    fs::create_dir_all(&parent)?;
    let parent = parent.canonicalize()?;
    let id = uuid::Uuid::new_v4().to_string();
    let staging = parent.join(format!(".install-{id}"));
    fs::create_dir(&staging)?;
    let mut owned = OwnedDirectory {
        path: staging.clone(),
        parent: parent.clone(),
        keep: false,
    };
    let zip_path = staging.join("download.zip");
    let content = staging.join("content");
    fs::create_dir(&content)?;
    progress.stage("downloading", asset.size);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&zip_path)?;
    let mut hash = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        progress.check()?;
        let n = reader
            .read(&mut buffer)
            .map_err(|_| error("clientNetwork"))?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > asset.size {
            return Err(error("clientIntegrity"));
        }
        hash.update(&buffer[..n]);
        output.write_all(&buffer[..n])?;
        progress.advance(n as u64);
    }
    output.sync_all()?;
    drop(output);
    let sha256 = format!("{:x}", hash.finalize());
    if count != asset.size {
        return Err(error("clientIntegrity"));
    }
    if let Some(digest) = &asset.digest
        && !digest.to_ascii_lowercase().eq(&format!("sha256:{sha256}"))
    {
        return Err(error("clientIntegrity"));
    }
    let candidates = extract(&zip_path, &content, progress)?;
    progress.check()?;
    let final_path = parent.join(format!("{}-{}-{id}", release.id, asset.id));
    let record = ClientInstallation {
        id,
        release_id: release.id,
        tag_name: release.tag_name.clone(),
        prerelease: release.prerelease,
        published_at: release.published_at.clone(),
        asset_id: asset.id,
        asset_name: asset.name.clone(),
        asset_updated_at: asset.updated_at.clone(),
        sha256,
        digest_verified: asset.digest.is_some(),
        directory: display_path(&final_path),
        storage_root: display_path(parent.parent().ok_or_else(|| error("clientUnsafePath"))?),
        executable_candidates: candidates,
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    let manifest = serde_json::to_vec_pretty(&record)?;
    let mut manifest_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(content.join(".abya-client.json"))?;
    manifest_file.write_all(&manifest)?;
    manifest_file.sync_all()?;
    drop(manifest_file);
    fs::rename(&content, &final_path)?;
    // Persistence failure rolls back only this fresh completed directory.
    let mut committed = OwnedDirectory {
        path: final_path,
        parent,
        keep: false,
    };
    persist(&record)?;
    committed.keep = true;
    // Remove the temporary downloaded ZIP, retaining the new version directory.
    if fs::remove_file(&zip_path).is_ok() && fs::remove_dir(&staging).is_ok() {
        owned.keep = true;
    }
    Ok(record)
}

fn extract(zip_path: &Path, destination: &Path, progress: &Progress) -> AppResult<Vec<String>> {
    let mut archive =
        zip::ZipArchive::new(File::open(zip_path)?).map_err(|_| error("clientUnsafeArchive"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(error("clientArchiveLimit"));
    }
    let mut total = 0_u64;
    let mut names = HashSet::new();
    let mut paths = Vec::new();
    for i in 0..archive.len() {
        progress.check()?;
        let entry = archive
            .by_index(i)
            .map_err(|_| error("clientUnsafeArchive"))?;
        let path = member_path(entry.name())?;
        if path == Path::new(".abya-client.json") {
            return Err(error("clientUnsafeArchive"));
        }
        if !names.insert(path.to_string_lossy().to_lowercase()) {
            return Err(error("clientUnsafeArchive"));
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if !matches!(kind, 0 | 0o100000 | 0o040000) {
                return Err(error("clientUnsafeArchive"));
            }
        }
        total = total
            .checked_add(entry.size())
            .filter(|value| *value <= MAX_EXPANDED_BYTES)
            .ok_or_else(|| error("clientArchiveLimit"))?;
        paths.push(path);
    }
    progress.stage("extracting", total);
    let mut executables = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        progress.check()?;
        let mut entry = archive
            .by_index(i)
            .map_err(|_| error("clientUnsafeArchive"))?;
        let target = destination.join(path);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        let parent = target
            .parent()
            .ok_or_else(|| error("clientUnsafeArchive"))?;
        fs::create_dir_all(parent)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let expected = entry.size();
        let mut written = 0_u64;
        let mut buffer = [0_u8; 128 * 1024];
        loop {
            progress.check()?;
            let n = entry
                .read(&mut buffer)
                .map_err(|_| error("clientUnsafeArchive"))?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > expected {
                return Err(error("clientArchiveLimit"));
            }
            output.write_all(&buffer[..n])?;
            progress.advance(n as u64);
        }
        if written != expected {
            return Err(error("clientIntegrity"));
        }
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        {
            executables.push(path.clone());
        }
    }
    let mut candidates = Vec::new();
    for path in executables {
        if unity_executable(&destination.join(&path))? {
            candidates.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
    candidates.sort();
    if candidates.is_empty() {
        return Err(error("clientNoExecutable"));
    }
    Ok(candidates)
}

fn unity_executable(path: &Path) -> AppResult<bool> {
    no_links(path)?;
    if !path.is_file() {
        return Ok(false);
    }
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let Some(stem) = path.file_stem() else {
        return Ok(false);
    };
    let data = parent.join(format!("{}_Data", stem.to_string_lossy()));
    let player = parent.join("UnityPlayer.dll");
    no_links(&data)?;
    no_links(&player)?;
    if !data.is_dir() || !player.is_file() {
        return Ok(false);
    }
    let mut magic = [0; 2];
    Ok(File::open(path)?.read_exact(&mut magic).is_ok() && magic == *b"MZ")
}

pub(super) fn resolve(
    record: &ClientInstallation,
    selection: &GameClientSelection,
) -> AppResult<String> {
    if !record
        .executable_candidates
        .contains(&selection.executable_relative_path)
    {
        return Err(error("clientChooseExecutable"));
    }
    let root = Path::new(&record.directory);
    no_links(root)?;
    let root = root
        .canonicalize()
        .map_err(|_| error("clientInstallationMissing"))?;
    let marker = root.join(".abya-client.json");
    no_links(&marker)?;
    let file = File::open(marker).map_err(|_| error("clientInstallationMissing"))?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024).read_to_end(&mut bytes)?;
    let manifest: ClientInstallation =
        serde_json::from_slice(&bytes).map_err(|_| error("clientInstallationMissing"))?;
    if manifest.id != record.id || manifest.sha256 != record.sha256 {
        return Err(error("clientInstallationMissing"));
    }
    let path = root.join(member_path(&selection.executable_relative_path)?);
    no_links(&path)?;
    let path = path
        .canonicalize()
        .map_err(|_| error("clientInstallationMissing"))?;
    if !path.starts_with(&root) || !unity_executable(&path)? {
        return Err(error("clientInstallationMissing"));
    }
    // Unity launches and working directories use normal drive paths, as local Players do.
    Ok(display_path(&path))
}
