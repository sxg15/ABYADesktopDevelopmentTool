use super::*;
use std::io::Write;
fn report_hash(r: &ProductionRecord) -> String {
    use sha2::{Digest, Sha256};
    let mut checks = BTreeMap::new();
    for t in r
        .self_tests
        .iter()
        .filter(|t| t["version"].as_str() == r.current_version.as_deref() && t["cycle"] == r.cycle)
    {
        if let Some(id) = t["id"].as_str() {
            let mut v = t.clone();
            if let Some(o) = v.as_object_mut() {
                o.remove("recordedAt");
                o.remove("runId");
            }
            checks.insert(id, v);
        }
    }
    let mut issues = r.issues.clone();
    for issue in &mut issues {
        if let Some(v) = issue.as_object_mut() {
            v.remove("updatedAt");
        }
        if let Some(v) = issue["executionHandling"].as_object_mut() {
            v.remove("updatedAt");
        }
    }
    format!(
        "{:x}",
        Sha256::digest(json!([r.current_version, r.cycle, checks, issues]).to_string())
    )
}

pub(super) fn core_ready(r: &ProductionRecord, root: &Path) -> AppResult<()> {
    if r.version_details["purpose"] == "fixture" {
        return Err(AppError::validation("测试夹具不能作为可试玩候选。"));
    }
    let version = validation::version(r)?;
    versions::verify(&r.version_details)?;
    for id in ["lua", "input", "core-loop", "save-reload"] {
        let selected = if id == "input"
            && r.self_tests.iter().any(|t| {
                t["id"] == "playable-input" && t["version"] == version && t["cycle"] == r.cycle
            }) {
            "playable-input"
        } else {
            id
        };
        let t = r
            .self_tests
            .iter()
            .rev()
            .find(|t| t["id"] == selected && t["version"] == version && t["cycle"] == r.cycle)
            .ok_or_else(|| AppError::validation(format!("可玩包缺少 {id} 实测。")))?;
        if t["status"] != "passed" {
            return Err(AppError::validation(format!(
                "{id} 尚未通过，不能标记可试玩。"
            )));
        }
        validation::evidence_refs(r, root, &t["evidenceIds"], version)?;
    }
    Ok(())
}
pub(super) fn valid(r: &ProductionRecord, root: &Path) -> bool {
    let p = &r.workbench.execution.verification_package;
    p["version"].as_str() == r.current_version.as_deref()
        && p["reportHash"] == report_hash(r)
        && p["scopeHash"] == autonomy::scope(r)
        && p["cycle"] == r.cycle
        && p["archiveHash"] == r.version_details["archiveHash"]
        && p["playerBuildHash"] == r.version_details["playerBuildHash"]
        && p["path"].as_str().is_some_and(|s| {
            files::safe_path(root, s)
                .and_then(|p| files::hash_file(&p))
                .is_ok_and(|(h, _)| p["sha256"] == h)
        })
        && core_ready(r, root).is_ok()
}
impl TaskService {
    /// Package I/O happens before CAS and outside the database mutex. A conflict never installs stale metadata.
    pub fn production_build_package(&self, task: &str, expected: u64) -> AppResult<ProductionView> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if r.revision != expected {
            return Err(conflict());
        }
        let root = self.production_workspace(task)?;
        validation::approved(&r, &root, "requirements")?;
        validation::approved(&r, &root, "plan")?;
        core_ready(&r, &root)?;
        if valid(&r, &root) {
            return self.production_get(task);
        }
        let archive = versions::archive_root(Path::new(mutation::text(
            &r.version_details,
            "archiveSource",
        )?))?;
        let inventory = versions::inventory(std::slice::from_ref(&archive))?;
        if inventory.len() > 20000
            || inventory.iter().map(|(_, _, n, _)| n).sum::<u64>() > 1024 * 1024 * 1024
        {
            return Err(AppError::validation("可玩包超过文件或容量上限。"));
        }
        let relative = format!("artifacts/playable-packages/{}.zip", uuid::Uuid::new_v4());
        let path = files::safe_path(&root, &relative)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        let file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut manifest = vec![];
        for (_, file, _, _) in inventory {
            let name = file
                .strip_prefix(&archive)
                .map_err(AppError::internal)?
                .to_string_lossy()
                .replace('\\', "/");
            let (hash, bytes) = files::hash_file(&file)?;
            zip.start_file(format!("archive/{name}"), options)
                .map_err(AppError::internal)?;
            std::io::copy(&mut std::fs::File::open(&file)?, &mut zip)?;
            if files::hash_file(&file)?.0 != hash {
                return Err(AppError::validation("打包期间存档变化，未登记该包。"));
            }
            manifest.push(json!({"path":format!("archive/{name}"),"sha256":hash,"bytes":bytes}));
        }
        let tests: Vec<_> = r
            .self_tests
            .iter()
            .filter(|t| {
                t["version"].as_str() == r.current_version.as_deref() && t["cycle"] == r.cycle
            })
            .collect();
        let mut evidence_manifest = vec![];
        let mut total = 0u64;
        let ids: std::collections::BTreeSet<_> = tests
            .iter()
            .flat_map(|t| t["evidenceIds"].as_array().into_iter().flatten())
            .filter_map(|v| v.as_str())
            .collect();
        for e in r.evidence.iter().filter(|e| ids.contains(e.id.as_str())) {
            let source = files::safe_path(&root, &e.path)?;
            let (hash, bytes) = files::hash_file(&source)?;
            total += bytes;
            if total > 512 * 1024 * 1024 {
                return Err(AppError::validation(
                    "验证证据超过512 MiB，请提供当前检查的精简原始证据。",
                ));
            }
            if hash != e.sha256 {
                return Err(AppError::validation("证据已改变，未登记该包。"));
            }
            let entry = format!("evidence/{}", e.path.replace('\\', "/"));
            zip.start_file(&entry, options)
                .map_err(AppError::internal)?;
            std::io::copy(&mut std::fs::File::open(source)?, &mut zip)?;
            evidence_manifest.push(json!({"id":e.id,"path":entry,"sha256":hash,"bytes":bytes}));
        }
        let report = json!({"schema":"abya.playable-package/v1","taskId":task,"version":r.current_version,"scopeHash":autonomy::scope(&r),"cycle":r.cycle,"evidence":evidence_manifest,
            "archiveGuid":r.version_details["archiveGuid"],"levelGuid":r.version_details["levelGuid"],"playerBuildHash":r.version_details["playerBuildHash"],
            "verification":"core-verified","humanAccepted":false,"allRequiredChecksPassed":iteration::ready(&r,&root).is_ok(),"checks":tests,"issues":r.issues,"files":manifest});
        zip.start_file("verification.json", options)
            .map_err(AppError::internal)?;
        zip.write_all(&serde_json::to_vec_pretty(&report)?)?;
        zip.start_file("试玩说明.txt", options)
            .map_err(AppError::internal)?;
        zip.write_all("这是核心实测通过的候选存档包，不代表最终人工验收。用匹配 Player 导入 archive 目录的 Main.PBArc；具体构建指纹、关卡、验证结果及未完成项见 verification.json。APP 内使用当前版本的试玩入口启动。请保留原存档，导入为独立副本。\n".as_bytes())?;
        zip.finish().map_err(AppError::internal)?.sync_all()?;
        // Read back package bytes, not just the source file list, before registering the result.
        {
            use sha2::{Digest, Sha256};
            use std::io::Read;
            let mut saved =
                zip::ZipArchive::new(std::fs::File::open(&path)?).map_err(AppError::internal)?;
            for item in report["files"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(report["evidence"].as_array().into_iter().flatten())
            {
                let mut entry = saved
                    .by_name(item["path"].as_str().ok_or_else(conflict)?)
                    .map_err(AppError::internal)?;
                let mut hash = Sha256::new();
                let mut buffer = [0u8; 65536];
                loop {
                    let count = entry.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    hash.update(&buffer[..count]);
                }
                if item["sha256"] != format!("{:x}", hash.finalize()) {
                    return Err(AppError::validation("包内文件读回校验失败，未登记。"));
                }
            }
        }
        versions::verify(&r.version_details)?;
        let (hash, bytes) = files::hash_file(&path)?;
        let descriptor = json!({"sourceRevision":r.revision,"reportHash":report_hash(&r),"path":relative,"sha256":hash,"bytes":bytes,"version":r.current_version,"cycle":r.cycle,"scopeHash":autonomy::scope(&r),
            "archiveHash":r.version_details["archiveHash"],"playerBuildHash":r.version_details["playerBuildHash"],"status":"core-verified","createdAt":now()});
        self.production_commit(task, expected, |record| {
            record.workbench.execution.verification_package = descriptor;
            Ok(())
        })?;
        self.production_get(task)
    }
    pub(super) fn production_auto_package(&self, task: &str) -> Option<String> {
        let r = self.production_get(task).ok()?.record?;
        if !completion::enabled(&r)
            || r.current_stage != "implementation"
            || r.current_version.is_none()
            || r.workbench.execution.verification_package["version"].as_str()
                == r.current_version.as_deref()
        {
            return None;
        }
        let ready = ["lua", "input", "core-loop", "save-reload"]
            .iter()
            .all(|id| {
                let selected = if *id == "input"
                    && r.self_tests.iter().any(|t| {
                        t["id"] == "playable-input"
                            && t["version"].as_str() == r.current_version.as_deref()
                            && t["cycle"] == r.cycle
                    }) {
                    "playable-input"
                } else {
                    id
                };
                r.self_tests
                    .iter()
                    .rev()
                    .find(|t| {
                        t["id"] == selected
                            && t["version"].as_str() == r.current_version.as_deref()
                            && t["cycle"] == r.cycle
                    })
                    .is_some_and(|t| t["status"] == "passed")
            });
        if !ready {
            return None;
        }
        self.production_build_package(task, r.revision)
            .err()
            .map(|e| e.message)
    }
}
