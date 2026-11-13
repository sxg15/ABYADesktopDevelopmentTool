use super::*;
use std::path::Path;

fn select_control(view: &Value, kind: &str, object: &str) -> AppResult<String> {
    let controls = view["interactables"]
        .as_array()
        .ok_or_else(|| AppError::validation("Player 未提供可操作控件列表。"))?;
    let mut matches = controls.iter().filter(|c| {
        c["viewId"] == view["viewId"]
            && c["type"] == kind
            && c["visible"] == true
            && c["interactable"] == true
            && (c["name"] == object
                || c["path"]
                    .as_str()
                    .is_some_and(|path| path.split('/').any(|part| part == object)))
    });
    let id = matches.next().and_then(|c| c["componentId"].as_str());
    if matches.next().is_some() {
        return Err(AppError::validation(format!(
            "当前创建界面没有唯一的 {object}。"
        )));
    }
    id.map(str::to_owned)
        .ok_or_else(|| AppError::validation(format!("当前界面缺少可操作的 {object}。")))
}

impl RuntimeBridgeService {
    fn archive_ui_call(
        &self,
        id: &str,
        tool: &str,
        args: Value,
        session: &str,
        cancel: Arc<AtomicBool>,
    ) -> AppResult<Value> {
        let response = self.run(
            id,
            &["capability", "run", tool],
            Some(args),
            session,
            &uuid::Uuid::new_v4().to_string(),
            Duration::from_secs(35),
            cancel,
        )?;
        if response["success"] != true {
            return Err(AppError::new(
                "native_archive_ui_failed",
                format!("原生创建界面的 {tool} 未成功：{}", response["data"]),
                "inspect creation receipt before retrying",
            ));
        }
        Ok(response["data"].clone())
    }

