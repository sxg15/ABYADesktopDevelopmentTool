use super::*;
use sha2::{Digest, Sha256};

fn hash(value: &Value) -> Value {
    json!(format!("{:x}", Sha256::digest(value.to_string())))
}

/// 只识别明确的存储结构；保留原始完整哈希，投影仅用于安排复验范围。
pub(super) fn projection(name: &str, mut value: Value) -> Value {
    if name.ends_with(".PBUI") && value["Version"] == 1 {
        let Some(pages) = value["Pages"].as_array_mut() else {
            return Value::Null;
        };
        for page in pages {
            let Some(nodes) = page["Nodes"].as_array_mut() else {
                return Value::Null;
            };
            for node in nodes {
                if !node["Id"].is_string() || !node["Layout"].is_object() {
                    return Value::Null;
                }
                node.as_object_mut().unwrap().remove("Layout");
            }
        }
        return json!({"nonLayoutSha256":hash(&value),"recognizedKind":"ui"});
    }
    if matches!(name, "Main.PBArc" | "Index.PBConf")
        && value["Guid"]
            .as_str()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
    {
        if let Some(time) = value["LastEditTime"].as_str() {
            if chrono::DateTime::parse_from_rfc3339(time).is_err() {
                return Value::Null;
            }
            value.as_object_mut().unwrap().remove("LastEditTime");
        }
        return json!({"contentSha256":hash(&value),"recognizedKind":"archive"});
    }
    Value::Null
}

pub(super) fn enrich(archive: &Path, manifest: &mut Value) -> AppResult<()> {
    let files = manifest["files"].as_array_mut().ok_or_else(conflict)?;
    for file in files {
        let name = file["path"].as_str().ok_or_else(conflict)?.to_owned();
        if !(name.ends_with(".PBUI") || name == "Main.PBArc" || name == "Index.PBConf") {
            continue;
        }
        if file["bytes"].as_u64().unwrap_or(u64::MAX) > 16 * 1024 * 1024 {
            continue;
        }
        let raw = std::fs::read(archive.join(&name))?;
        if let Ok(value) =
            serde_json::from_slice::<Value>(raw.strip_prefix(&[239, 187, 191]).unwrap_or(&raw))
        {
            let projected = projection(&name, value);
            if let Some(fields) = projected.as_object() {
                for (key, value) in fields {
                    file[key] = value.clone();
                }
            }
        }
    }
    manifest["coverage"] = json!("file-hashes-with-declared-projections");
    Ok(())
}

pub(super) fn difference(before: &Value, after: &Value, player_changed: bool) -> Value {
    let mut result = version_snapshots::difference(before, after, player_changed);
    let old = before["files"].as_array();
    if let Some(changed) = result["changedFiles"].as_array_mut() {
        for file in changed {
            if after["files"]
                .as_array()
                .is_none_or(|files| !files.iter().any(|v| v["path"] == file["path"]))
            {
                continue;
            }
            let previous = old.and_then(|files| files.iter().find(|v| v["path"] == file["path"]));
            if let Some(previous) = previous {
                if file["contentSha256"].is_string()
                    && file["contentSha256"] == previous["contentSha256"]
                {
                    file["kind"] = json!("metadata");
                } else if file["nonLayoutSha256"].is_string()
                    && file["nonLayoutSha256"] == previous["nonLayoutSha256"]
                {
                    file["kind"] = json!("layout");
                }
            }
        }
    }
    let changed = result["changedFiles"].as_array().unwrap();
    let unknown = player_changed || old.is_none() || changed.iter().any(|f| f["kind"] == "unknown");
    let checks = if unknown {
        json!([
            "architecture",
            "lua",
            "core-loop",
            "input",
            "visual",
            "lifecycle",
            "save-reload",
            "authority"
        ])
    } else if changed.iter().any(|f| f["kind"] == "logic") {
        json!([
            "architecture",
            "lua",
            "core-loop",
            "input",
            "lifecycle",
            "save-reload",
            "authority"
        ])
    } else if changed.iter().any(|f| f["kind"] != "metadata") {
        json!(["visual", "input", "save-reload"])
    } else {
        json!(["save-reload"])
    };
    result["unknownImpact"] = json!(unknown);
    result["mustRetest"] = checks;
    result["mode"] = json!("scoped");
    result["note"] = json!(
        "完整文件指纹始终保留。仅识别 PBUI v1 的纯 Layout 变化与已解析存档的编辑时间变化；未知字段、绑定、代码或构建影响不忽略。布局变化仍复验输入、视觉和保存重载。"
    );
    result
}
