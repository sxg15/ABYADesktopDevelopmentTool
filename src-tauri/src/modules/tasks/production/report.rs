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
                "<p>版本 {} · {}</p><pre>{}</pre>",
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
    body += "<p>资料检查、实际运行、视觉审阅与策划验收分别记录；本报告不自动证明作品质量。</p>";
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
                    r.stages[s["id"].as_str().unwrap()].clone(),
                ]
            })
            .collect(),
    );
    if phase == "plan" {
        body += "<h2>需求文档</h2>";
        body += &document(r, "requirements");
        body += "<h2>执行计划</h2>";
        body += &document(r, "plan");
    } else if phase == "review" {
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
    body += "<h2>问题与阻塞</h2>";
    for i in &r.issues {
        body += &format!(
            "<p>{} · {} · {} · {}</p>",
            cell(&i["id"]),
            cell(&i["status"]),
            cell(&i["description"]),
            cell(&i["recheck"])
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
}
