use super::*;
use base64::Engine as _;

fn media_type(path: &Path) -> AppResult<&'static str> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "webp" => Ok("image/webp"),
        "gif" => Ok("image/gif"),
        "mp4" => Ok("video/mp4"),
        "webm" => Ok("video/webm"),
        _ => Err(AppError::validation(
            "预览支持 PNG/JPEG/WebP/GIF/MP4/WebM。",
        )),
    }
}

pub(super) fn attach_feedback(
    r: &mut ProductionRecord,
    root: &Path,
    data: &Value,
) -> AppResult<()> {
    let name = mutation::text(data, "name")?;
    let extension = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    media_type(Path::new(name))?;
    let encoded = mutation::text(data, "base64")?;
    if encoded.len() > 88 * 1024 * 1024 {
        return Err(AppError::validation("反馈附件过大。"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(AppError::internal)?;
    let limit = if ["mp4", "webm"].contains(&extension.as_str()) {
        64
    } else {
        16
    };
    if bytes.is_empty() || bytes.len() > limit * 1024 * 1024 {
        return Err(AppError::validation("图片上限16 MiB，视频上限64 MiB。"));
    }
    let path = format!("artifacts/feedback/{}.{}", uuid::Uuid::new_v4(), extension);
    files::write_file(root, &path, &bytes)?;
    register(
        r,
        root,
        &json!({"stage":r.current_stage,"path":path,"title":name,"sourceType":"feedback",
        "roles":["feedback"],"visualVersion":r.current_version.as_deref().unwrap_or("proposal"),
        "groupId":path,"source":"策划上传"}),
    )
}

pub(super) fn register(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let path = mutation::text(data, "path")?;
    if !path.replace('\\', "/").starts_with("artifacts/") {
        return Err(AppError::validation(
            "视觉产物须保存在本任务 artifacts 中。",
        ));
    }
    let file = files::safe_path(root, path)?;
    let mime = media_type(&file)?;
    let (sha, bytes) = files::hash_file(&file)?;
    if bytes == 0
        || bytes
            > if mime.starts_with("video/") {
                64 * 1024 * 1024
            } else {
                16 * 1024 * 1024
            }
    {
        return Err(AppError::validation(
            "图片上限16 MiB，视频上限64 MiB，文件不得为空。",
        ));
    }
    let stage = mutation::text(data, "stage")?;
    let source = mutation::text(data, "sourceType")?;
    if !r.stages.contains_key(stage)
        || !["reference", "mockup", "render", "gameplay", "feedback"].contains(&source)
    {
        return Err(AppError::validation("请选择有效阶段及视觉产物类型。"));
    }
    let roles = data["roles"]
        .as_array()
        .ok_or_else(|| AppError::validation("请声明素材板、布局或状态等用途。"))?;
    if roles.is_empty()
        || roles.iter().any(|v| {
            !["asset-board", "layout", "states", "effect", "feedback"]
                .iter()
                .any(|s| v == s)
        })
    {
        return Err(AppError::validation("视觉用途无效。"));
    }
    let title = mutation::text(data, "title")?;
    let version = mutation::text(data, "visualVersion")?;
    let group = mutation::text(data, "groupId")?;
    if [title, version, group]
        .iter()
        .any(|s| s.chars().count() > 160)
    {
        return Err(AppError::validation("视觉名称或版本过长。"));
    }
    if let Some(old) = r
        .artifacts
        .iter()
        .find(|a| a["path"] == path && a["mediaType"].is_string())
    {
        if old["sha256"] == sha && old["visualVersion"] == version {
            return Ok(());
        }
        return Err(AppError::validation(
            "已登记图片不可覆盖，请用新文件保存新版本。",
        ));
    }
    r.artifacts.push(json!({"id":uuid::Uuid::new_v4().to_string(),"path":path,"stage":stage,
        "title":title,"summary":data["summary"].as_str().unwrap_or(""),"mediaType":mime,"sourceType":source,
        "roles":roles,"visualVersion":version,"groupId":group,"sha256":sha,"bytes":bytes,"cycle":r.cycle,
        "temporary":data["temporary"].as_bool().unwrap_or(false),"source":data["source"].as_str().unwrap_or(""),
        "status":"draft","updatedAt":now()}));
    Ok(())
}

pub(super) fn bindings(r: &ProductionRecord, root: &Path) -> AppResult<BTreeMap<String, String>> {
    let mut groups = BTreeMap::new();
    for a in &r.artifacts {
        if a["stage"] == "resources"
            && a["mediaType"].is_string()
            && a["sourceType"] != "gameplay"
            && a["sourceType"] != "feedback"
            && a["cycle"] == r.cycle
        {
            groups.insert(a["groupId"].as_str().unwrap_or(""), a);
        }
    }
    let mut result = BTreeMap::new();
    for a in groups.values() {
        let path = mutation::text(a, "path")?;
        let hash = files::hash_file(&files::safe_path(root, path)?)?.0;
        if a["sha256"] != hash {
            return Err(AppError::validation(
                "视觉方案文件已变化，请以新文件登记修订。",
            ));
        }
        result.insert(path.into(), hash);
    }
    if super::iteration::enabled(r)
        && ["asset-board", "layout", "states"].iter().any(|role| {
            !groups.values().any(|a| {
                a["roles"]
                    .as_array()
                    .is_some_and(|roles| roles.iter().any(|v| v == role))
            })
        })
    {
        return Err(AppError::validation(
            "资源阶段需要素材板、整体布局和关键状态预览；一张组合图可覆盖多个用途。",
        ));
    }
    Ok(result)
}

pub(super) fn verify_bindings(d: &Document, root: &Path) -> AppResult<()> {
    for (path, hash) in &d.artifact_bindings {
        if files::hash_file(&files::safe_path(root, path)?)?.0 != *hash {
            return Err(AppError::validation(
                "已提交的视觉方案文件已变化，请重新提交执行计划。",
            ));
        }
    }
    Ok(())
}

impl TaskService {
    pub fn production_media(&self, id: &str, artifact_id: &str) -> AppResult<String> {
        let root = self.production_workspace(id)?;
        let r = self
            .database
            .with_connection(|c| load(c, id))?
            .ok_or_else(|| AppError::validation("缺少制作记录。"))?;
        let a = r
            .artifacts
            .iter()
            .find(|a| a["id"] == artifact_id && a["mediaType"].is_string())
            .ok_or_else(|| AppError::validation("只能预览此任务已登记的视觉产物。"))?;
        let path = files::safe_path(&root, mutation::text(a, "path")?)?;
        let mime = media_type(&path)?;
        let (sha, bytes) = files::hash_file(&path)?;
        if a["sha256"] != sha || bytes > 64 * 1024 * 1024 {
            return Err(AppError::validation("视觉产物已变化或过大。"));
        }
        Ok(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(std::fs::read(path)?)
        ))
    }
}
