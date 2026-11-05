use super::*;
use sha2::{Digest, Sha256};
static INSTALL_GATE: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

fn fingerprint(snapshot: &ArchiveSnapshot) -> AppResult<String> {
    let mut entries: Vec<_> = snapshot
        .entries
        .iter()
        .filter(|e| !e.is_directory)
        .collect();
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut hash = Sha256::new();
    for entry in entries {
        hash.update(entry.relative_path.as_bytes());
        hash.update([0]);
        let mut file = std::fs::File::open(&entry.absolute_path)?;
        std::io::copy(&mut file, &mut hash)?;
    }
    Ok(format!("{:x}", hash.finalize()))
}

impl ArchiveCatalogService {
    /// 只接受上层验证的冻结版本；完整复制、同 GUID 冲突拒绝、永不覆盖。
    pub fn export_snapshot(&self, source: &Path, task: &str, version: &str) -> AppResult<Value> {
        let _guard = INSTALL_GATE.lock();
        uuid::Uuid::parse_str(task).map_err(AppError::internal)?;
        let source = source.canonicalize()?;
        let before = self.snapshot(&source.join("Main.PBArc"))?;
        let hash = fingerprint(&before)?;
        let root = &self.paths.default_archive_root;
        std::fs::create_dir_all(root)?;
        let target = root.join(
            source
                .file_name()
                .ok_or_else(|| AppError::validation("存档名称无效。"))?,
        );
        for existing in self.discover(None)? {
            if existing.archive_guid == before.archive.archive_guid
                || Path::new(&existing.archive_path) == target
            {
                let existing = PathBuf::from(existing.archive_path);
                let snapshot = self.snapshot(&existing.join("Main.PBArc"))?;
                if existing.canonicalize()? == target.canonicalize().unwrap_or(target.clone())
                    && fingerprint(&snapshot)? == hash
                {
                    return self.export_receipt(
                        task,
                        version,
                        &source,
                        &existing,
                        &hash,
                        "already-present",
                    );
                }
                return Err(AppError::validation(
                    "全局目录已有同 GUID 或同名的不同存档，未覆盖；请先处理冲突。",
                ));
            }
        }
        if target.exists() {
            return Err(AppError::validation("导出目标已存在，未覆盖。"));
        }
        let staging = root
            .parent()
            .ok_or_else(|| AppError::validation("全局目录无父目录。"))?
            .join(".abya-export-staging")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&staging)?;
        for entry in &before.entries {
            let destination = staging.join(&entry.relative_path);
            if entry.is_directory {
                std::fs::create_dir_all(destination)?;
            } else {
                std::fs::create_dir_all(destination.parent().unwrap())?;
                std::fs::copy(&entry.absolute_path, destination)?;
            }
        }
        if fingerprint(&self.snapshot(&source.join("Main.PBArc"))?)? != hash
            || fingerprint(&self.snapshot(&staging.join("Main.PBArc"))?)? != hash
        {
            return Err(AppError::validation("导出期间内容变化，暂存副本未安装。"));
        }
        std::fs::rename(&staging, &target)?;
        if fingerprint(&self.snapshot(&target.join("Main.PBArc"))?)? != hash {
            return Err(AppError::validation(
                "导出读回不一致，请检查目标；未报告成功。",
            ));
        }
        self.export_receipt(task, version, &source, &target, &hash, "installed")
    }

    fn export_receipt(
        &self,
        task: &str,
        version: &str,
        source: &Path,
        target: &Path,
        hash: &str,
        outcome: &str,
    ) -> AppResult<Value> {
        let result = serde_json::json!({"taskId":task,"version":version,"source":source,"destination":target,"sha256":hash,"outcome":outcome,"at":Utc::now().to_rfc3339()});
        let receipts = self.paths.reports_dir.join("ArchiveExports");
        std::fs::create_dir_all(&receipts)?;
        std::fs::write(
            receipts.join(format!("{}.json", uuid::Uuid::new_v4())),
            serde_json::to_vec_pretty(&result)?,
        )?;
        Ok(result)
    }
}
