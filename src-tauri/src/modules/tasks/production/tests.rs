use super::*;
use crate::foundation::{AppPaths, Database};
use crate::modules::tasks::TaskInput;

struct Fixture {
    service: TaskService,
    id: String,
    root: PathBuf,
    workspace: PathBuf,
}

#[test]
fn drafts_remain_visible_while_scoped_blocker_prevents_stage_completion() {
    let f = Fixture::new();
    f.doc("requirements", "Confirmed rules");
    f.accept("requirements");
    files::write_file(&f.workspace, &format!("{ROOT}/plan.md"), b"Draft plan").unwrap();
    f.apply(
        "register-artifact",
        json!({"stage":"plan","path":format!("{ROOT}/plan.md"),"title":"Execution draft"}),
    )
    .unwrap();
    f.apply("save-issue",json!({"id":"recorder","stage":"plan","kind":"blocker","status":"open","description":"Wrong window","resumeWhen":"Record correct window","affectedStages":["plan"],"blockedOperations":["complete-stage","submit-document"]})).unwrap();
    let view = f
        .apply(
            "complete-stage",
            json!({"stage":"resources","summary":"Assets ready"}),
        )
        .unwrap();
    assert_eq!(view.stage_statuses["plan"], "blocked");
    assert_eq!(view.artifacts[0]["exists"], true);
    assert_eq!(view.record.unwrap().stages["requirements"], "passed");
    assert!(
        f.apply(
            "submit-document",
            json!({"kind":"plan","path":format!("{ROOT}/plan.md")})
        )
        .is_err()
    );
    assert!(
        f.apply(
            "register-artifact",
            json!({"stage":"plan","path":"../foreign.md","title":"Bad"})
        )
        .is_err()
    );
}

#[test]
fn stage_answers_do_not_reopen_confirmed_requirements() {
    let f = Fixture::new();
    f.apply(
        "configure",
        json!({"questionMode":"ask","playerMode":"single"}),
    )
    .unwrap();
    f.doc("requirements", "Rules");
    f.accept("requirements");
    f.apply(
        "complete-stage",
        json!({"stage":"resources","summary":"Ready"}),
    )
    .unwrap();
    f.apply("publish-questions",json!({"id":"plan-choice","stage":"plan","title":"Plan choice","provider":"codex","conversationId":uuid::Uuid::new_v4().to_string(),"questions":[{"id":"a","text":"Choose an approach"}]})).unwrap();
    f.service
        .production_answer(ProductionMutation {
            task_id: f.id.clone(),
            expected_revision: f.record().revision,
            operation: "submit-answers".into(),
            data: json!({"id":"plan-choice","answers":{"a":"First approach"}}),
        })
        .unwrap();
    assert_eq!(f.record().stages["requirements"], "passed");
    assert_eq!(f.record().current_stage, "plan");
    assert_eq!(f.record().question_groups[0].stage, "plan");
    assert_eq!(f.record().approvals.len(), 1);
}

#[test]
fn approved_template_backfill_preserves_document_and_cycle() {
    let f = Fixture::new();
    f.doc("requirements", "taskTemplate: none");
    f.accept("requirements");
    let before = f.record();
    f.apply("register-approved-template",json!({"key":"taskTemplate","templateId":"none","documentHash":before.documents["requirements"].sha256})).unwrap();
    f.apply("configure", json!({"taskTemplate":"none"}))
        .unwrap();
    let after = f.record();
    assert_eq!(before.cycle, after.cycle);
    assert_eq!(before.approvals.len(), after.approvals.len());
    assert_eq!(after.stages["requirements"], "passed");
    assert!(
        f.apply(
            "register-approved-template",
            json!({"key":"taskTemplate","templateId":"none","documentHash":"stale"})
        )
        .is_err()
    );
}

#[test]
fn template_choice_is_applied_only_when_document_is_accepted() {
    let f = Fixture::new();
    let path = format!("{ROOT}/requirements.md");
    files::write_file(&f.workspace, &path, b"taskTemplate: none").unwrap();
    f.apply(
        "submit-document",
        json!({"kind":"requirements","path":path,"taskTemplate":"none"}),
    )
    .unwrap();
    assert_eq!(f.record().task_template, None);
    f.accept("requirements");
    assert_eq!(f.record().task_template.as_deref(), Some("none"));
}

#[test]
fn legacy_records_get_additive_defaults_without_new_approvals() {
    let f = Fixture::new();
    let mut body = serde_json::to_value(f.record()).unwrap();
    for key in ["artifacts", "stageUpdates", "events"] {
        body.as_object_mut().unwrap().remove(key);
    }
    let r: ProductionRecord = serde_json::from_value(body).unwrap();
    assert!(r.artifacts.is_empty());
    assert!(r.approvals.is_empty());
    assert!(r.events.is_empty());
}

