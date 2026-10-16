use super::art_library::{directory, media_bytes, read};
use super::*;
use std::io::{Read, Write};

pub(super) fn valid_media(mime: &str, bytes: &[u8]) -> bool {
    match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        "video/mp4" => bytes.get(4..8) == Some(b"ftyp"),
        "video/webm" => bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]),
        _ => false,
    }
}

impl TaskService {
    pub(super) fn art_library_export(&self, task_id: &str, data: &Value) -> AppResult<Value> {
        let task = self.get_readonly(task_id)?;
        let root = Path::new(&task.workspace_path);
        self.database.with_connection(|c| {
            let id = super::art_design::text(data, "id", 100)?;
            let version = data["version"]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or_else(|| AppError::validation("缺少案例版本。"))?;
            let mut entry = read(c, &id, version)?;
            let library = directory(c)?;
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (i, m) in entry["media"]
                .as_array_mut()
                .ok_or_else(|| AppError::validation("案例没有素材。"))?
                .iter_mut()
                .enumerate()
            {
                let bytes = media_bytes(&library, m)?;
                let ext = Path::new(mutation::text(m, "path")?)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let path = format!("media/{i}.{ext}");
                zip.start_file(&path, options).map_err(AppError::internal)?;
                zip.write_all(&bytes)?;
                m["path"] = json!(path);
            }
            // Keep only the curated case, never source task IDs, machine paths, account data or transcripts.
            for key in ["sourceTaskId", "sourceDraftId", "designHash", "cycle"] {
                entry.as_object_mut().unwrap().remove(key);
            }
            zip.start_file("case.json", options)
                .map_err(AppError::internal)?;
            zip.write_all(serde_json::to_string_pretty(&entry)?.as_bytes())?;
            let bytes = zip.finish().map_err(AppError::internal)?.into_inner();
            let path = format!("artifacts/art-library-exports/{}.zip", uuid::Uuid::new_v4());
            files::write_file(root, &path, &bytes)?;
            Ok(json!({"path":path}))
        })
    }

    pub(super) fn art_library_import(&self, data: &Value) -> AppResult<Value> {
        let file = super::art_design::text(data, "path", 32768)?;
        let source = Path::new(&file);
        if std::fs::metadata(source)?.len() > 140 * 1024 * 1024 {
            return Err(AppError::validation("案例包上限140 MiB。"));
        }
        let key = format!("import:{}", files::hash_file(source)?.0);
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(source)?).map_err(AppError::internal)?;
        if archive.len() > 17 {
            return Err(AppError::validation("案例包包含过多文件。"));
        }
        let mut manifest = String::new();
        archive
            .by_name("case.json")
            .map_err(AppError::internal)?
            .take(1024 * 1024 + 1)
            .read_to_string(&mut manifest)?;
        if manifest.len() > 1024 * 1024 {
            return Err(AppError::validation("案例描述过大。"));
        }
        let original: Value = serde_json::from_str(&manifest)?;
        if original["schema"] != "abya.art-case/v1" {
            return Err(AppError::validation("无效案例包格式。"));
        }
        let items = original["media"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 16)
            .ok_or_else(|| AppError::validation("案例包须包含1–16个素材。"))?;
        let mut payloads = vec![];
        let mut total = 0;
        let mut names = std::collections::BTreeSet::new();
        for m in items {
            let path = mutation::text(m, "path")?;
            if !path.starts_with("media/")
                || path.contains('\\')
                || path.contains(':')
                || path
                    .split('/')
                    .any(|p| p.is_empty() || p == ".." || p == ".")
                || !names.insert(path)
            {
                return Err(AppError::validation("案例素材路径无效或重复。"));
            }
            let mut bytes = vec![];
            archive
                .by_name(path)
                .map_err(AppError::internal)?
                .take(64 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            total += bytes.len();
            if bytes.len() > 64 * 1024 * 1024
                || total > 128 * 1024 * 1024
                || !valid_media(mutation::text(m, "mediaType")?, &bytes)
            {
                return Err(AppError::validation("案例素材类型或大小无效。"));
            }
            use sha2::{Digest, Sha256};
            if m["sha256"] != format!("{:x}", Sha256::digest(&bytes)) {
                return Err(AppError::validation("案例素材校验失败。"));
            }
            payloads.push((m.clone(), bytes));
        }
        if archive.len() != payloads.len() + 1 {
            return Err(AppError::validation("案例包只能包含清单及声明的素材。"));
        }
        self.import_art_payload(original, payloads, &key)
    }

    fn import_art_payload(
        &self,
        original: Value,
        payloads: Vec<(Value, Vec<u8>)>,
        key: &str,
    ) -> AppResult<Value> {
        self.database.with_connection(|c| {
            let existing:Option<(String,u32)>=c.query_row("SELECT id,version FROM art_library_cases WHERE source_key=?1",[key],
                |r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((id,version))=existing { return read(c,&id,version); }
            let id=uuid::Uuid::new_v4().to_string();
            let mut entry=json!({"schema":"abya.art-case/v1","id":id,"version":1,"provenance":"imported",
                "createdAt":now(),"designApproved":false,"runtimeVerified":false,
                "reportedDesignApproved":original["designApproved"],"reportedRuntimeVerified":original["runtimeVerified"],
                "importedFrom":{"id":original["id"],"version":original["version"]}});
            for key in ["title","context","plannerRequirements","visualRules","decisions","production",
                "applicability","limitations","sources","satisfaction"] {
                entry[key]=json!(super::art_design::text(&original,key,8000)?);
            }
            entry["tags"]=json!(super::art_design::list(&original,"tags",20)?);
            let library=directory(c)?;
            let mut media=vec![];
            for (i,(m,bytes)) in payloads.iter().enumerate() {
                let mime=mutation::text(m,"mediaType")?;
                let ext=match mime {"image/png"=>"png","image/jpeg"=>"jpg","image/gif"=>"gif",
                    "image/webp"=>"webp","video/mp4"=>"mp4","video/webm"=>"webm",_=>return Err(AppError::validation("素材格式无效。"))};
                let path=format!("{id}/1/{i}.{ext}");
                files::write_file(&library,&path,bytes)?;
                media.push(json!({"path":path,"sha256":m["sha256"],"mediaType":mime,
                    "title":super::art_design::text(m,"title",200)?,"sourceType":m["sourceType"],"source":m["source"]}));
            }
            entry["media"]=json!(media);
            c.execute("INSERT INTO art_library_cases(id,version,status,body,source_key,created_at) VALUES(?1,1,'accepted',?2,?3,?4)",
                rusqlite::params![id,entry.to_string(),key,now()])?;
            read(c,&id,1)
        })
    }
}
