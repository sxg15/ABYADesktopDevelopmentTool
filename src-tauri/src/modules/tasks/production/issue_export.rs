use super::*;

// Native fields remain literal text even if they contain Markdown or HTML.
fn block(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(3.max(longest + 1));
    format!("{fence}\n{text}\n{fence}\n\n")
}

fn field(out: &mut String, label: &str, value: &Value) {
    out.push_str(&format!("### {label}\n\n"));
    let text = match value {
        Value::Null => "未记录".into(),
        Value::String(s) if s.trim().is_empty() => "未记录".into(),
        Value::String(s) => s.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_default(),
    };
    out.push_str(&block(&text));
}

impl TaskService {
    pub fn export_programmer_issues(
        &self,
        task_id: &str,
        expected_revision: u64,
        issue_ids: &[String],
    ) -> AppResult<String> {
        let task = self.get_readonly(task_id)?;
        let record = self
            .database
            .with_connection(|c| load(c, task_id))?
            .ok_or_else(|| AppError::validation("尚未启用制作流程。"))?;
        if record.revision != expected_revision {
            return Err(conflict());
        }
        if issue_ids.is_empty() {
            return Err(AppError::validation("没有可导出的程序问题。"));
        }
        let mut selected = Vec::new();
        for id in issue_ids {
            let issue = record
                .issues
                .iter()
                .find(|i| i["id"].as_str() == Some(id))
                .ok_or_else(|| AppError::not_found("Issue"))?;
            if !issue_repair::needs_programmer(issue) {
                return Err(AppError::validation("只能导出 ABYA 源码／构建包问题。"));
            }
            if !selected.contains(&issue) {
                selected.push(issue);
            }
        }
        let root = Path::new(&task.workspace_path);
        let mut out = String::from("# ABYA 源码／构建包问题交接文档\n\n");
        out.push_str(&format!(
            "导出时间（UTC）：{}\n\n记录版本：{}；问题数：{}\n\n",
            now(),
            record.revision,
            selected.len()
        ));
        out.push_str("本文为已登记问题的快照；疑似问题仍需程序排查。导出不代表问题已修复或通过验收。证据仅列记录和路径，附件需另行提供；未重新验证文件内容。\n\n");
        field(
            &mut out,
            "任务",
            &json!({"title":task.title,"taskId":task_id,"workspace":task.workspace_path}),
        );
        field(
            &mut out,
            "制作与构建上下文",
            &json!({"workflowVersion":record.workflow_version,"currentStage":record.current_stage,"cycle":record.cycle,"currentVersion":record.current_version,"versionDetails":record.version_details}),
        );
        for (index, issue) in selected.iter().enumerate() {
            out.push_str(&format!("## 问题 {}\n\n", index + 1));
            out.push_str(match issue["repairTarget"].as_str() {
                Some("abya-source") => "归属：ABYA 游戏源码缺陷（已登记）\n\n",
                Some("abya-build") => "归属：ABYA 构建包缺陷（已登记）\n\n",
                _ => "归属：疑似 ABYA 源码／构建包缺陷，待程序排查\n\n",
            });
            for (key, label) in [
                ("id", "问题 ID"),
                ("title", "标题"),
                ("kind", "类型"),
                ("status", "状态"),
                ("stage", "发现阶段"),
                ("affectedStages", "影响阶段"),
                ("blockedOperations", "受阻操作"),
                ("description", "问题描述／复现线索"),
                ("fix", "已尝试处理／修复方案"),
                ("resumeWhen", "恢复条件"),
                ("recheck", "复验结果"),
            ] {
                field(&mut out, label, &issue[key]);
            }
            out.push_str("### 关联证据\n\n");
            let ids = issue["evidenceIds"].as_array().cloned().unwrap_or_default();
            if ids.is_empty() {
                out.push_str("未关联证据。\n\n");
            }
            for id in ids {
                let evidence = record
                    .evidence
                    .iter()
                    .find(|e| Some(e.id.as_str()) == id.as_str());
                let entry = match evidence {
                    Some(e) => {
                        json!({"record":e,"fileStatus":match files::safe_path(root, &e.path) {
                            Ok(path) if path.is_file() => "文件存在，未复核哈希",
                            Ok(_) => "文件缺失",
                            Err(_) => "路径不可访问",
                        }})
                    }
                    None => json!({"id":id,"fileStatus":"证据记录缺失"}),
                };
                out.push_str(&block(&serde_json::to_string_pretty(&entry)?));
            }
        }
        let relative = format!(
            "{ROOT}/reports/programmer-issues-r{}-{}.md",
            record.revision,
            uuid::Uuid::new_v4()
        );
        files::write_file(root, &relative, out.as_bytes())?;
        Ok(relative)
    }
}
