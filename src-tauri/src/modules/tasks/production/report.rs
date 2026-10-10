use super::{ProductionRecord, ROOT, TaskService, files, load};
use crate::foundation::{AppError, AppResult};
use serde_json::Value;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn cell(value: &Value) -> String {
    escape(value.as_str().unwrap_or("未提供"))
}
fn relative_url(value: &str) -> String {
    value
        .replace('\\', "/")
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/._-".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn rows(headers: &[&str], values: Vec<Vec<String>>) -> String {
    let head = headers
        .iter()
        .map(|x| format!("<th>{}</th>", escape(x)))
        .collect::<String>();
    let body = values
        .iter()
        .map(|row| {
            format!(
                "<tr>{}</tr>",
                row.iter()
                    .map(|v| format!("<td>{}</td>", escape(v)))
                    .collect::<String>()
            )
        })
        .collect::<String>();
    format!("<table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table>")
}
fn document(r: &ProductionRecord, kind: &str) -> String {
    r.documents.get(kind).map_or_else(
        || "<p class=missing>尚未提交</p>".into(),
        |d| {
            format!(
                "<p>第 {} 版</p><details><summary>查看文档正文</summary><small>{}</small><pre>{}</pre></details>",
                d.revision,
                escape(&d.sha256),
                escape(&d.content)
            )
        },
    )
}

