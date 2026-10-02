use crate::foundation::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub(super) fn safe_path(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let value = Path::new(relative);
    if relative.is_empty()
        || relative.contains(':')
        || value
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(AppError::validation(
            "Only a relative path inside this task is allowed.",
        ));
    }
    let root = root.canonicalize()?;
    let mut current = root.clone();
    for component in value.components() {
        current.push(component);
        if let Ok(meta) = std::fs::symlink_metadata(&current) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err(AppError::validation(
                        "Linked artifact paths are not allowed.",
                    ));
                }
            }
            if meta.file_type().is_symlink() || !current.canonicalize()?.starts_with(&root) {
                return Err(AppError::validation(
                    "Artifact path leaves the task workspace.",
                ));
            }
        }
    }
    Ok(current)
}

pub(super) fn hash_file(path: &Path) -> AppResult<(String, u64)> {
    let mut file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok((format!("{:x}", hash.finalize()), length))
}

pub(super) fn text_file(root: &Path, relative: &str) -> AppResult<(String, String)> {
    let path = safe_path(root, relative)?;
    if std::fs::metadata(&path)?.len() > 1024 * 1024 {
        return Err(AppError::validation("Document exceeds 1 MiB."));
    }
    let content = std::fs::read_to_string(&path)?;
    if content.trim().is_empty() {
        return Err(AppError::validation("Document is empty."));
    }
    Ok((content, hash_file(&path)?.0))
}

pub(super) fn write_file(root: &Path, relative: &str, bytes: &[u8]) -> AppResult<()> {
    let path = safe_path(root, relative)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, bytes)?;
    // rename replaces atomically on Unix; MoveFileExW preserves the old file on Windows failure.
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let source: Vec<_> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let dest: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                dest.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(not(windows))]
    std::fs::rename(temporary, path)?;
    Ok(())
}