    fn archive_view(
        &self,
        id: &str,
        kind: &str,
        session: &str,
        cancel: &Arc<AtomicBool>,
    ) -> AppResult<Value> {
        let start = Instant::now();
        let mut visible = String::new();
        while start.elapsed() < Duration::from_secs(30) {
            let state =
                self.archive_ui_call(id, "ui_get_state", json!({}), session, cancel.clone())?;
            if let Some(view) = state["activeViewId"].as_str() {
                let snapshot = self.archive_ui_call(
                    id,
                    "ui_get_view_snapshot",
                    json!({"viewId":view}),
                    session,
                    cancel.clone(),
                )?;
                visible = snapshot["type"].as_str().unwrap_or_default().to_owned();
                if visible == kind && snapshot["viewId"] == view {
                    return Ok(snapshot);
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Err(AppError::new(
            "native_archive_view_missing",
            format!("未进入 {kind}，最后观察到的界面是 {visible}。"),
            "",
        ))
    }

    pub(super) fn open_archive_form(
        &self,
        id: &str,
        session: &str,
        cancel: &Arc<AtomicBool>,
    ) -> AppResult<Value> {
        let menu = self.archive_view(id, "MineMenu", session, cancel)?;
        let start = select_control(&menu, "Button", "StartCreatingButton")?;
        self.archive_ui_call(
            id,
            "ui_click_button",
            json!({"componentId":start}),
            session,
            cancel.clone(),
        )?;
        self.archive_view(id, "CreatArchiveBox", session, cancel)
    }
}

// A task-private Player has to demonstrate use of its requested data root
// before any creation click. Never use the system-wide Archives directory.
pub(super) struct RootWitness {
    root: PathBuf,
    before: Vec<Option<(u64, std::time::SystemTime)>>,
}
const WITNESSES: [&str; 4] = [
    "Settings.PBConf",
    "APA/AgentRuntime/apa-agent-runtime.sqlite3",
    "APA/AgentRuntime/apa-agent-runtime.sqlite3-wal",
    "APA/AgentRuntime/apa-agent-runtime.sqlite3-shm",
];
impl RootWitness {
    pub(super) fn capture(root: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        let before = Self::stamps(&root)?;
        Ok(Self { root, before })
    }
    fn stamps(root: &Path) -> AppResult<Vec<Option<(u64, std::time::SystemTime)>>> {
        WITNESSES
            .iter()
            .map(|relative| {
                let p = root.join(relative);
                if !p.exists() {
                    return Ok(None);
                }
                let actual = p.canonicalize()?;
                if !actual.starts_with(root) || !actual.is_file() {
                    return Err(AppError::validation(
                        "Player 数据目录中出现了无效启动文件。",
                    ));
                }
                let meta = std::fs::metadata(actual)?;
                Ok(Some((meta.len(), meta.modified()?)))
            })
            .collect()
    }
    fn verify(&self) -> AppResult<()> {
        let archives = self.root.join("Archives");
        if archives.canonicalize()? != archives
            || !Self::stamps(&self.root)?
                .iter()
                .zip(&self.before)
                .any(|(now, before)| now.is_some() && now != before)
        {
            return Err(AppError::validation(
                "未观察到本次 Player 在私有数据根初始化，未点击创建存档。",
            ));
        }
        Ok(())
    }
}

pub(super) struct ArchiveFormInput<'a> {
    pub name: &'a str,
    pub author: &'a str,
    pub description: &'a str,
}

impl RuntimeBridgeService {
    pub(super) fn create_from_native_form(
        &self,
        id: &str,
        scope: &RootWitness,
        fields: ArchiveFormInput<'_>,
        session: &str,
        cancel: &Arc<AtomicBool>,
        before_submit: impl FnOnce() -> AppResult<()>,
    ) -> AppResult<Value> {
        let form = self.open_archive_form(id, session, cancel)?;
        scope.verify()?;
        let name = select_control(&form, "InputField", "ArchiveNameInputField")?;
        let author = select_control(&form, "InputField", "ArchiveAuthorInputField")?;
        let intro = select_control(&form, "InputField", "ArchiveDescriptionInputField")?;
        for (control, text) in [
            (&name, fields.name),
            (&author, fields.author),
            (&intro, fields.description),
        ] {
            self.archive_ui_call(id,"ui_set_input_text",json!({"componentId":control,"text":text,"mode":"replace","notifyValueChanged":true,"commit":"endEdit"}),session,cancel.clone())?;
        }
        let ready = self.archive_view(id, "CreatArchiveBox", session, cancel)?;
        if ready["viewId"] != form["viewId"] {
            return Err(AppError::validation("创建窗口已变化，请重新核对。"));
        }
        let submit = select_control(&ready, "Button", "CreatButton")?;
        for (control, wanted) in [
            (&name, fields.name),
            (&author, fields.author),
            (&intro, fields.description),
        ] {
            let actual = ready["interactables"]
                .as_array()
                .and_then(|items| items.iter().find(|i| i["componentId"] == control.as_str()));
            if actual.and_then(|i| i["text"].as_str()).unwrap_or_default() != wanted {
                return Err(AppError::validation(
                    "创建表单未回读出本次输入，未点击创建。",
                ));
            }
        }
        if scope.root.join("Archives").join(fields.name).exists() {
            return Err(AppError::validation("目标存档目录已存在，未点击创建。"));
        }
        let existing =
            crate::modules::game_archives::discover_archives(&scope.root.join("Archives"))?;
        if existing
            .iter()
            .any(|a| a.archive_name.eq_ignore_ascii_case(fields.name))
        {
            return Err(AppError::validation(
                "私有存档目录已经存在同名项目，未点击创建。",
            ));
        }
        let old_ids: Vec<_> = existing.into_iter().map(|a| a.archive_guid).collect();
        before_submit()?;
        self.archive_ui_call(
            id,
            "ui_click_button",
            json!({"componentId":submit}),
            session,
            cancel.clone(),
        )?;
        // Some builds create the native files and return to the menu. Observe
        // the new identity on disk, then let the caller launch its start level.
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(45) {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::validation("建档已派发；取消后请先核对实际文件。"));
            }
            if let Ok(found) =
                crate::modules::game_archives::discover_archives(&scope.root.join("Archives"))
            {
                let mut matches = found.iter().filter(|a| {
                    a.archive_name == fields.name && !old_ids.contains(&a.archive_guid)
                });
                if let Some(archive) = matches.next() {
                    if matches.next().is_some() {
                        return Err(AppError::validation("新建存档身份不唯一。"));
                    }
                    if let Some(level) = archive
                        .levels
                        .iter()
                        .find(|l| l.is_start)
                        .or(archive.levels.first())
                    {
                        let value = json!({"guid":archive.archive_guid,"name":archive.archive_name,"path":archive.archive_path,"currentLevelGuid":level.level_guid,"currentLevelName":level.level_name});
                        archive_creation::checked_archive(&scope.root, &value)?;
                        return Ok(json!({"route":"native-creator-ui","archive":value}));
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Err(AppError::new(
            "native_archive_load_timeout",
            "已点击创建，私有目录中尚未读到唯一的新存档及其初始关卡。",
            "inspect data and receipt; do not replay creation",
        ))
    }
}

pub(super) fn ui_archive_selection(
    created: &Value,
) -> AppResult<crate::modules::instances::ArchiveSelection> {
    let a = &created["archive"];
    let field = |key: &str| {
        a[key]
            .as_str()
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| AppError::validation(format!("原生新建结果缺少 {key}")))
    };
    Ok(crate::modules::instances::ArchiveSelection {
        archive_path: field("path")?,
        archive_guid: field("guid")?,
        archive_name: field("name")?,
        level_guid: field("currentLevelGuid")?,
        level_name: a["currentLevelName"].as_str().unwrap_or_default().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_must_be_unique_visible_and_owned_by_the_active_form() {
        let c = json!({"componentId":"chosen","viewId":"active","type":"InputField","name":"InputField","path":"Main/ArchiveNameInputField/InputField","visible":true,"interactable":true});
        let mut other = c.clone();
        other["viewId"] = json!("other");
        let mut form = json!({"viewId":"active","interactables":[other,c]});
        assert_eq!(
            select_control(&form, "InputField", "ArchiveNameInputField").unwrap(),
            "chosen"
        );
        assert!(select_control(&form, "Button", "ArchiveNameInputField").is_err());
        form["interactables"]
            .as_array_mut()
            .unwrap()
            .push(c.clone());
        assert!(select_control(&form, "InputField", "ArchiveNameInputField").is_err());
        form["interactables"] = json!([c]);
        form["interactables"][0]["interactable"] = json!(false);
        assert!(select_control(&form, "InputField", "ArchiveNameInputField").is_err());
    }
}
