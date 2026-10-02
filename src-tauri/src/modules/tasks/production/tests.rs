use super::*;
use crate::foundation::{AppPaths, Database};
use crate::modules::tasks::TaskInput;

struct Fixture {
    service: TaskService,
    id: String,
    root: PathBuf,
    workspace: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("abya-production-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let mut service = TaskService::new(
            Database::open(&paths).unwrap(),
            paths.default_workspace_root,
        );
        service.managed_skill_sources.clear();
        let task = service
            .create(TaskInput {
                title: "Production test".into(),
                description: "Synthetic contract fixtures; no real game acceptance".into(),
            })
            .unwrap();
        let f = Self {
            service,
            id: task.id,
            root,
            workspace: task.workspace_path.into(),
        };
        f.apply("initialize", json!({"questionMode":"no-followup"}))
            .unwrap();
        f
    }
    fn record(&self) -> ProductionRecord {
        self.service
            .production_get(&self.id)
            .unwrap()
            .record
            .unwrap()
    }
    fn apply(&self, operation: &str, data: Value) -> AppResult<ProductionView> {
        let revision = self
            .service
            .production_get(&self.id)?
            .record
            .map_or(0, |r| r.revision);
        self.service.production_update(ProductionMutation {
            task_id: self.id.clone(),
            expected_revision: revision,
            operation: operation.into(),
            data,
        })
    }
    fn doc(&self, kind: &str, content: &str) {
        let path = format!("{ROOT}/{kind}.md");
        files::write_file(&self.workspace, &path, content.as_bytes()).unwrap();
        self.apply("submit-document", json!({"kind":kind,"path":path}))
            .unwrap();
    }
    fn accept(&self, kind: &str) {
        let r = self.record();
        self.service
            .production_decide(ProductionDecision {
                task_id: self.id.clone(),
                expected_revision: r.revision,
                kind: kind.into(),
                document_hash: r.documents[kind].sha256.clone(),
                accepted: true,
                feedback: String::new(),
            })
            .unwrap();
    }
    fn plan(&self) {
        self.doc("requirements", "# Requirements\nA complete test cycle.");
        self.accept("requirements");
        self.apply(
            "complete-stage",
            json!({"stage":"resources","summary":"Existing fixture assets"}),
        )
        .unwrap();
        self.doc("plan", "# Plan\nImplement and check all required paths.");
        self.accept("plan");
        let archive = self.workspace.join("fixture-archive");
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(archive.join("Main.PBArc"), "fixture archive").unwrap();
        let player = self.workspace.join("fixture-player.exe");
        std::fs::write(&player, "fixture player").unwrap();
        self.service
            .production_capture_version(
                &self.id,
                self.record().revision,
                "v1",
                VersionSources {
                    archive_path: archive,
                    player_path: player,
                    archive_guid: "archive".into(),
                    level_guid: "level".into(),
                    instance_id: "fixture".into(),
                },
            )
            .unwrap();
    }
    fn evidence(&self, id: &str, kind: &str, capture: &str) {
        let extension = match kind {
            "image" => "png",
            "video" => "mp4",
            _ => "json",
        };
        let path = format!("artifacts/evidence/{id}.{extension}");
        files::write_file(
            &self.workspace,
            &path,
            b"synthetic checker fixture, not runtime evidence",
        )
        .unwrap();
        self.apply("register-evidence", json!({"id":id,"kind":kind,"captureType":capture,"path":path,"reviewed":true,"description":"Synthetic material"})).unwrap();
    }
    fn initial(&self) {
        self.plan();
        self.evidence("initial-video", "video", "full-cycle");
        self.evidence("initial-image", "image", "inspection");
        self.evidence("initial-state", "json", "state");
        self.apply(
            "set-milestone",
            json!({"name":"R0","evidenceIds":["initial-video","initial-image","initial-state"]}),
        )
        .unwrap();
        let checks: Vec<_> = ["architecture", "lua", "save-reload", "runtime"]
            .iter()
            .map(|id| json!({"id":id,"status":"passed","evidenceIds":["initial-state"]}))
            .collect();
        self.apply(
            "complete-stage",
            json!({"stage":"implementation","checks":checks}),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SQLite may still hold Windows handles. Files remain in a task-specific temp root on failure.
        if self.root.starts_with(std::env::temp_dir()) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn approval_requires_current_file_revision_and_cannot_be_forged_by_mutation() {
    let f = Fixture::new();
    f.doc("requirements", "Original");
    let old = f.record();
    assert!(f.apply("approve", json!({"accepted":true})).is_err());
    std::fs::write(
        f.workspace.join(&old.documents["requirements"].path),
        "Changed",
    )
    .unwrap();
    assert!(
        f.service
            .production_decide(ProductionDecision {
                task_id: f.id.clone(),
                expected_revision: old.revision,
                kind: "requirements".into(),
                document_hash: old.documents["requirements"].sha256.clone(),
                accepted: true,
                feedback: String::new()
            })
            .is_err()
    );
    assert!(
        f.apply(
            "complete-stage",
            json!({"stage":"resources","summary":"ready"})
        )
        .is_err()
    );
    assert!(f.service.set_status(&f.id, TaskStatus::Completed).is_err());
}

#[test]
fn revision_conflict_retains_newer_state_and_export_cannot_override_database() {
    let f = Fixture::new();
    let old = f.record();
    f.apply("configure", json!({"questionMode":"ask"})).unwrap();
    assert!(
        f.service
            .production_update(ProductionMutation {
                task_id: f.id.clone(),
                expected_revision: old.revision,
                operation: "configure".into(),
                data: json!({"questionMode":"no-followup"})
            })
            .is_err()
    );
    std::fs::write(
        f.workspace.join(ROOT).join("workflow.json"),
        "{\"approved\":true}",
    )
    .unwrap();
    assert_eq!(f.record().question_mode, "ask");
    assert!(f.record().approvals.is_empty());
}

#[test]
fn complete_pilot_requires_all_rounds_questions_evidence_and_user_acceptance() {
    let f = Fixture::new();
    f.initial();
    assert!(f.apply("save-round", json!({"number":2})).is_err());
    for number in 1..=12 {
        f.apply("save-round", json!({"number":number})).unwrap();
        assert!(
            f.apply(
                "save-round",
                json!({"number":number,"close":true,"checks":[]})
            )
            .is_err()
        );
        let id = format!("run-{number}");
        f.evidence(&id, "json", "state");
        if [8, 12].contains(&number) {
            let video = format!("video-{number}");
            let image = format!("image-{number}");
            f.evidence(&video, "video", "full-cycle");
            f.evidence(&image, "image", "inspection");
            f.apply(
                "set-milestone",
                json!({"name":format!("R{number}"),"evidenceIds":[video,image,id]}),
            )
            .unwrap();
        }
        let p = policy();
        let checks: Vec<_> = p["dimensions"].as_array().unwrap().iter().map(|d|
            json!({"dimensionId":d["id"],"status":"passed","observations":"fixture assertion","evidenceIds":[id]})).collect();
        let questions: Vec<_> = if number >= 9 {
            p["questions"].as_array().unwrap().iter().map(|q|
            json!({"questionId":q["id"],"status":"passed","conclusion":"fixture","counterexample":"fixture","problem":"none in fixture","change":"none","recheck":"fixture","evidenceIds":[id]})).collect()
        } else {
            vec![]
        };
        f.apply("save-round", json!({"number":number,"close":true,"checks":checks,"questions":questions,"evidenceIds":[id]})).unwrap();
    }
    assert!(f.service.set_status(&f.id, TaskStatus::Completed).is_err());
    f.doc("delivery", "# Delivery\nSynthetic v1 only.");
    f.accept("delivery");
    assert!(f.service.set_status(&f.id, TaskStatus::Completed).is_err());
    f.doc(
        "closeout",
        "# Closeout\nNo reusable gameplay knowledge from fixtures.",
    );
    f.apply(
        "save-knowledge",
        json!({"entries":[{"status":"none","summary":"Synthetic tests only"}]}),
    )
    .unwrap();
    f.apply("complete-stage", json!({"stage":"closeout"}))
        .unwrap();
    assert!(f.service.set_status(&f.id, TaskStatus::Completed).is_ok());
    let html =
        std::fs::read_to_string(f.workspace.join(ROOT).join("reports/review-report.html")).unwrap();
    assert!(html.contains("R12"));
}

#[test]
fn changed_requirements_preserve_history_but_invalidate_downstream_approval_and_evidence() {
    let f = Fixture::new();
    f.initial();
    let old = f.record();
    f.doc("requirements", "Changed requirements.");
    f.accept("requirements");
    assert!(f.record().cycle > old.cycle);
    assert!(f.apply("set-version", json!({"id":"v2","archiveGuid":"archive","levelGuid":"level","archiveHash":"c".repeat(64),"playerBuildHash":"b".repeat(64)})).is_err());
    assert!(!f.record().approvals.is_empty());
    assert!(!f.record().evidence.is_empty());
    assert!(
        validation::evidence_refs(&f.record(), &f.workspace, &json!(["initial-state"]), "v1")
            .is_err()
    );
}

#[test]
fn edited_evidence_and_paths_outside_task_are_rejected() {
    let f = Fixture::new();
    f.plan();
    f.evidence("state", "json", "state");
    let record = f.record();
    std::fs::write(f.workspace.join(&record.evidence[0].path), "different").unwrap();
    assert!(validation::evidence_refs(&record, &f.workspace, &json!(["state"]), "v1").is_err());
    for path in [
        "../foreign.json",
        "C:/private.json",
        "artifacts/file.json:stream",
    ] {
        assert!(files::safe_path(&f.workspace, path).is_err());
    }
    assert!(f.apply("save-issue", json!({"id":"problem","kind":"defect","status":"resolved","stage":"review","description":"bad"})).is_err());
}

#[test]
fn report_escapes_document_content() {
    let f = Fixture::new();
    f.doc("requirements", "<script>alert('x')</script>");
    let html = report::render(&f.record(), "plan", &[]);
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
}

#[test]
fn active_production_read_does_not_refresh_pinned_skills() {
    let f = Fixture::new();
    let path = f.workspace.join(".codex/skills/abya-game-development-task");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("SKILL.md"), "pinned").unwrap();
    f.service.get(&f.id).unwrap();
    assert_eq!(
        std::fs::read_to_string(path.join("SKILL.md")).unwrap(),
        "pinned"
    );
    let catalog = f.service.production_catalog(&f.id).unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].sha256.len(), 64);
}

#[test]
fn actual_saved_archive_changes_invalidate_version() {
    let f = Fixture::new();
    f.initial();
    versions::verify(&f.record().version_details).unwrap();
    std::fs::write(
        f.workspace.join("fixture-archive/Main.PBArc"),
        "new saved behavior",
    )
    .unwrap();
    assert!(versions::verify(&f.record().version_details).is_err());
}

#[test]
fn changing_pending_choices_requires_document_resubmission() {
    let f = Fixture::new();
    f.doc("requirements", "No follow-up questions.");
    f.apply("configure", json!({"questionMode":"ask"})).unwrap();
    let record = f.record();
    assert_eq!(record.stages["requirements"], "in-progress");
    assert!(
        f.service
            .production_decide(ProductionDecision {
                task_id: f.id.clone(),
                expected_revision: record.revision,
                kind: "requirements".into(),
                document_hash: record.documents["requirements"].sha256.clone(),
                accepted: true,
                feedback: String::new()
            })
            .is_err()
    );
    f.doc("requirements", "Questions are now allowed.");
    f.accept("requirements");
    assert_eq!(f.record().stages["requirements"], "passed");
}
