use super::*;

fn seed(f: &Fixture) {
    f.service.production_commit(&f.id, f.record().revision, |r| {
        r.current_version = Some("candidate-v3".into());
        r.version_details = json!({"playerHash":"build-sha","playerPath":"D:/Player/ABYA.exe"});
        r.issues = vec![
            json!({"id":"source","title":"输入失效","stage":"implementation","kind":"blocker","status":"open","repairTarget":"abya-source","description":"中文复现\n```\n<script>unsafe()</script>","resumeWhen":"修复后提供构建包复验","evidenceIds":["log","missing"]}),
            json!({"id":"build","stage":"plan","kind":"defect","status":"resolved","repairTarget":"abya-build","description":"构建包缺文件","recheck":"新版已通过"}),
            json!({"id":"legacy","stage":"implementation","kind":"blocker","status":"open","description":"ABYA 底层异常，需要程序排查"}),
            json!({"id":"lua","stage":"implementation","kind":"defect","status":"open","repairTarget":"lua","description":"普通脚本问题"}),
        ];
        r.evidence.push(Evidence{stage:"implementation".into(),cycle:r.cycle,id:"log".into(),path:"artifacts/missing.log".into(),sha256:"recorded-hash".into(),bytes:42,kind:"log".into(),capture_type:"runtime".into(),version:"candidate-v3".into(),reviewed:false,description:"错误日志".into(),instance_id:None,recorded_at:now()});
        Ok(())
    }).unwrap();
}

#[test]
fn programmer_export_preserves_snapshot_and_evidence_without_mutation() {
    let f = Fixture::new();
    seed(&f);
    let before = f.record();
    let ids = vec!["source".into(), "legacy".into(), "source".into()];
    let relative = f
        .service
        .export_programmer_issues(&f.id, before.revision, &ids)
        .unwrap();
    assert!(relative.starts_with(&format!("{ROOT}/reports/programmer-issues-")));
    assert!(relative.ends_with(".md"));
    let text = std::fs::read_to_string(f.workspace.join(&relative)).unwrap();
    for expected in [
        "Production test",
        "candidate-v3",
        "build-sha",
        "输入失效",
        "修复后提供构建包复验",
        "recorded-hash",
        "文件缺失",
        "证据记录缺失",
        "疑似 ABYA",
        "问题数：2",
        "````\n中文复现\n```\n<script>unsafe()</script>\n````",
    ] {
        assert!(text.contains(expected), "Missing {expected}");
    }
    assert!(!text.contains("普通脚本问题"));
    assert!(!text.contains("构建包缺文件"));
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(f.record()).unwrap()
    );
    let second = f
        .service
        .export_programmer_issues(&f.id, before.revision, &ids)
        .unwrap();
    assert_ne!(relative, second);
    assert_eq!(
        text,
        std::fs::read_to_string(f.workspace.join(relative)).unwrap()
    );
}

#[test]
fn programmer_export_rejects_stale_missing_ordinary_and_empty_selection() {
    let f = Fixture::new();
    seed(&f);
    let revision = f.record().revision;
    assert!(
        f.service
            .export_programmer_issues(&f.id, revision - 1, &["source".into()])
            .is_err()
    );
    for ids in [
        vec![],
        vec!["unknown".into()],
        vec!["lua".into()],
        vec!["source".into(), "../../escape".into()],
    ] {
        assert!(
            f.service
                .export_programmer_issues(&f.id, revision, &ids)
                .is_err()
        );
    }
    assert!(
        !std::fs::read_dir(f.workspace.join(ROOT).join("reports"))
            .unwrap()
            .any(|p| p.unwrap().path().extension().is_some_and(|ext| ext == "md"))
    );
}

#[test]
fn programmer_export_allows_archived_task_and_resolved_issue_without_reopening() {
    let f = Fixture::new();
    seed(&f);
    f.service.set_status(&f.id, TaskStatus::Archived).unwrap();
    let before = f.record();
    let relative = f
        .service
        .export_programmer_issues(&f.id, before.revision, &["build".into()])
        .unwrap();
    let text = std::fs::read_to_string(f.workspace.join(relative)).unwrap();
    assert!(text.contains("resolved") && text.contains("新版已通过"));
    assert_eq!(
        f.service.get_readonly(&f.id).unwrap().status,
        TaskStatus::Archived
    );
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(f.record()).unwrap()
    );
}