pub(super) fn render(r: &ProductionRecord, phase: &str, warnings: &[String]) -> String {
    let title = match phase {
        "plan" => "需求与方案",
        "review" => "开发与整体迭代",
        _ => "交付与知识分析",
    };
    let mut body = format!(
        "<h1>{title}</h1><p>任务 {} · 流程 {} · 记录版本 {} · 更新 {}</p>",
        escape(&r.task_id),
        escape(&r.workflow_version),
        r.revision,
        escape(&r.updated_at)
    );
    body += "<nav><a href='plan-report.html'>需求与方案</a> · <a href='review-report.html'>开发与迭代</a> · <a href='closeout-report.html'>交付与复盘</a></nav>";
    if let Some(update) = r.stage_updates.get(&r.current_stage) {
        body += &format!(
            "<p>{}</p><p>下一步：{}</p>",
            cell(&update["summary"]),
            cell(&update["nextAction"])
        );
    }
    for warning in warnings {
        body += &format!("<p class=missing>{}</p>", escape(warning));
    }
    body += &rows(
        &["阶段", "状态"],
        r.policy["stages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                vec![
                    s["name"].as_str().unwrap().into(),
                    status_text(&super::workspace::stage_statuses(r)[s["id"].as_str().unwrap()])
                        .into(),
                ]
            })
            .collect(),
    );
    if phase == "plan" {
        body += "<h2>资源与视觉预览</h2>";
        for a in r.artifacts.iter().filter(|a| a["mediaType"].is_string()) {
            let url = format!(
                "../../../{}",
                relative_url(a["path"].as_str().unwrap_or(""))
            );
            body += &format!(
                "<figure><figcaption>{} · {} · {}</figcaption>",
                cell(&a["title"]),
                cell(&a["sourceType"]),
                cell(&a["visualVersion"])
            );
            if a["mediaType"]
                .as_str()
                .is_some_and(|s| s.starts_with("image/"))
            {
                body += &format!(
                    "<a href='{}'><img style='max-width:100%;max-height:400px' loading='lazy' src='{}' alt='{}'></a>",
                    escape(&url),
                    escape(&url),
                    cell(&a["title"])
                );
            } else {
                body += &format!(
                    "<video controls preload='metadata' style='max-width:100%' src='{}'></video>",
                    escape(&url)
                );
            }
            body += "</figure>";
        }
        body += &format!(
            "<h2>需求问答</h2><p>人数模式：{}</p>",
            escape(&r.player_mode)
        );
        for group in r
            .question_groups
            .iter()
            .filter(|g| ["requirements", "resources", "plan"].contains(&g.stage.as_str()))
        {
            body += &format!(
                "<details><summary>{} · {}</summary><p>发布 {} · 更新 {}</p>",
                escape(&group.title),
                escape(&group.status),
                escape(&group.published_at),
                escape(&group.updated_at)
            );
            for version in &group.answers {
                body += &format!(
                    "<h3>答案第 {} 版 · {}</h3>",
                    version.revision,
                    escape(&version.submitted_at)
                );
                for question in &group.questions {
                    body += &format!(
                        "<p>{}：{}</p>",
                        escape(&question.text),
                        escape(
                            version
                                .answers
                                .get(&question.id)
                                .map(String::as_str)
                                .unwrap_or("未回答")
                        )
                    );
                }
            }
            body += "</details>";
        }
        body += "<h2>需求文档</h2>";
        body += &document(r, "requirements");
        body += "<h2>执行计划</h2>";
        body += &document(r, "plan");
    } else if phase == "review" {
        body += "<h2>必要自测与人工反馈</h2>";
        for c in &r.self_tests {
            body += &format!(
                "<p>{} · {} · {} · {}</p>",
                cell(&c["version"]),
                cell(&c["id"]),
                cell(&c["status"]),
                cell(&c["observations"])
            );
        }
        body += &format!(
            "<h2>当前版本</h2><pre>{}</pre>",
            escape(&serde_json::to_string_pretty(&r.version_details).unwrap_or_default())
        );
        for round in &r.rounds {
            body += &format!(
                "<details><summary>周期 {} · R{} · {}</summary>",
                round["cycle"],
                round["number"],
                cell(&round["status"])
            );
            body += &format!(
                "<p>输入 {} → 输出 {}</p>",
                cell(&round["inputVersion"]),
                cell(&round["outputVersion"])
            );
            for check in round["checks"].as_array().into_iter().flatten() {
                body += &format!(
                    "<p><b>{}</b> · {} · {}</p>",
                    cell(&check["dimensionId"]),
                    cell(&check["status"]),
                    cell(&check["observations"])
                );
            }
            for answer in round["questions"].as_array().into_iter().flatten() {
                body += &format!("<h3>{}</h3>", cell(&answer["questionId"]));
                for key in [
                    "conclusion",
                    "counterexample",
                    "problem",
                    "change",
                    "recheck",
                ] {
                    body += &format!("<p>{key}: {}</p>", cell(&answer[key]));
                }
            }
            body += "</details>";
        }
    } else {
        body += "<h2>交付候选</h2>";
        body += &document(r, "delivery");
        body += "<h2>收尾</h2>";
        body += &document(r, "closeout");
        body += "<h2>知识采用、候选与工具建议</h2>";
        for knowledge in &r.knowledge {
            body += &format!(
                "<p>{} · {}</p>",
                cell(&knowledge["status"]),
                cell(&knowledge["summary"])
            );
        }
    }
    body += "<h2>阶段成果</h2>";
    for a in r
        .artifacts
        .iter()
        .filter(|a| phase_for(a["stage"].as_str().unwrap_or("closeout")) == phase)
    {
        body += &format!(
            "<p><a href='../../../{}'>{}</a> · {}</p>",
            relative_url(a["path"].as_str().unwrap_or("")),
            cell(&a["title"]),
            cell(&a["summary"])
        );
    }
    body += "<h2>问题与处理</h2>";
    for i in r
        .issues
        .iter()
        .filter(|i| phase_for(i["stage"].as_str().unwrap_or("closeout")) == phase)
    {
        body += &format!(
            "<details {}><summary>{} · {}</summary><p>{}</p><p>处理方案：{}</p><p>复验结果：{}</p><small>{}</small></details>",
            if i["status"] == "resolved" {
                ""
            } else {
                "open"
            },
            cell(i.get("title").unwrap_or(&i["id"])),
            status_text(i["status"].as_str().unwrap_or("open")),
            cell(&i["description"]),
            cell(i.get("fix").unwrap_or(&i["resumeWhen"])),
            cell(&i["recheck"]),
            cell(&i["id"])
        );
    }
    if phase != "plan" {
        body += "<h2>阶段问答</h2>";
        for g in r
            .question_groups
            .iter()
            .filter(|g| phase_for(&g.stage) == phase)
        {
            body += &format!("<details><summary>{}</summary>", escape(&g.title));
            for a in &g.answers {
                for q in &g.questions {
                    body += &format!(
                        "<p>{}：{}</p>",
                        escape(&q.text),
                        escape(a.answers.get(&q.id).map(String::as_str).unwrap_or("未回答"))
                    );
                }
            }
            body += "</details>";
        }
    }
    if phase == "closeout" {
        body += "<h2>阶段时间记录</h2><p>时间来自APP记录的状态变化。执行区间包含工具与等待，旧记录缺少的时间保持未知。</p>";
        for e in &r.events {
            body += &format!(
                "<p>{} · {} · {}</p>",
                cell(&e["at"]),
                cell(&e["stage"]),
                status_text(e["status"].as_str().unwrap_or(""))
            );
        }
    }
    body += "<h2>策划反馈与处理历史</h2>";
    for f in &r.feedback {
        body += &format!(
            "<details><summary>{} · {}</summary><p>{} → {}</p><p>{}</p><p>{}</p><pre>{}</pre></details>",
            cell(&f["description"]),
            cell(&f["status"]),
            cell(&f["version"]),
            cell(&f["candidateVersion"]),
            cell(&f["fix"]),
            cell(&f["recheck"]),
            escape(&serde_json::to_string_pretty(&f["history"]).unwrap_or_default())
        );
    }
    body += "<h2>用户确认历史</h2>";
    body += &rows(
        &["对象", "版本", "决定", "反馈", "时间"],
        r.approvals
            .iter()
            .map(|a| {
                vec![
                    a.kind.clone(),
                    a.document_revision.to_string(),
                    a.decision.clone(),
                    a.feedback.clone(),
                    a.decided_at.clone(),
                ]
            })
            .collect(),
    );
    body += "<h2>证据索引</h2>";
    for e in &r.evidence {
        let url = format!("../../../{}", relative_url(&e.path));
        body += &format!(
            "<p><a href=\"{}\">{}</a> · {} · {} · {} · {}</p>",
            escape(&url),
            escape(&e.id),
            escape(&e.description),
            escape(&e.version),
            escape(&e.capture_type),
            if e.reviewed {
                "已记录审阅"
            } else {
                "未审阅"
            }
        );
    }
    format!(
        "<!doctype html><html lang=zh-CN><meta charset=utf-8><meta name=viewport content='width=device-width'><title>{title}</title><style>body{{max-width:1100px;margin:32px auto;padding:0 24px;font:16px/1.7 system-ui;color:#20303c;background:#fafbf9}}table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccd4d8;padding:8px;text-align:left}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;background:white;padding:16px}}details{{padding:12px;border-bottom:1px solid #ccc}}.missing{{color:#9c4210}}a{{color:#176852}}</style>{body}</html>"
    )
}

