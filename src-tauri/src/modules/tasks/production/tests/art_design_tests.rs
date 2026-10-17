use super::*;
use base64::Engine as _;
pub(super) const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=";

#[test]
fn embedded_design_catalog_reads_pinned_resources_without_new_pins() {
    let f = Fixture::new();
    let before = serde_json::to_value(f.record()).unwrap();
    for provider in ["codex", "grok"] {
        let prefix = format!(".{provider}/skills/abya-game-development-task");
        files::write_file(
            &f.workspace,
            &format!("{prefix}/SKILL.md"),
            b"Pinned workflow",
        )
        .unwrap();
        for reference in [
            "references/game-art-design",
            "references/game-art-design/references/product-interface-design",
        ] {
            files::write_file(
                &f.workspace,
                &format!("{prefix}/{reference}/SKILL.md"),
                b"---\ndescription: Pinned design method\n---\nRead these original constraints.",
            )
            .unwrap();
        }
    }
    let catalog = f.service.production_catalog(&f.id).unwrap();
    let designs: Vec<_> = catalog
        .iter()
        .filter(|entry| entry.kind == "design")
        .collect();
    assert_eq!(designs.len(), 4);
    for entry in designs {
        assert_eq!(entry.sha256.len(), 64);
        assert!(
            f.service
                .production_skill_text(&f.id, &entry.provider, &entry.id)
                .unwrap()
                .contains("Read these original constraints.")
        );
    }
    assert!(
        catalog::scan(&f.workspace)
            .unwrap()
            .iter()
            .all(|entry| entry.kind != "design")
    );
    assert_eq!(serde_json::to_value(f.record()).unwrap(), before);
    assert!(
        f.service
            .production_skill_text(&f.id, "codex", "../../outside")
            .is_err()
    );
}

pub(super) fn enabled_fixture() -> Fixture {
    let f = Fixture::new();
    f.service
        .update(
            &f.id,
            TaskInput {
                title: "Game art".into(),
                description: "不要用数字显示库存，直接显示剩余物品。保留玩家身份颜色。".into(),
            },
        )
        .unwrap();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy = policy();
            r.workflow_version = "2.2.0".into();
            Ok(())
        })
        .unwrap();
    f.doc("requirements", "Inventory and identity");
    f.accept("requirements");
    f
}
pub(super) fn register_image(f: &Fixture, name: &str, source: &str) -> String {
    let path = format!("artifacts/{name}.png");
    files::write_file(
        &f.workspace,
        &path,
        &base64::engine::general_purpose::STANDARD
            .decode(PNG)
            .unwrap(),
    )
    .unwrap();
    f.apply(
        "register-visual",
        json!({"stage":"resources","path":path,"title":name,"sourceType":source,
        "roles":["asset-board","layout","states"],"groupId":name,"visualVersion":"1"}),
    )
    .unwrap();
    f.record().artifacts.last().unwrap()["id"]
        .as_str()
        .unwrap()
        .into()
}
pub(super) fn prepare(f: &Fixture) -> String {
    let source = f.record().art_design.sources[0].clone();
    for (id, quote) in [
        ("ART-1", "不要用数字显示库存，直接显示剩余物品。"),
        ("ART-2", "保留玩家身份颜色。"),
    ] {
        f.apply("save-art-requirement",json!({"id":id,"sourceId":source["id"],"sourceHash":source["hash"],
            "quote":quote,"kind":"must","interpretation":quote,"scope":"HUD","acceptance":"Inspect the HUD in gameplay"})).unwrap();
    }
    f.apply(
        "review-art-source",
        json!({"sourceId":source["id"],"sourceHash":source["hash"],
        "requirementIds":["ART-1","ART-2"],"reason":"Both original instructions are itemized."}),
    )
    .unwrap();
    direction(f);
    let id = register_image(f, "layout", "mockup");
    for q in ["ART-1", "ART-2"] {
        f.apply(
            "save-art-coverage",
            json!({"requirementId":q,"artifactIds":[id],
        "designLocation":"Inventory objects and stable player markers","status":"addressed"}),
        )
        .unwrap();
    }
    id
}
pub(super) fn direction(f: &Fixture) {
    f.apply("save-art-direction",json!({"playerExperience":"Read inventory and player identity while aiming",
        "tasksAndInformation":"Aim, fire, inspect remaining objects","platformAndCamera":"Desktop fixed camera",
        "visualRules":"Stable identity colors, object silhouettes","representativeScene":"Mid-round HUD and last item",
        "assetProduction":"Reuse existing sprites with separate text and input","feasibility":"Fixture only, no real runtime claim",
        "acceptance":"Inspect object count and stable identities","librarySearch":"No matching fixture cases"})).unwrap();
}
pub(super) fn submit_art(f: &Fixture) -> AppResult<ProductionView> {
    files::write_file(
        &f.workspace,
        "artifacts/art.md",
        b"new art: object inventory",
    )
    .unwrap();
    f.apply("submit-document",json!({"kind":"resources","path":"artifacts/art.md","artScope":"new-art","artScopeReason":"new art"}))
}

