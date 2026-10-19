use super::{memory_models::MemoryPolicy, memory_probe::MemorySample};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

// 报告只保留有界元数据；令牌相关行、URL 与长的不透明值不进入报告。
pub(crate) fn safe_text(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if [
        "token",
        "secret",
        "password",
        "authorization",
        "api_key",
        "credential",
        "bearer",
        "sk-",
        "ghp_",
        "cookie",
        "api-key",
        "apikey",
        "access_key",
        "://",
    ]
    .iter()
    .any(|key| lower.contains(key))
    {
        return "[已过滤敏感内容]".into();
    }
    text.split_whitespace()
        .take(48)
        .map(|part| {
            if part.len() > 96 {
                "[已省略长值]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(512)
        .collect()
}
pub(super) fn append(path: &Path, value: &Value, limit: u64) -> std::io::Result<()> {
    if path.metadata().is_ok_and(|m| m.len() >= limit) {
        let previous = path.with_extension("previous.jsonl");
        if previous.exists() {
            fs::remove_file(&previous)?;
        }
        fs::rename(path, previous)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.flush()
}
pub(super) fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = fs::File::create(&temporary)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.sync_all()?;
    drop(file);
    // 目标是我们自己生成的单个报告文件；不覆盖用户路径或目录。
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)
}
pub(super) fn incident(
    directory: &Path,
    identity: &Value,
    policy: &MemoryPolicy,
    reason: &str,
    samples: &[MemorySample],
    events: &[Value],
) -> std::io::Result<String> {
    let incident = uuid::Uuid::new_v4().to_string();
    let target = directory.join(&incident);
    fs::create_dir(&target)?;
    write_json(
        &target.join("incident.json"),
        &json!({"schemaVersion":1,"incidentId":incident,
        "triggeredAt":chrono::Utc::now().to_rfc3339(),"identity":identity,"policy":policy,"reason":reason,
        "classification":"memory_pressure_or_abnormal_exit_not_proven_leak"}),
    )?;
    for sample in samples {
        append(&target.join("samples.jsonl"), &json!(sample), 512 * 1024)?;
    }
    for event in events {
        append(&target.join("operations.jsonl"), event, 512 * 1024)?;
    }
    write_json(
        &target.join("before-trigger.json"),
        &json!({"samples":samples,"operations":events}),
    )?;
    fs::write(
        target.join("report.md"),
        format!(
            "# ABYA 内存异常记录\n\n事件编号：{incident}\n\n触发原因：{reason}\n\n身份、版本和阈值：incident.json\n采样：samples.jsonl\n操作：operations.jsonl\nCodex 上下文：codex-context.json（若无法取得会记录原因）\n结束或停止结果：outcome.json\n\n私有提交、物理驻留和系统提交量分别记录，单位为字节。\n操作与异常时间相关不等于因果；该记录不证明具体代码泄漏，也不代表玩法验证失败。\n采样可能漏过两次采样间的峰值；监测中断或文件写入失败不得解释为无异常。\n原始命令参数、令牌及连接凭据不复制到此报告；通过请求编号关联原工作流。\n"
        ),
    )?;
    let mut report = OpenOptions::new()
        .append(true)
        .open(target.join("report.md"))?;
    writeln!(
        report,
        "\n## 任务与进程\n\n```json\n{}\n```",
        serde_json::to_string_pretty(identity)?
    )?;
    writeln!(
        report,
        "\n## 触发前最后十个样本\n\n| UTC 时间 | 私有提交 GiB | 驻留 GiB | 系统空闲 GiB |\n|---|---:|---:|---:|"
    )?;
    for sample in samples.iter().rev().take(10).rev() {
        writeln!(
            report,
            "| {} | {:.3} | {:.3} | {:.3} |",
            sample.at,
            sample.private_bytes as f64 / 1073741824.0,
            sample.working_set_bytes as f64 / 1073741824.0,
            sample.available_bytes as f64 / 1073741824.0
        )?;
    }
    writeln!(
        report,
        "\n## 最近操作（完整有界记录见 operations.jsonl）\n\n```json\n{}\n```",
        serde_json::to_string_pretty(&events.iter().rev().take(12).collect::<Vec<_>>())?
    )?;
    report.flush()?;
    player_tail(&target.join("player-tail-before.log"), identity)?;
    writeln!(
        report,
        "\n## Player 日志\n\n启动日志原位置见 playerLogPath。过滤后的触发前/结束后摘录见 player-tail-before.log、player-tail-after.log；完整原日志保持在 InstanceLogs。"
    )?;
    Ok(incident)
}

pub(super) fn player_tail(target: &Path, identity: &Value) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let Some(source) = identity["playerLogPath"].as_str() else {
        return fs::write(target, "未提供 Player 日志路径。");
    };
    let Ok(mut file) = fs::File::open(source) else {
        return fs::write(target, "Player 日志暂不可读，请核对原始路径。");
    };
    let length = file.metadata()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(128 * 1024)))?;
    let mut bytes = Vec::new();
    file.take(128 * 1024).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let filtered: Vec<_> = text.lines().rev().take(200).collect();
    let mut output = fs::File::create(target)?;
    writeln!(
        output,
        "有界日志摘录：最多最后128 KiB/200行；凭据相关内容与不透明长值已过滤。"
    )?;
    for line in filtered.into_iter().rev() {
        let safe = safe_text(line);
        let words: Vec<_> = safe
            .split_whitespace()
            .map(|part| {
                let core = part.trim_matches(|c: char| {
                    !c.is_ascii_alphanumeric() && !matches!(c, '_' | '-' | '=' | '+' | '/')
                });
                if core.len() >= 32
                    && core.chars().all(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '=' | '+' | '/')
                    })
                {
                    "[已过滤不透明值]"
                } else {
                    part
                }
            })
            .collect();
        writeln!(output, "{}", words.join(" "))?;
    }
    output.flush()
}
