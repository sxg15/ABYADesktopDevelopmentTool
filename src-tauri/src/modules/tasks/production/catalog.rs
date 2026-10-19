use super::{SkillEntry, TaskService, files, now, policy};
use crate::foundation::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::path::Path;

fn tree_hash(workspace: &Path, relative: &str) -> AppResult<String> {
    let canonical = workspace.canonicalize()?;
    let workspace = canonical.as_path();
    let directory = files::safe_path(workspace, relative)?;
    let mut pending = vec![directory];
    let mut entries = Vec::new();
    while let Some(folder) = pending.pop() {
        for entry in std::fs::read_dir(folder)? {
            let entry = entry?;
            let name = entry
                .path()
                .strip_prefix(workspace)
                .map_err(AppError::internal)?
                .to_string_lossy()
                .replace('\\', "/");
            let path = files::safe_path(workspace, &name)?;
            if path.is_dir() {
                pending.push(path);
            } else {
                entries.push((name, files::hash_file(&path)?.0));
            }
        }
    }
    entries.sort();
    let mut hash = Sha256::new();
    for (name, value) in entries {
        hash.update(name);
        hash.update([0]);
        hash.update(value);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(super) fn scan(workspace: &Path) -> AppResult<Vec<SkillEntry>> {
    let mut output = Vec::new();
    for provider in [".codex"] {
        let parent = files::safe_path(workspace, &format!("{provider}/skills"))?;
        if !parent.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(parent)? {
            let entry = entry?;
            let id = entry.file_name().to_string_lossy().into_owned();
            let relative = format!("{provider}/skills/{id}");
            let folder = files::safe_path(workspace, &relative)?;
            if !folder.is_dir() || !folder.join("SKILL.md").is_file() {
                continue;
            }
            let (content, _) = files::text_file(workspace, &format!("{relative}/SKILL.md"))?;
            let mut kind = "user";
            let mut name = id.clone();
            let mut description = content
                .lines()
                .find_map(|l| l.strip_prefix("description:"))
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_owned();
            if id == "abya-game-development-task" {
                kind = "workflow";
            }
            for (prefix, label) in [("abya-task-template", "task"), ("abya-art-template", "art")] {
                if super::super::is_managed_template(&folder, &id) && id.starts_with(prefix) {
                    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(
                        folder.join(format!("{prefix}.json")),
                    )?)?;
                    kind = label;
                    name = marker["displayName"].as_str().unwrap_or(&id).into();
                    description = marker["description"].as_str().unwrap_or("").into();
                }
            }
            output.push(SkillEntry {
                id,
                provider: provider.trim_start_matches('.').into(),
                kind: kind.into(),
                name,
                description,
                path: relative.clone(),
                sha256: tree_hash(workspace, &relative)?,
            });
        }
    }
    output.sort_by(|a, b| (&a.provider, &a.id).cmp(&(&b.provider, &b.id)));
    Ok(output)
}

pub(super) fn validate_choice(root: &Path, key: &str, id: &str) -> AppResult<()> {
    if id == "none" {
        return Ok(());
    }
    let prefix = if key == "taskTemplate" {
        "abya-task-template-"
    } else {
        "abya-art-template-"
    };
    if !id.starts_with(prefix) {
        return Err(AppError::validation("模板种类不匹配。"));
    }
    let folder = files::safe_path(root, &format!(".codex/skills/{id}"))?;
    if !super::super::is_managed_template(&folder, id) {
        return Err(AppError::validation("选定模板不可用。"));
    }
    Ok(())
}

impl TaskService {
    pub fn production_catalog(&self, id: &str) -> AppResult<Vec<SkillEntry>> {
        let root = self.production_workspace(id)?;
        let mut entries = scan(&root)?;
        // Browsing projections only: the owning workflow remains the single pinned tree.
        for provider in ["codex"] {
            if !entries
                .iter()
                .any(|entry| entry.provider == provider && entry.id == "abya-game-development-task")
            {
                continue;
            }
            for (suffix, name) in [
                ("references/game-art-design", "game-art-design"),
                (
                    "references/game-art-design/references/product-interface-design",
                    "product-interface-design",
                ),
            ] {
                let skill_id = format!("abya-game-development-task/{suffix}");
                let relative = format!(".{provider}/skills/{skill_id}");
                let path = files::safe_path(&root, &format!("{relative}/SKILL.md"))?;
                if !path.is_file() {
                    continue;
                }
                let (content, _) = files::text_file(&root, &format!("{relative}/SKILL.md"))?;
                let description = content
                    .lines()
                    .find_map(|line| line.strip_prefix("description:"))
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .to_owned();
                entries.push(SkillEntry {
                    id: skill_id,
                    provider: provider.into(),
                    kind: "design".into(),
                    name: name.into(),
                    description,
                    path: relative.clone(),
                    sha256: tree_hash(&root, &relative)?,
                });
            }
        }
        Ok(entries)
    }
    pub fn production_skill_text(
        &self,
        id: &str,
        provider: &str,
        skill: &str,
    ) -> AppResult<String> {
        if !["codex"].contains(&provider) {
            return Err(AppError::validation("无效 provider。"));
        }
        let root = self.production_workspace(id)?;
        Ok(files::text_file(&root, &format!(".{provider}/skills/{skill}/SKILL.md"))?.0)
    }
}

impl TaskService {
    pub fn production_upgrade(
        &self,
        id: &str,
        expected_revision: u64,
    ) -> AppResult<super::ProductionView> {
        if crate::foundation::cli_sessions::task_active(id) {
            return Err(AppError::validation(
                "请先停止此任务的终端，再升级流程版本。",
            ));
        }
        let root = self.production_workspace(id)?.canonicalize()?;
        self.production_commit(id, expected_revision, |r| {
            for pin in r.skill_pins.iter().filter(|p| p.kind != "user" && p.provider == "codex") {
                if tree_hash(&root, &pin.path)? != pin.sha256 {
                    return Err(AppError::validation("任务中的受管 Skill 有本地修改，请先处理差异；升级未执行。"));
                }
            }
            let backup = format!("{}/skill-backups/{}", super::ROOT, uuid::Uuid::new_v4());
            for pin in r.skill_pins.iter().filter(|p| p.kind != "user" && p.provider == "codex") {
                let from = files::safe_path(&root, &pin.path)?;
                let mut pending = vec![from.clone()];
                while let Some(folder) = pending.pop() {
                    for entry in std::fs::read_dir(folder)? {
                        let entry = entry?;
                        let relative = entry.path().strip_prefix(&root).map_err(AppError::internal)?.to_string_lossy().replace('\\', "/");
                        let checked = files::safe_path(&root, &relative)?;
                        if checked.is_dir() { pending.push(checked); }
                        else { files::write_file(&root, &format!("{backup}/{relative}"), &std::fs::read(checked)?)?; }
                    }
                }
            }
            self.sync_managed_skills(&root)?;
            r.skill_pins = scan(&root)?;
            let next_policy=policy();
            let design_upgrade = !super::art_design_sources::enabled(r) && next_policy["artDesignVersion"] == 1;
            let art_upgrade = !super::interaction::art_approval(r) && next_policy["stages"].as_array().is_some_and(|stages| stages.iter().any(|s| s["approval"] == "resources"));
            let feedback_upgrade = !super::iteration::enabled(r) && next_policy["iterationMode"] == "human-feedback";
            let compatible=["rounds","questions","dimensions","stages","checkStatuses","stageStatuses"].iter().all(|key|r.policy[*key]==next_policy[*key]);
            r.policy = next_policy; r.workflow_version = r.policy["workflowVersion"].as_str().unwrap().into();
            if art_upgrade || design_upgrade {
                for stage in ["resources", "plan", "implementation", "review", "delivery", "closeout"] { r.stages.insert(stage.into(), "not-started".into()); }
                r.current_stage = if r.stages.get("requirements").is_some_and(|s| s == "passed") { "resources" } else { "requirements" }.into();
                r.stages.insert(r.current_stage.clone(), "in-progress".into());
                r.issues.push(serde_json::json!({"id":format!("art-review-upgrade-{}",r.revision),"kind":"suggestion","status":"resolved","stage":"resources",
                    "description":"已升级美术需求与设计追踪。既有文档、批准、轮次与证据保留为历史；需核对策划原话、参考、设计落实及受影响的后续阶段，不补造确认。","backup":backup,"updatedAt":now()}));
                return Ok(());
            }
            if feedback_upgrade {
                for s in r.policy["stages"].as_array().unwrap() {
                    r.stages.entry(s["id"].as_str().unwrap().into()).or_insert("not-started".into());
                }
                if r.stages.get("plan").is_some_and(|s| s == "passed") {
                    r.current_stage = "implementation".into();
                    r.stages.insert("implementation".into(), "in-progress".into());
                    for stage in ["review", "delivery", "closeout"] { r.stages.insert(stage.into(), "not-started".into()); }
                }
                r.issues.push(serde_json::json!({"id":format!("feedback-upgrade-{}",r.revision),"kind":"suggestion","status":"resolved","stage":"closeout",
                    "description":"已切换人工反馈流程；需求和计划确认、旧轮次与证据保留。当前作品须登记必要自测后交给策划验收，不补造新流程通过。","backup":backup,"updatedAt":now()}));
                return Ok(());
            }
            if compatible {
                r.issues.push(serde_json::json!({"id":format!("workflow-update-{}",r.revision),"kind":"suggestion","status":"resolved","stage":"closeout","description":"制作流程界面与沟通规则已更新，已确认的需求、计划和历史继续有效。","backup":backup,"updatedAt":now()}));
                return Ok(());
            }
            r.cycle += 1; r.current_round = 0; r.milestones.clear(); r.checks.clear();
            for status in r.stages.values_mut() { *status = "not-started".into(); }
            r.current_stage = "requirements".into();
            r.documents.clear();
            r.issues.push(serde_json::json!({"id":format!("upgrade-{}",r.cycle),"kind":"suggestion","status":"open",
                "stage":"requirements","description":"流程已升级，历史记录已保留，请重新提交并确认适用文档。",
                "backup":backup,"updatedAt":now()}));
            Ok(())
        })?;
        self.production_export(id)?;
        self.production_get(id)
    }
}
