mod catalog;
mod files;
mod models;
mod mutation;
mod questions;
mod report;
mod validation;
mod versions;
pub use versions::VersionSources;
#[cfg(test)]
mod tests;
use super::{TaskService, TaskStatus, read_task};
use crate::foundation::{AppError, AppResult};
use chrono::Utc;
pub use models::*;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const POLICY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../.codex/skills/abya-game-development-task/assets/production/workflow-policy.json"
));
const ROOT: &str = "artifacts/game-development";
pub(super) fn policy() -> Value {
    serde_json::from_str(POLICY).expect("bundled production policy")
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn conflict() -> AppError {
    AppError::new("conflict", "任务记录已变化，请刷新后再操作。", "")
}

fn load(connection: &rusqlite::Connection, id: &str) -> AppResult<Option<ProductionRecord>> {
    let body: Option<String> = connection
        .query_row(
            "SELECT body FROM task_production WHERE task_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    body.map(|text| serde_json::from_str(&text).map_err(Into::into))
        .transpose()
}

impl TaskService {
    fn production_initialize(&self, input: ProductionMutation) -> AppResult<ProductionView> {
        let task = self.get(&input.task_id)?;
        if task.status != TaskStatus::Active || input.expected_revision != 0 {
            return Err(conflict());
        }
        let question = mutation::text(&input.data, "questionMode")?;
        if !["ask", "no-followup"].contains(&question) {
            return Err(AppError::validation("请选择需求提问模式。"));
        }
        let workspace = Path::new(&task.workspace_path);
        if self.production_exists(&task.id)? {
            return Err(conflict());
        }
        let pins = self.production_catalog(&task.id)?;
        let legacy_path = files::safe_path(workspace, &format!("{ROOT}/workflow.json"))?;
        let legacy = if legacy_path.exists() {
            let name = format!("{ROOT}/legacy-{}.json", uuid::Uuid::new_v4());
            let bytes = std::fs::read(legacy_path)?;
            files::write_file(workspace, &name, &bytes)?;
            Some(name)
        } else {
            None
        };
        let mut stages = BTreeMap::new();
        for stage in policy()["stages"].as_array().unwrap() {
            stages.insert(stage["id"].as_str().unwrap().into(), "not-started".into());
        }
        stages.insert("requirements".into(), "in-progress".into());
        let record = ProductionRecord {
            schema: "abya.game-development-workflow/v1".into(),
            task_id: task.id.clone(),
            revision: 1,
            workflow_version: policy()["workflowVersion"].as_str().unwrap().into(),
            policy: policy(),
            question_mode: question.into(),
            player_mode: questions::player_mode(&input.data)?
                .unwrap_or("unspecified")
                .into(),
            question_groups: vec![],
            task_template: None,
            art_template: None,
            current_stage: "requirements".into(),
            current_round: 0,
            cycle: 1,
            stages,
            documents: BTreeMap::new(),
            approvals: vec![],
            current_version: None,
            version_details: json!({}),
            issues: vec![],
            evidence: vec![],
            rounds: vec![],
            milestones: BTreeMap::new(),
            checks: vec![],
            knowledge: vec![],
            skill_pins: pins,
            legacy_record: legacy,
            updated_at: now(),
        };
        self.database.with_connection(|c| {
            if load(c, &task.id)?.is_some() {
                return Err(conflict());
            }
            c.execute(
                "INSERT INTO task_production(task_id,revision,body) VALUES(?1,1,?2)",
                rusqlite::params![task.id, serde_json::to_string(&record)?],
            )?;
            Ok(())
        })?;
        self.production_export(&task.id)?;
        self.production_get(&task.id)
    }

    pub fn production_decide(&self, input: ProductionDecision) -> AppResult<ProductionView> {
        let root = self.production_workspace(&input.task_id)?;
        self.production_commit(&input.task_id, input.expected_revision, |r| {
            if !["requirements", "plan", "delivery"].contains(&input.kind.as_str()) {
                return Err(AppError::validation("该阶段不需要人工确认。"));
            }
            let doc = r
                .documents
                .get(&input.kind)
                .ok_or_else(|| AppError::validation("尚未提交待确认文档。"))?
                .clone();
            if r.stages.get(&input.kind).map(String::as_str) != Some("awaiting-confirmation") {
                return Err(conflict());
            }
            if doc.sha256 != input.document_hash
                || files::text_file(&root, &doc.path)?.1 != doc.sha256
            {
                return Err(conflict());
            }
            if input.accepted {
                validation::document_gate(r, &root, &input.kind)?;
            }
            r.approvals.push(Approval {
                kind: input.kind.clone(),
                document_hash: doc.sha256,
                document_revision: doc.revision,
                decision: if input.accepted {
                    "accepted"
                } else {
                    "rejected"
                }
                .into(),
                feedback: input.feedback.chars().take(4000).collect(),
                decided_at: now(),
            });
            let status = if input.accepted {
                "passed"
            } else {
                "in-progress"
            };
            r.stages.insert(input.kind.clone(), status.into());
            r.current_stage = if input.accepted {
                match input.kind.as_str() {
                    "requirements" => "resources",
                    "plan" => "implementation",
                    _ => "closeout",
                }
                .into()
            } else {
                input.kind.clone()
            };
            Ok(())
        })?;
        self.production_export(&input.task_id)?;
        self.production_get(&input.task_id)
    }

    pub(super) fn production_can_complete(&self, id: &str) -> AppResult<()> {
        let root = self.production_workspace(id)?;
        if let Some(record) = self.database.with_connection(|c| load(c, id))? {
            validation::approved(&record, &root, "delivery")?;
            validation::document_gate(&record, &root, "delivery")?;
            if record.stages.get("closeout").map(String::as_str) != Some("passed") {
                return Err(AppError::validation("请先完成交付验收与收尾。"));
            }
        }
        Ok(())
    }
}

impl TaskService {
    pub(super) fn production_exists(&self, id: &str) -> AppResult<bool> {
        self.database.with_connection(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM task_production WHERE task_id=?1",
                [id],
                |r| r.get::<_, u32>(0),
            )? > 0)
        })
    }

    fn production_workspace(&self, id: &str) -> AppResult<PathBuf> {
        let task = self.database.with_connection(|c| read_task(c, id))?;
        Ok(PathBuf::from(task.workspace_path))
    }

    pub fn production_get(&self, id: &str) -> AppResult<ProductionView> {
        let root = self.production_workspace(id)?;
        let record = self.database.with_connection(|c| load(c, id))?;
        let warnings = record
            .as_ref()
            .map(|r| validation::warnings(r, &root))
            .unwrap_or_default();
        let available_update = record
            .as_ref()
            .is_some_and(|r| r.workflow_version != policy()["workflowVersion"]);
        let report_paths = [
            "plan-report.html",
            "review-report.html",
            "closeout-report.html",
        ]
        .iter()
        .map(|name| {
            root.join(ROOT)
                .join("reports")
                .join(name)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
        let effective_policy = record.as_ref().map_or_else(policy, |r| r.policy.clone());
        Ok(ProductionView {
            record,
            policy: effective_policy,
            warnings,
            report_paths,
            available_update,
        })
    }

    pub fn production_update(&self, input: ProductionMutation) -> AppResult<ProductionView> {
        if input.operation == "initialize" {
            return self.production_initialize(input);
        }
        let root = self.production_workspace(&input.task_id)?;
        self.production_commit(&input.task_id, input.expected_revision, |record| {
            mutation::apply(record, &root, &input.operation, &input.data)
        })?;
        self.production_export(&input.task_id).map_err(|e| {
            AppError::new(
                "outcome_unknown",
                "记录已保存，但文件导出失败。先读取 production get，不要重放写入。",
                e.to_string(),
            )
        })?;
        self.production_get(&input.task_id)
    }

    fn production_commit(
        &self,
        id: &str,
        expected: u64,
        change: impl FnOnce(&mut ProductionRecord) -> AppResult<()>,
    ) -> AppResult<()> {
        self.database.with_connection(|c| {
            if read_task(c, id)?.status != TaskStatus::Active {
                return Err(AppError::validation("仅进行中的任务可以修改制作记录。"));
            }
            let mut record =
                load(c, id)?.ok_or_else(|| AppError::validation("请先启用制作流程。"))?;
            if record.revision != expected {
                return Err(conflict());
            }
            let previous = serde_json::to_string(&record)?;
            change(&mut record)?;
            record.revision += 1;
            record.updated_at = now();
            let body = serde_json::to_string(&record)?;
            if body.len() > 8 * 1024 * 1024 {
                return Err(AppError::validation(
                    "制作记录超过 8 MiB，请使用文件引用保存证据。",
                ));
            }
            let tx = c.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO task_production_history(task_id,revision,body) VALUES(?1,?2,?3)",
                rusqlite::params![id, expected as i64, previous],
            )?;
            tx.execute(
                "UPDATE task_production SET revision=?2,body=?3 WHERE task_id=?1",
                rusqlite::params![id, record.revision as i64, body],
            )?;
            tx.commit()?;
            Ok(())
        })
    }
}
