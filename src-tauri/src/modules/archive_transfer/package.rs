use crate::foundation::{AppError, AppResult};
use crate::modules::game_archives::ArchiveSnapshot;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

pub(crate) struct ArchivePackage {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}

pub(crate) fn build_package(
    snapshot: ArchiveSnapshot,
    final_path: PathBuf,
) -> AppResult<ArchivePackage> {
    let part_path = final_path.with_extension("zip.part");
    let _ = std::fs::remove_file(&part_path);
    let output = File::create(&part_path)?;
    let mut zip = zip::ZipWriter::new(BufWriter::new(output));
    let file_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let directory_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o755);
    let mut buffer = vec![0_u8; 128 * 1024];
    for entry in snapshot.entries {
        if entry.is_directory {
            zip.add_directory(format!("{}/", entry.relative_path), directory_options)
                .map_err(AppError::internal)?;
            continue;
        }
        zip.start_file(entry.relative_path, file_options)
            .map_err(AppError::internal)?;
        let mut input = BufReader::new(File::open(entry.absolute_path)?);
        loop {
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            zip.write_all(&buffer[..read])?;
        }
    }
    let mut writer = zip.finish().map_err(AppError::internal)?;
    writer.flush()?;
    drop(writer);
    let _ = std::fs::remove_file(&final_path);
    std::fs::rename(&part_path, &final_path)?;
    let bytes = std::fs::metadata(&final_path)?.len();
    let sha256 = sha256_file(&final_path)?;
    Ok(ArchivePackage {
        path: final_path,
        bytes,
        sha256,
    })
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut input = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 128 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::game_archives::{ArchiveEntry, ArchiveSnapshot, TransferableArchive};
    use std::io::Read;
    use uuid::Uuid;

    #[test]
    fn packages_the_complete_archive_folder_and_hashes_the_zip() {
        let root = std::env::temp_dir().join(format!("abya-package-{}", Uuid::new_v4()));
        let archive_root = root.join("archive");
        let nested = archive_root.join("Resources");
        std::fs::create_dir_all(&nested).unwrap();
        let main = archive_root.join("Main.PBArc");
        let asset = nested.join("asset.bin");
        std::fs::write(&main, br#"{"Guid":"archive-1","Name":"Demo"}"#).unwrap();
        std::fs::write(&asset, [1_u8, 2, 3, 4]).unwrap();
        let output = root.join("transfer.zip");
        let package = build_package(
            ArchiveSnapshot {
                archive: TransferableArchive {
                    main_archive_path: main.to_string_lossy().into_owned(),
                    archive_path: archive_root.to_string_lossy().into_owned(),
                    archive_guid: "archive-1".into(),
                    archive_name: "Demo".into(),
                    author: String::new(),
                    file_count: 2,
                    uncompressed_bytes: 37,
                    last_modified_at: String::new(),
                },
                entries: vec![
                    ArchiveEntry {
                        absolute_path: main,
                        relative_path: "Main.PBArc".into(),
                        is_directory: false,
                    },
                    ArchiveEntry {
                        absolute_path: nested,
                        relative_path: "Resources".into(),
                        is_directory: true,
                    },
                    ArchiveEntry {
                        absolute_path: asset,
                        relative_path: "Resources/asset.bin".into(),
                        is_directory: false,
                    },
                ],
            },
            output.clone(),
        )
        .unwrap();

        assert_eq!(package.bytes, std::fs::metadata(&output).unwrap().len());
        assert_eq!(package.sha256, sha256_file(&output).unwrap());
        let file = File::open(&output).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        assert!(zip.by_name("Main.PBArc").is_ok());
        let mut contents = Vec::new();
        zip.by_name("Resources/asset.bin")
            .unwrap()
            .read_to_end(&mut contents)
            .unwrap();
        assert_eq!(contents, [1, 2, 3, 4]);
        let _ = std::fs::remove_dir_all(root);
    }
}
