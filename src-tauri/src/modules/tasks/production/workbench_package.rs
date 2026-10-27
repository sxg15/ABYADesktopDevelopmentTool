use super::*;
use std::io::{Cursor, Read, Write};

impl TaskService {
    pub fn export_developer_package(
        &self,
        task_id: &str,
        revision: u64,
        ids: &[String],
    ) -> AppResult<String> {
        let report = self.export_programmer_issues(task_id, revision, ids)?;
        let task = self.get_readonly(task_id)?;
        let root = Path::new(&task.workspace_path);
        let record = self.production_snapshot(task_id, revision)?;
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut manifest = Vec::new();
        let mut paths = vec![(report.clone(), None)];
        for issue in record
            .issues
            .iter()
            .filter(|i| ids.iter().any(|id| i["id"] == *id))
        {
            for id in issue["evidenceIds"].as_array().into_iter().flatten() {
                if let Some(e) = record
                    .evidence
                    .iter()
                    .find(|e| Some(e.id.as_str()) == id.as_str())
                {
                    if !paths.iter().any(|(p, _)| p == &e.path) {
                        paths.push((e.path.clone(), Some(e.sha256.clone())));
                    }
                } else {
                    manifest.push(json!({"id":id,"status":"omitted","reason":"证据记录缺失"}));
                }
            }
        }
        let mut total = 0usize;
        let selected: Vec<_> = record
            .issues
            .iter()
            .filter(|i| ids.iter().any(|id| i["id"] == *id))
            .collect();
        let checks: Vec<_> = record
            .self_tests
            .iter()
            .filter(|t| {
                t["version"].as_str() == record.current_version.as_deref()
                    && t["cycle"] == record.cycle
                    && t["blockedByIssueIds"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|i| ids.iter().any(|id| i == id)))
            })
            .collect();
        zip.start_file("diagnosis.json", options)
            .map_err(|e| AppError::validation(e.to_string()))?;
        zip.write_all(&serde_json::to_vec_pretty(&json!({"schema":"abya.issue-diagnosis/v1",
            "taskId":task_id,"revision":revision,"workflowVersion":record.workflow_version,
            "version":record.current_version,"cycle":record.cycle,
            "archiveHash":record.version_details["archiveHash"],"playerBuildHash":record.version_details["playerBuildHash"],
            "issues":selected,"affectedChecks":checks}))?)?;
        for (index, (path, expected)) in paths.into_iter().enumerate() {
            let result = (|| -> AppResult<Vec<u8>> {
                let file = files::safe_path(root, &path)?;
                let mut bytes = Vec::new();
                std::fs::File::open(&file)?
                    .take(16 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 16 * 1024 * 1024 || total + bytes.len() > 64 * 1024 * 1024 {
                    return Err(AppError::validation("超出单文件16 MiB或总计64 MiB限制"));
                }
                use sha2::{Digest, Sha256};
                let hash = format!("{:x}", Sha256::digest(&bytes));
                if expected.as_ref().is_some_and(|e| e != &hash) {
                    return Err(AppError::validation("证据已改变，哈希不匹配"));
                }
                Ok(bytes)
            })();
            match result {
                Ok(bytes) => {
                    let name = if index == 0 {
                        "report.md".into()
                    } else {
                        format!(
                            "evidence/{index}.{}",
                            Path::new(&path)
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("bin")
                        )
                    };
                    zip.start_file(&name, options)
                        .map_err(|e| AppError::validation(e.to_string()))?;
                    zip.write_all(&bytes)?;
                    total += bytes.len();
                    manifest.push(json!({"source":path,"entry":name,"status":"included","bytes":bytes.len(),"expectedHash":expected}));
                }
                Err(e) => {
                    manifest.push(json!({"source":path,"status":"omitted","reason":e.message}))
                }
            }
        }
        zip.start_file("manifest.json", options)
            .map_err(|e| AppError::validation(e.to_string()))?;
        zip.write_all(&serde_json::to_vec_pretty(&json!({"revision":revision,"files":manifest,
            "scope":"仅包含所选程序问题与其关联证据。未采集完整终端、账号配置或无关项目文件；未自动发送。"}))?)?;
        let bytes = zip
            .finish()
            .map_err(|e| AppError::validation(e.to_string()))?
            .into_inner();
        let path = format!(
            "{ROOT}/reports/developer-feedback-r{revision}-{}.zip",
            uuid::Uuid::new_v4()
        );
        files::write_file(root, &path, &bytes)?;
        Ok(path)
    }
}
