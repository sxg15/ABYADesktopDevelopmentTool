mod autonomy;
mod autonomy_budget;
mod autonomy_service;
mod receipts;
pub use autonomy::autonomous_implementation;
mod acceptance;
mod art_design;
mod art_design_sources;
mod art_design_validation;
mod art_library;
mod art_library_package;
mod art_library_requests;
mod art_recovery;
mod art_references;
pub use acceptance::AcceptanceSpec;
mod catalog;
mod files;
mod interaction;
mod issue_export;
mod issue_repair;
pub use issue_repair::IssueRepairSpec;
mod execution;
mod iteration;
mod models;
mod mutation;
mod questions;
mod report;
mod validation;
mod validation_runs;
mod version_snapshots;
mod versions;
mod visuals;
mod workbench;
mod workbench_actions;
mod workbench_import;
mod workbench_package;
mod workspace;
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
        let mut record = ProductionRecord {
            workbench: WorkbenchState::default(),
            art_design: ArtDesign::default(),
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
            artifacts: vec![],
            acceptance_config: Value::Null,
            acceptance_sessions: vec![],
            feedback: vec![],
            self_tests: vec![],
            stage_updates: BTreeMap::new(),
            events: vec![
                json!({"stage":"requirements","status":"in-progress","at":now(),"cycle":1,"revision":1}),
            ],
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
        art_design_sources::refresh(&mut record, &task);
        workbench::observe(&mut record, None);
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
        crate::foundation::change_signal::notify();
        self.production_get(&task.id)
    }

    pub fn production_decide(&self, input: ProductionDecision) -> AppResult<ProductionView> {
        let root = self.production_workspace(&input.task_id)?;
        self.production_commit(&input.task_id, input.expected_revision, |r| {
            interaction::review(r, &root, &input.kind)?;
            if !input.accepted && input.feedback.trim().is_empty() {
                return Err(AppError::validation("退回修改时请填写具体修改意见。"));
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
                interaction::approve(r, &root, &input.kind)?;
                visuals::verify_bindings(&doc, &root)?;
                validation::document_gate(r, &root, &input.kind)?;
                validation::clear_issues(r, &input.kind)?;
                workspace::accept_templates(r, &root, &doc)?;
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
            if input.kind == "delivery" && input.accepted && iteration::enabled(r) {
                r.stages.insert("review".into(), "passed".into());
            }
            let status = if input.accepted {
                "passed"
            } else {
                "in-progress"
            };
            r.stages.insert(input.kind.clone(), status.into());
            r.current_stage = if input.accepted {
                match input.kind.as_str() {
                    "requirements" => "resources",
                    "resources" => "plan",
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
        self.production_view(id, &root, false)
    }

    /// A bounded projection for all-task attention monitoring. No export, Skill sync,
    /// workspace creation, report generation or full Player hashing is performed.
    pub fn production_attention(&self, task: &super::DevelopmentTask) -> AppResult<ProductionView> {
        self.production_view(&task.id, Path::new(&task.workspace_path), true)
    }

    fn production_view(
        &self,
        id: &str,
        root: &Path,
        attention_only: bool,
    ) -> AppResult<ProductionView> {
        let mut record = self.database.with_connection(|c| load(c, id))?;
        let task = self.database.with_connection(|c| read_task(c, id))?;
        if let Some(r) = &mut record {
            art_design_sources::refresh(r, &task);
            if r.workbench.goals.is_empty() {
                workbench::observe(r, None);
            }
        }
        let warnings = record
            .as_ref()
            .filter(|_| !attention_only)
            .map(|r| validation::warnings(r, root))
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
        let stage_statuses = record
            .as_ref()
            .map(workspace::stage_statuses)
            .unwrap_or_default();
        let artifacts = record
            .as_ref()
            .filter(|_| !attention_only)
            .map(|r| workspace::artifacts(r, root))
            .unwrap_or_default();
        let active = self
            .database
            .with_connection(|c| Ok(read_task(c, id)?.status == TaskStatus::Active))?;
        let actions = record
            .as_ref()
            .map(|r| interaction::actions(r, root, active))
            .unwrap_or_else(|| json!({}));
        Ok(ProductionView {
            approval_statuses: record.as_ref().map(|r| r.documents.keys().map(|kind| {
                (kind.clone(), validation::accepted_document(r, root, kind).err().map(|e| e.message).unwrap_or_default())
            }).collect()).unwrap_or_default(),
            art_design_status: record.as_ref().map(|r| json!({"enabled":art_design_sources::enabled(r),
                "inputHash":art_design_sources::inputs_hash(r),"designHash":art_design_sources::design_hash(r)})).unwrap_or(Value::Null),
            actions,
            stage_statuses,
            artifacts,
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
        if ["inspect-art-case", "use-art-case"].contains(&input.operation.as_str()) {
            return self.art_library_use(input);
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
            let task = read_task(c, id)?;
            if task.status != TaskStatus::Active {
                return Err(AppError::validation("仅进行中的任务可以修改制作记录。"));
            }
            let mut record =
                load(c, id)?.ok_or_else(|| AppError::validation("请先启用制作流程。"))?;
            if record.revision != expected {
                return Err(conflict());
            }
            let previous = serde_json::to_string(&record)?;
            let previous_case_ids: Vec<_> = record
                .art_design
                .case_uses
                .iter()
                .map(|c| c["id"].clone())
                .collect();
            art_design_sources::refresh(&mut record, &task);
            change(&mut record)?;
            art_design_sources::refresh(&mut record, &task);
            for case in record
                .art_design
                .case_uses
                .iter()
                .filter(|case| !previous_case_ids.contains(&case["id"]))
            {
                let id = mutation::text(case, "caseId")?;
                let version = case["version"]
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or_else(conflict)?;
                let current = art_library::read(c, id, version)?;
                if current["status"] != "accepted"
                    || case["caseHash"] != art_design_sources::hash(&current)
                {
                    return Err(AppError::validation(
                        "案例在引用提交前已变化或撤回，请重新选择。",
                    ));
                }
            }
            workspace::record_transition(&mut record, &previous)?;
            workbench::observe(&mut record, Some(&serde_json::from_str(&previous)?));
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
        })?;
        crate::foundation::change_signal::notify();
        Ok(())
    }
}