#[test]
fn missing_requirement_blocks_submission_and_new_input_invalidates_approval() {
    let f = enabled_fixture();
    prepare(&f);
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.art_design.coverage.pop();
            Ok(())
        })
        .unwrap();
    assert!(submit_art(&f).unwrap_err().to_string().contains("ART-2"));
    let image = f.record().artifacts[0]["id"].clone();
    f.apply("save-art-coverage",json!({"requirementId":"ART-2","artifactIds":[image],"designLocation":"Stable marker","status":"addressed"})).unwrap();
    submit_art(&f).unwrap();
    f.accept("resources");
    let approvals = f.record().approvals.len();
    human(
        &f,
        "add-art-note",
        json!({"text":"最后一枚物品也必须可见。"}),
    )
    .unwrap();
    let r = f.record();
    assert_eq!(r.stages["resources"], "in-progress");
    assert_eq!(r.approvals.len(), approvals);
    assert_eq!(r.art_design.requirements.len(), 2);
    assert!(submit_art(&f).is_err());
    assert!(validation::approved(&r, &f.workspace, "resources").is_err());
}

#[test]
fn agent_cannot_rewrite_or_downgrade_original_requirement_or_fake_a_source() {
    let f = enabled_fixture();
    prepare(&f);
    let original = f.record().art_design.requirements[0].clone();
    let mut changed = original.clone();
    changed["kind"] = json!("preference");
    assert!(f.apply("save-art-requirement", changed).is_err());
    let mut changed = original.clone();
    changed["quote"] = json!("忽略库存");
    assert!(f.apply("save-art-requirement", changed).is_err());
    assert!(
        f.apply(
            "withdraw-art-requirement",
            json!({"id":"ART-1","reason":"skip"})
        )
        .is_err()
    );
    human(
        &f,
        "withdraw-art-requirement",
        json!({"id":"ART-1","reason":"策划明确撤回"}),
    )
    .unwrap();
    assert_eq!(f.record().art_design.requirements[0]["withdrawn"], true);
    assert!(f.apply("save-art-requirement", original).is_err());
}

#[test]
fn references_require_analysis_and_cannot_satisfy_design_or_coverage() {
    let f = enabled_fixture();
    prepare(&f);
    human(
        &f,
        "attach-art-reference",
        json!({"name":"Colors","base64":PNG,"summary":"只参考配色，不参考布局"}),
    )
    .unwrap();
    let r = f.record();
    let reference = r.artifacts.last().unwrap()["id"].clone();
    let source = r.art_design.sources.last().unwrap();
    f.apply("review-art-source",json!({"sourceId":source["id"],"sourceHash":source["hash"],"requirementIds":[],"reason":"Visual input, analyzed separately"})).unwrap();
    direction(&f);
    assert!(submit_art(&f).unwrap_err().to_string().contains("参考图"));
    assert!(f.apply("save-art-coverage",json!({"requirementId":"ART-1","artifactIds":[reference],"designLocation":"reference","status":"addressed"})).is_err());
    f.apply("analyze-art-reference",json!({"artifactId":reference,"role":"overall","observations":"Limited color palette",
        "borrow":"Colors only","avoid":"Original layout","application":"Player identity accents","conflicts":"No conflict"})).unwrap();
    assert!(submit_art(&f).is_err()); // Old coverage must be checked against the new source set.
    let image = r.artifacts[0]["id"].clone();
    for q in ["ART-1", "ART-2"] {
        f.apply("save-art-coverage",json!({"requirementId":q,"artifactIds":[image],"designLocation":"Checked unchanged HUD","status":"addressed"})).unwrap();
    }
    submit_art(&f).unwrap();
    let root = f
        .workspace
        .join(r.artifacts.last().unwrap()["path"].as_str().unwrap());
    std::fs::write(root, "tampered").unwrap();
    assert!(
        f.service
            .production_decide(ProductionDecision {
                task_id: f.id.clone(),
                expected_revision: f.record().revision,
                kind: "resources".into(),
                document_hash: f.record().documents["resources"].sha256.clone(),
                accepted: true,
                feedback: String::new()
            })
            .is_err()
    );
}