#[test]
fn corrective_document_can_be_submitted_but_required_issue_still_blocks_acceptance() {
    let f = Fixture::new();
    f.apply("save-issue",json!({"id":"clarify","stage":"requirements","kind":"checkpoint","status":"open","description":"Resolve before approval","affectedStages":["requirements"],"blockedOperations":["complete-stage"]})).unwrap();
    f.doc("requirements", "Corrective draft");
    let r = f.record();
    assert!(
        f.service
            .production_decide(ProductionDecision {
                task_id: f.id.clone(),
                expected_revision: r.revision,
                kind: "requirements".into(),
                document_hash: r.documents["requirements"].sha256.clone(),
                accepted: true,
                feedback: String::new()
            })
            .is_err()
    );
    assert!(f.record().approvals.is_empty());
}

#[test]
fn compatible_workflow_upgrade_retains_approved_documents() {
    let f = Fixture::new();
    f.doc("requirements", "Rules");
    f.accept("requirements");
    let original = f.record();
    f.service
        .production_commit(&f.id, original.revision, |r| {
            r.workflow_version = "1.2.1".into();
            r.policy["workflowVersion"] = json!("1.2.1");
            Ok(())
        })
        .unwrap();
    let result = f
        .service
        .production_upgrade(&f.id, f.record().revision)
        .unwrap()
        .record
        .unwrap();
    assert_eq!(result.stages["requirements"], "passed");
    assert_eq!(result.cycle, original.cycle);
    assert_eq!(
        result.documents["requirements"].sha256,
        original.documents["requirements"].sha256
    );
    assert_eq!(result.approvals.len(), original.approvals.len());
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
    f.apply(
        "configure",
        json!({"questionMode":"ask", "playerMode":"single"}),
    )
    .unwrap();
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
fn intake_answers_survive_reload_reject_agent_submission_and_invalidate_decisions() {
    let f = Fixture::new();
    assert_eq!(f.record().player_mode, "unspecified");
    f.apply(
        "configure",
        json!({"questionMode":"ask", "playerMode":"single"}),
    )
    .unwrap();
    let question = json!({"id":"rules","title":"需求梳理","provider":"codex",
        "conversationId":uuid::Uuid::new_v4().to_string(),"nativeSessionId":uuid::Uuid::new_v4().to_string(),
        "questions":[{"id":"rules","text":"如何计分？","options":["按时间","按步数"]}]});
    f.apply("publish-questions", question.clone()).unwrap();
    assert!(f.apply("publish-questions", question).is_err());
    assert!(validation::document_gate(&f.record(), &f.workspace, "requirements").is_err());
    let data = json!({"id":"rules","answers":{"rules":"按步数"}});
    assert!(f.apply("submit-answers", data.clone()).is_err());
    let change = |op: &str, data: Value| {
        f.service.production_answer(ProductionMutation {
            task_id: f.id.clone(),
            expected_revision: f.record().revision,
            operation: op.into(),
            data,
        })
    };
    change("save-draft", data.clone()).unwrap();
    assert_eq!(f.record().question_groups[0].draft["rules"], "按步数");
    let stale = f.record().revision;
    change("submit-answers", data.clone()).unwrap();
    assert!(
        f.service
            .production_answer(ProductionMutation {
                task_id: f.id.clone(),
                expected_revision: stale,
                operation: "submit-answers".into(),
                data: data.clone()
            })
            .is_err()
    );
    assert!(change("submit-answers", data).is_err());
    f.doc("requirements", "Confirmed scoring.");
    f.accept("requirements");
    change("amend", json!({"id":"rules"})).unwrap();
    assert_eq!(f.record().stages["requirements"], "in-progress");
    change(
        "submit-answers",
        json!({"id":"rules","answers":{"rules":"按时间"}}),
    )
    .unwrap();
    let record = f.record();
    assert_eq!(record.question_groups[0].answers.len(), 2);
    assert_eq!(
        record.question_groups[0].answers[0].answers["rules"],
        "按步数"
    );
    assert!(
        f.apply("configure", json!({"playerMode":"multiplayer"}))
            .is_err()
    );
    change("set-player-mode", json!({"playerMode":"multiplayer"})).unwrap();
    assert_eq!(f.record().player_mode, "multiplayer");
}

#[test]
fn intake_no_followup_unknown_fields_required_answers_and_legacy_defaults() {
    let f = Fixture::new();
    let mut legacy = serde_json::to_value(f.record()).unwrap();
    legacy.as_object_mut().unwrap().remove("playerMode");
    legacy.as_object_mut().unwrap().remove("questionGroups");
    let restored: ProductionRecord = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.player_mode, "unspecified");
    assert!(restored.question_groups.is_empty());
    assert!(f.apply("publish-questions", json!({})).is_err());
    assert!(
        f.apply("configure", json!({"playerMode":"online"}))
            .is_err()
    );
    f.apply("configure", json!({"questionMode":"ask"})).unwrap();
    assert!(validation::document_gate(&f.record(), &f.workspace, "requirements").is_err());
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
    f.apply(
        "configure",
        json!({"questionMode":"ask", "playerMode":"single"}),
    )
    .unwrap();
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
