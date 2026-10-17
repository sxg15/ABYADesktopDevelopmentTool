use super::art_design::{list, text};
use super::art_design_sources::{design_hash, hash};
use super::*;
use base64::Engine as _;

pub(super) fn directory(c: &rusqlite::Connection) -> AppResult<PathBuf> {
    let root = Path::new(
        c.path()
            .ok_or_else(|| AppError::validation("案例库需要持久数据库。"))?,
    )
    .parent()
    .ok_or_else(|| AppError::validation("案例库路径不可用。"))?
    .to_path_buf();
    let path = files::safe_path(&root, "ArtLibrary")?;
    std::fs::create_dir_all(&path)?;
    Ok(path)
}
pub(super) fn read(c: &rusqlite::Connection, id: &str, version: u32) -> AppResult<Value> {
    let row: Option<(String, String)> = c
        .query_row(
            "SELECT body,status FROM art_library_cases WHERE id=?1 AND version=?2",
            rusqlite::params![id, version],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (body, status) = row.ok_or_else(|| AppError::validation("案例版本不存在。"))?;
    let mut entry: Value = serde_json::from_str(&body)?;
    entry["status"] = json!(status);
    Ok(entry)
}
pub(super) fn media_bytes(root: &Path, item: &Value) -> AppResult<Vec<u8>> {
    let path = files::safe_path(root, mutation::text(item, "path")?)?;
    let (sha, size) = files::hash_file(&path)?;
    if item["sha256"] != sha || size > 64 * 1024 * 1024 || size == 0 {
        return Err(AppError::validation("案例素材缺失、过大或已被修改。"));
    }
    let bytes = std::fs::read(path)?;
    if !super::art_library_package::valid_media(mutation::text(item, "mediaType")?, &bytes) {
        return Err(AppError::validation("案例素材不是声明的图片或视频格式。"));
    }
    Ok(bytes)
}

impl TaskService {
    pub fn art_library_action(&self, input: ProductionMutation) -> AppResult<Value> {
        match input.operation.as_str() {
            "request" => self.request_art_case_work(input),
            "collect" => self.collect_art_case(input),
            "withdraw" => self.database.with_connection(|c| {
                let id=text(&input.data,"id",100)?;
                let version=input.data["version"].as_u64().and_then(|v|u32::try_from(v).ok()).ok_or_else(||AppError::validation("缺少案例版本。"))?;
                text(&input.data,"reason",2000)?;
                let mut entry=read(c,&id,version)?;
                entry["withdrawReason"]=input.data["reason"].clone();
                c.execute("UPDATE art_library_cases SET status='withdrawn',body=?3 WHERE id=?1 AND version=?2",
                    rusqlite::params![id,version,entry.to_string()])?;
                read(c,&id,version)
            }),
            "export" => self.art_library_export(&input.task_id,&input.data),
            "import" => self.art_library_import(&input.data),
            _ => Err(AppError::validation("无效案例库操作。")),
        }
    }

    fn collect_art_case(&self, input: ProductionMutation) -> AppResult<Value> {
        self.database.with_connection(|c| {
            let task=read_task(c,&input.task_id)?;
            let mut r=load(c,&input.task_id)?.ok_or_else(||AppError::validation("缺少制作记录。"))?;
            if r.revision != input.expected_revision { return Err(conflict()); }
            super::art_design_sources::refresh(&mut r,&task);
            let draft=r.art_design.case_drafts.iter().find(|d|d["id"] == input.data["draftId"])
                .ok_or_else(||AppError::validation("请先由 AI 整理可审阅的案例草稿。"))?;
            if draft["designHash"] != design_hash(&r) || draft["cycle"] != r.cycle {
                return Err(AppError::validation("案例草稿已过期，请按最新方案重新整理。"));
            }
            let satisfaction=text(&input.data,"satisfaction",4000)?;
            let key=hash(&json!([input.task_id,draft,satisfaction]));
            let existing:Option<(String,u32)>=c.query_row("SELECT id,version FROM art_library_cases WHERE source_key=?1",[&key],
                |row|Ok((row.get(0)?,row.get(1)?))).optional()?;
            if let Some((id,version))=existing { return read(c,&id,version); }
            let (id,version)=if let Some(parent)=draft["parentCaseId"].as_str() {
                let latest:u32=c.query_row("SELECT COALESCE(MAX(version),0) FROM art_library_cases WHERE id=?1",[parent],|row|row.get(0))?;
                if latest == 0 || draft["parentVersion"].as_u64() != Some(latest as u64) { return Err(conflict()); }
                (parent.to_owned(),latest+1)
            } else { (uuid::Uuid::new_v4().to_string(),1) };
            let library=directory(c)?;
            let workspace=Path::new(&task.workspace_path);
            let mut ids=list(draft,"artifactIds",16)?;
            super::art_design_validation::design_bindings(&r,workspace,&ids)?;
            for reference in draft["referenceIds"].as_array().into_iter().flatten() {
                if !r.artifacts.iter().any(|a|a["id"]==*reference && a["sourceType"]=="reference" && a["status"]!="withdrawn") {
                    return Err(AppError::validation("案例引用的原始参考已撤回或不可用。"));
                }
                ids.push(reference.clone());
            }
            let mut media=vec![];
            let mut total=0u64;
            let mut runtime_count=0;
            for (i,aid) in ids.iter().enumerate() {
                let a=r.artifacts.iter().find(|a|a["id"] == *aid).ok_or_else(||AppError::validation("成果不存在。"))?;
                let bytes=media_bytes(workspace,a)?; total+=bytes.len() as u64;
                if total > 128*1024*1024 { return Err(AppError::validation("单个案例素材上限128 MiB。")); }
                let extension=Path::new(mutation::text(a,"path")?).extension().and_then(|x|x.to_str()).unwrap_or("png");
                let path=format!("{id}/{version}/{i}.{extension}");
                files::write_file(&library,&path,&bytes)?;
                media.push(json!({"path":path,"sha256":a["sha256"],"mediaType":a["mediaType"],
                    "title":a["title"],"sourceType":a["sourceType"],"source":a["source"]}));
                if a["sourceType"] == "gameplay" && r.evidence.iter().any(|e|e.path == a["path"] && e.reviewed
                    && e.cycle == r.cycle && Some(&e.version) == r.current_version.as_ref() && e.sha256 == a["sha256"]) { runtime_count+=1; }
            }
            let mut entry=draft.clone();
            let runtime_total=media.iter().filter(|m|m["sourceType"] == "gameplay").count();
            let visual_test=r.self_tests.iter().rev().find(|t|t["id"] == "visual"
                && t["version"] == json!(r.current_version) && t["cycle"] == r.cycle);
            let runtime_verified=runtime_count>0 && runtime_count==runtime_total && visual_test.is_some_and(|t|
                t["status"] == "passed" && validation::version(&r).is_ok_and(|v|
                    validation::evidence_refs(&r,workspace,&t["evidenceIds"],v).is_ok()))
                && versions::verify(&r.version_details).is_ok();
            for key in ["artifactIds","referenceIds","parentCaseId","parentVersion"] { entry.as_object_mut().unwrap().remove(key); }
            entry["schema"]=json!("abya.art-case/v1"); entry["id"]=json!(id); entry["version"]=json!(version);
            entry["media"]=json!(media); entry["satisfaction"]=json!(satisfaction); entry["designApproved"]=json!(true);
            entry["runtimeVerified"]=json!(runtime_verified); entry["runtimeVersion"]=json!(r.current_version);
            entry["sourceTaskId"]=json!(input.task_id); entry["sourceDraftId"]=draft["id"].clone();
            entry["createdAt"]=json!(now()); entry["provenance"]=json!("local-planner");
            c.execute("INSERT INTO art_library_cases(id,version,status,body,source_key,created_at) VALUES(?1,?2,'accepted',?3,?4,?5)",
                rusqlite::params![id,version,entry.to_string(),key,now()])?;
            read(c,&id,version)
        })
    }
    pub fn art_library_list(&self, query: &str, history: bool) -> AppResult<Value> {
        if query.chars().count() > 200 {
            return Err(AppError::validation("检索条件最多200字。"));
        }
        self.database.with_connection(|c| {
            let mut statement = c.prepare("SELECT body,status FROM art_library_cases a WHERE
                (?1 OR (a.status='accepted' AND a.version=(SELECT MAX(b.version) FROM art_library_cases b WHERE b.id=a.id)))
                ORDER BY created_at DESC LIMIT 1000")?;
            let rows = statement.query_map([history],|r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;
            let tokens: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
            let mut found = vec![];
            for row in rows {
                let (body,status)=row?;
                let mut entry: Value = serde_json::from_str(&body)?;
                let haystack = format!("{} {} {} {}",entry["title"],entry["context"],entry["tags"],entry["applicability"]).to_lowercase();
                let score = tokens.iter().filter(|t| haystack.contains(t.as_str())).count();
                if !tokens.is_empty() && score == 0 { continue; }
                entry["status"] = json!(status);
                found.push((score,entry));
            }
            found.sort_by_key(|a| std::cmp::Reverse(a.0));
            Ok(json!(found.into_iter().take(50).map(|(_,v)|v).collect::<Vec<_>>()))
        })
    }
    pub fn art_library_get(&self, id: &str, version: u32) -> AppResult<Value> {
        self.database.with_connection(|c| read(c, id, version))
    }
    pub(super) fn art_library_use(&self, input: ProductionMutation) -> AppResult<ProductionView> {
        let id = text(&input.data, "caseId", 100)?;
        let version = input.data["version"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| AppError::validation("请选择案例版本。"))?;
        let inspecting = input.operation == "inspect-art-case";
        let decision = |key| {
            if inspecting {
                Ok(String::new())
            } else {
                text(&input.data, key, 4000)
            }
        };
        let borrow = decision("borrow")?;
        let adapt = decision("adapt")?;
        let avoid = decision("avoid")?;
        let root = self.production_workspace(&input.task_id)?;
        // Resolve and verify a fixed snapshot before the task transaction. No library locks are nested.
        let (entry, media) = self.database.with_connection(|c| {
            let entry = read(c, &id, version)?;
            if entry["status"] != "accepted" {
                return Err(AppError::validation("已撤回案例不能新增引用。"));
            }
            let library = directory(c)?;
            let media = entry["media"]
                .as_array()
                .ok_or_else(|| AppError::validation("案例素材无效。"))?
                .iter()
                .map(|m| Ok((m.clone(), media_bytes(&library, m)?)))
                .collect::<AppResult<Vec<_>>>()?;
            Ok((entry, media))
        })?;
        self.production_commit(&input.task_id,input.expected_revision,|r| {
            if !super::art_design_sources::enabled(r) { return Err(AppError::validation("请先升级美术流程。")); }
            let before=design_hash(r);
            let mut references=vec![];
            for (i,(m,bytes)) in media.iter().enumerate() {
                let ext=Path::new(mutation::text(m,"path")?).extension().and_then(|x|x.to_str()).unwrap_or("png");
                let path=format!("artifacts/art-library/{id}/{version}/{i}.{ext}");
                let full=files::safe_path(&root,&path)?;
                if full.exists() {
                    if files::hash_file(&full)?.0 != m["sha256"] { return Err(AppError::validation("本任务案例快照已修改，未覆盖。")); }
                } else { files::write_file(&root,&path,bytes)?; }
                super::visuals::register(r,&root,&json!({"stage":"resources","path":path,
                    "title":m["title"],"sourceType":"reference","roles":["feedback"],
                    "groupId":path,"visualVersion":version.to_string(),"source":format!("art-case:{id}@{version}")}))?;
                if let Some(a)=r.artifacts.iter_mut().find(|a|a["path"]==path) { a["libraryReference"]=json!(true); }
                references.push(json!(path));
            }
            if !inspecting { super::art_design::put(&mut r.art_design.case_uses,json!({"id":format!("{id}@{version}"),
                "caseId":id,"version":version,"caseHash":hash(&entry),"title":entry["title"],
                "borrow":borrow,"adapt":adapt,"avoid":avoid,"paths":references,"snapshot":entry}))?; }
            if before != design_hash(r) { super::art_design_sources::invalidate(r); }
            Ok(())
        })?;
        self.production_export(&input.task_id)?;
        self.production_get(&input.task_id)
    }
    pub fn art_library_media(&self, id: &str, version: u32, index: usize) -> AppResult<String> {
        self.database.with_connection(|c| {
            let entry = read(c, id, version)?;
            let item = entry["media"]
                .get(index)
                .ok_or_else(|| AppError::validation("案例图片不存在。"))?;
            let bytes = media_bytes(&directory(c)?, item)?;
            Ok(format!(
                "data:{};base64,{}",
                mutation::text(item, "mediaType")?,
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ))
        })
    }
}