fn phase_for(stage: &str) -> &str {
    match stage {
        "requirements" | "resources" | "plan" => "plan",
        "implementation" | "review" => "review",
        _ => "closeout",
    }
}
fn status_text(status: &str) -> &str {
    match status {
        "not-started" => "未开始",
        "in-progress" => "进行中",
        "awaiting-confirmation" => "待确认",
        "waiting-for-answers" => "待回答",
        "blocked" => "遇到问题",
        "passed" => "已通过",
        "stopped" => "已暂停",
        "open" => "待处理",
        "awaiting-recheck" => "待复验",
        "resolved" => "已解决",
        _ => status,
    }
}

impl TaskService {
    pub fn recording_directory(
        &self,
        task_id: &str,
        instance_id: &str,
    ) -> AppResult<std::path::PathBuf> {
        uuid::Uuid::parse_str(instance_id).map_err(AppError::internal)?;
        let root = self.production_workspace(task_id)?;
        let path = files::safe_path(
            &root,
            &format!("artifacts/runtime/{instance_id}/recordings"),
        )?;
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn production_export(&self, id: &str) -> AppResult<Vec<String>> {
        let root = self.production_workspace(id)?;
        let record = self
            .database
            .with_connection(|c| load(c, id))?
            .ok_or_else(|| AppError::validation("尚未启用制作流程。"))?;
        let warnings = super::validation::warnings(&record, &root);
        files::write_file(
            &root,
            &format!("{ROOT}/workflow.json"),
            &serde_json::to_vec_pretty(&record)?,
        )?;
        let mut result = Vec::new();
        for phase in ["plan", "review", "closeout"] {
            let relative = format!("{ROOT}/reports/{phase}-report.html");
            files::write_file(
                &root,
                &relative,
                render(&record, phase, &warnings).as_bytes(),
            )?;
            result.push(
                files::safe_path(&root, &relative)?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        for round in &record.rounds {
            let relative = format!(
                "{ROOT}/rounds/cycle-{}-R{:02}.json",
                round["cycle"],
                round["number"].as_u64().unwrap_or(0)
            );
            files::write_file(&root, &relative, &serde_json::to_vec_pretty(round)?)?;
        }
        Ok(result)
    }

    pub fn production_artifact_path(&self, id: &str, relative: &str) -> AppResult<String> {
        let path = files::safe_path(&self.production_workspace(id)?, relative)?;
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ![
            "md", "json", "txt", "log", "png", "jpg", "jpeg", "webp", "mp4", "mkv", "webm", "avi",
            "html",
        ]
        .contains(&extension.as_str())
        {
            return Err(AppError::validation("仅可打开任务文档、媒体和生成报告。"));
        }
        if extension == "html" {
            let allowed = ["plan", "review", "closeout"]
                .iter()
                .any(|p| relative == format!("{ROOT}/reports/{p}-report.html"));
            if !allowed {
                return Err(AppError::validation("只能打开固定生成的 HTML 报告。"));
            }
            self.production_export(id)?;
        }
        if !path.is_file() {
            return Err(AppError::not_found("Artifact"));
        }
        Ok(path.to_string_lossy().into_owned())
    }

    pub fn production_report_path(&self, id: &str, phase: &str) -> AppResult<String> {
        if !["plan", "review", "closeout"].contains(&phase) {
            return Err(AppError::validation("无效阶段报告。"));
        }
        self.production_artifact_path(id, &format!("{ROOT}/reports/{phase}-report.html"))
    }
}
