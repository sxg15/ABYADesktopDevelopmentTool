use super::*;
use std::collections::BTreeSet;

/// 可复用源码与原生资产仅作为文件交付，不在导入或查阅时执行。
pub(super) fn validate(value: &Value) -> AppResult<()> {
    if value.is_null() {
        return Ok(());
    }
    if value["schema"] != "abya.art-resources/v1"
        || !["asset", "effect", "interaction", "native-guide"]
            .contains(&value["kind"].as_str().unwrap_or(""))
    {
        return Err(AppError::validation("资源包 schema 或类型无效。"));
    }
    for key in [
        "license",
        "compatibility",
        "lifecycle",
        "integration",
        "verification",
    ] {
        super::art_design::text(value, key, 8000)?;
    }
    let items = value["files"]
        .as_array()
        .filter(|x| !x.is_empty() && x.len() <= 256)
        .ok_or_else(|| AppError::validation("资源包须声明1至256个文件。"))?;
    let mut names = BTreeSet::new();
    for item in items {
        let name = mutation::text(item, "name")?;
        let ext = Path::new(name)
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_lowercase();
        if name.contains(['\\', ':'])
            || name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            || !names.insert(name.to_lowercase())
            || ![
                "lua",
                "json",
                "png",
                "jpg",
                "jpeg",
                "webp",
                "svg",
                "mat",
                "prefab",
                "asset",
                "meta",
                "fbx",
                "glb",
                "ttf",
                "otf",
                "shader",
                "hlsl",
                "cs",
                "md",
                "js",
                "mjs",
                "html",
                "css",
                "wav",
                "ogg",
                "anim",
                "controller",
                "mp4",
                "webm",
                "gif",
            ]
            .contains(&ext.as_str())
        {
            return Err(AppError::validation("资源文件名重复、越界或格式不受支持。"));
        }
        let sha = mutation::text(item, "sha256")?;
        if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppError::validation("资源文件需要实际 SHA-256。"));
        }
        mutation::text(item, "path")?;
        mutation::text(item, "role")?;
    }
    for item in items {
        for dependency in item["dependencies"].as_array().into_iter().flatten() {
            if !dependency
                .as_str()
                .is_some_and(|s| names.contains(&s.to_lowercase()))
            {
                return Err(AppError::validation("资源依赖未包含在文件清单内。"));
            }
        }
    }
    if !value["parameters"].is_object()
        || !value["requiredCapabilities"].is_array()
        || !value["tests"].is_array()
    {
        return Err(AppError::validation(
            "资源包必须声明参数、所需能力和测试场景。",
        ));
    }
    Ok(())
}

pub(super) fn read_files(root: &Path, value: &Value) -> AppResult<Vec<(Value, Vec<u8>)>> {
    validate(value)?;
    let mut output = vec![];
    let mut total = 0;
    for item in value["files"].as_array().into_iter().flatten() {
        let path = files::safe_path(root, mutation::text(item, "path")?)?;
        let (sha, size) = files::hash_file(&path)?;
        total += size;
        if item["sha256"] != sha || size > 64 * 1024 * 1024 || total > 128 * 1024 * 1024 {
            return Err(AppError::validation("资源文件已变化或超过资源包容量。"));
        }
        output.push((item.clone(), std::fs::read(path)?));
    }
    Ok(output)
}

pub(super) fn copy(
    root: &Path,
    prefix: &str,
    value: &Value,
    payloads: &[(Value, Vec<u8>)],
) -> AppResult<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let mut result = value.clone();
    let mut copied = vec![];
    for (item, bytes) in payloads {
        let path = format!("{prefix}/{}", mutation::text(item, "name")?);
        let full = files::safe_path(root, &path)?;
        if full.exists() && files::hash_file(&full)?.0 != item["sha256"] {
            return Err(AppError::validation("目标资源已修改，未覆盖。"));
        }
        if !full.exists() {
            files::write_file(root, &path, bytes)?;
        }
        let mut item = item.clone();
        item["path"] = json!(path);
        copied.push(item);
    }
    result["files"] = json!(copied);
    Ok(result)
}