#[test]
fn task_edit_is_projected_without_writes_and_cannot_approve_a_stale_document() {
    let f = enabled_fixture();
    prepare(&f);
    submit_art(&f).unwrap();
    let old = f.record();
    f.service
        .update(
            &f.id,
            TaskInput {
                title: "Game art".into(),
                description: "新的策划要求：只用图形。".into(),
            },
        )
        .unwrap();
    let projected = f.record();
    assert_eq!(projected.revision, old.revision);
    assert_ne!(projected.art_design.sources, old.art_design.sources);
    assert_eq!(projected.stages["resources"], "in-progress");
    assert!(validation::approved(&projected, &f.workspace, "resources").is_err());
    f.service.production_export(&f.id).unwrap();
    let exported = std::fs::read_to_string(f.workspace.join(ROOT).join("workflow.json")).unwrap();
    assert!(exported.contains("新的策划要求：只用图形。"));
}

#[test]
fn realization_requires_visual_evidence_for_current_game_and_design() {
    let f = enabled_fixture();
    prepare(&f);
    submit_art(&f).unwrap();
    f.accept("resources");
    f.doc("plan", "Plan");
    f.accept("plan");
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.current_version = Some("game-1".into());
            Ok(())
        })
        .unwrap();
    files::write_file(
        &f.workspace,
        "artifacts/runtime.png",
        &base64::engine::general_purpose::STANDARD
            .decode(PNG)
            .unwrap(),
    )
    .unwrap();
    f.apply("register-evidence",json!({"id":"screen","path":"artifacts/runtime.png","kind":"image","captureType":"inspection","reviewed":true,"description":"Fixture screenshot only"})).unwrap();
    let data = json!({"requirementId":"ART-1","target":"HUD/Inventory","evidenceIds":["screen"],
        "comparison":"Compare object count at same size","differences":"None in fixture","status":"matched"});
    f.apply("save-art-realization", data.clone()).unwrap();
    assert!(
        art_design_validation::gate(&f.record(), &f.workspace, "implementation", None).is_err()
    );
    let mut other = data;
    other["requirementId"] = json!("ART-2");
    f.apply("save-art-realization", other).unwrap();
    art_design_validation::gate(&f.record(), &f.workspace, "implementation", None).unwrap();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.current_version = Some("game-2".into());
            Ok(())
        })
        .unwrap();
    assert!(
        art_design_validation::gate(&f.record(), &f.workspace, "implementation", None).is_err()
    );
}

#[test]
fn legacy_tasks_reject_new_mutations_until_explicit_upgrade() {
    let f = Fixture::new();
    let original = f.record();
    assert!(f.apply("save-art-direction", json!({})).is_err());
    assert_eq!(f.record().revision, original.revision);
    f.service
        .production_upgrade(&f.id, original.revision)
        .unwrap();
    let current = f.record();
    assert_eq!(current.workflow_version, "2.2.0");
    assert!(!current.art_design.sources.is_empty());
    assert!(current.art_design.requirements.is_empty());
}

#[test]
fn conversation_quotes_are_append_only_and_gameplay_feedback_does_not_reopen_art() {
    let f = enabled_fixture();
    prepare(&f);
    submit_art(&f).unwrap();
    f.accept("resources");
    f.service.production_commit(&f.id,f.record().revision,|r|{
        r.feedback.push(json!({"id":"gameplay","stage":"review","description":"碰撞规则错误","status":"open","history":[]}));Ok(())
    }).unwrap();
    assert_eq!(f.record().stages["resources"], "passed");
    let note = json!({"id":"message-1","locator":"conversation A / user message 1","text":"暂停时保留库存显示。"});
    f.apply("record-art-input", note.clone()).unwrap();
    assert_eq!(f.record().stages["resources"], "in-progress");
    assert_eq!(
        f.record().art_design.sources.last().unwrap()["kind"],
        "agent-transcribed-conversation"
    );
    let mut changed = note;
    changed["text"] = json!("改写原话");
    assert!(f.apply("record-art-input", changed).is_err());
}

#[test]
fn nested_design_skill_deploys_to_long_workspace_paths() {
    let f = Fixture::new();
    let workspace = f
        .root
        .join("较长的美术项目工作目录-with-spaces-and-platform-context")
        .join("nested-case-workspaces");
    let service = TaskService::new(f.service.database.clone(), workspace);
    let task = service
        .create(TaskInput {
            title: "美术需求设计与实机还原的较长任务名称".into(),
            description: "Long path fixture".into(),
        })
        .unwrap();
    let deep=Path::new(&task.workspace_path).join(".codex/skills/abya-game-development-task/references/game-art-design/references/product-interface-design/references/workflow-integration.md");
    assert!(deep.is_file());
    assert!(std::fs::read_to_string(deep).unwrap().contains("接入已有"));
}
