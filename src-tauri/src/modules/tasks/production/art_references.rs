use super::*;
use base64::Engine as _;

pub(super) fn attach(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let name = mutation::text(data, "name")?;
    let summary = data["summary"].as_str().unwrap_or("");
    if name.chars().count() > 160 || summary.chars().count() > 2000 {
        return Err(AppError::validation(
            "图片名称最多160字，参考说明最多2000字。",
        ));
    }
    if r.artifacts
        .iter()
        .filter(|a| a["plannerReference"] == true && a["status"] != "withdrawn")
        .count()
        >= 32
    {
        return Err(AppError::validation(
            "本任务最多保留32张参考图，请先移除不再使用的图片。",
        ));
    }
    let encoded = mutation::text(data, "base64")?;
    if encoded.len() > 24 * 1024 * 1024 {
        return Err(AppError::validation("每张参考图片上限16 MiB。"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| AppError::validation("图片数据无法读取，请重新选择。"))?;
    if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
        return Err(AppError::validation("每张参考图片上限16 MiB，不得为空。"));
    }
    // Derive the extension from bytes, never from a user-provided path or MIME type.
    let extension = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "jpg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        return Err(AppError::validation(
            "请选择 PNG/JPEG/WebP/GIF 图片；不支持网页或视频。",
        ));
    };
    let path = format!(
        "artifacts/art-references/{}.{}",
        uuid::Uuid::new_v4(),
        extension
    );
    files::write_file(root, &path, &bytes)?;
    visuals::register(
        r,
        root,
        &json!({"stage":"resources","path":path,"title":name,
        "summary":summary,"sourceType":"reference","roles":["feedback"],
        "visualVersion":"planner-input","groupId":path,"source":"策划上传参考图"}),
    )?;
    let artifact = r.artifacts.last_mut().expect("registered reference");
    artifact["plannerReference"] = json!(true);
    artifact["status"] = json!("reference");
    Ok(())
}

pub(super) fn withdraw(r: &mut ProductionRecord, data: &Value) -> AppResult<()> {
    let id = mutation::text(data, "id")?;
    let a = r
        .artifacts
        .iter_mut()
        .find(|a| a["id"] == id && a["plannerReference"] == true)
        .ok_or_else(|| AppError::validation("参考图不属于本任务。"))?;
    a["status"] = json!("withdrawn");
    a["updatedAt"] = json!(now());
    Ok(())
}
