use super::{
    GameInstance, memory::MemoryGuard, memory_models::MemoryPolicy, memory_probe::MemorySample,
};
use serde_json::json;

fn fixture() -> (std::path::PathBuf, MemoryGuard, GameInstance) {
    let root = std::env::temp_dir().join(format!("abya-memory-test-{}", uuid::Uuid::new_v4()));
    let guard = MemoryGuard::new(root.clone());
    let instance:GameInstance=serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"taskId":"task",
        "origin":"managed","name":"隔离内存测试","sanitizedArgs":[],"pid":std::process::id(),
        "processState":"running","connectionState":"disconnected","failureReason":"","gameVersion":"fixture",
        "platform":"Windows","runtimeCapabilities":[]})).unwrap();
    guard.start(&instance).unwrap();
    (root, guard, instance)
}
fn sample(mib: u64) -> MemorySample {
    MemorySample {
        at: chrono::Utc::now().to_rfc3339(),
        private_bytes: mib * 1024 * 1024,
        working_set_bytes: 256 * 1024 * 1024,
        available_bytes: 16 * 1024 * 1024 * 1024,
        commit_bytes: 8 * 1024 * 1024 * 1024,
        commit_limit_bytes: 32 * 1024 * 1024 * 1024,
    }
}
#[test]
fn records_episode_and_request_before_protection_without_implicit_kill() {
    let (root, guard, instance) = fixture();
    guard.event(
        &instance.id,
        json!({"requestId":"request-1","tool":"ui_batch_edit","status":"started"}),
    );
    assert!(!guard.sample(&instance.id, sample(5000)));
    let first = guard.status(&instance.id);
    assert!(first.pressured);
    assert!(!first.auto_stop);
    let report = guard.report(&instance.id).unwrap();
    assert!(report.is_file());
    let before =
        std::fs::read_to_string(report.parent().unwrap().join("before-trigger.json")).unwrap();
    assert!(before.contains("request-1"));
    assert!(before.contains("ui_batch_edit"));
    guard.sample(&instance.id, sample(5100));
    assert_eq!(first.incident_id, guard.status(&instance.id).incident_id);
    guard.arm(&instance.id, true).unwrap();
    assert!(guard.sample(&instance.id, sample(5200)));
    for _ in 0..10 {
        guard.sample(&instance.id, sample(1000));
    }
    assert!(!guard.status(&instance.id).pressured);
    guard.sample(&instance.id, sample(5200));
    assert_ne!(first.incident_id, guard.status(&instance.id).incident_id);
    guard.finish(&instance.id, json!({"exitCode":0}), false);
    assert!(
        root.join(guard.status(&instance.id).incident_id.unwrap())
            .join("outcome.json")
            .is_file()
    );
}
#[test]
fn survives_restart_with_interrupted_monitoring_and_validates_policy() {
    let (root, guard, instance) = fixture();
    let policy = MemoryPolicy {
        private_limit_mib: 2048,
        ..Default::default()
    };
    guard.set_policy(policy).unwrap();
    guard.sample(&instance.id, sample(1000));
    assert!(
        guard
            .set_policy(MemoryPolicy {
                private_limit_mib: 0,
                ..Default::default()
            })
            .is_err()
    );
    let recovered = MemoryGuard::new(root.clone());
    assert_eq!(recovered.policy().private_limit_mib, 2048);
    let metadata: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("active").join(format!("{}.json", instance.id))).unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["ended"], true);
    let id = metadata["recoveredIncidentId"].as_str().unwrap();
    let report = std::fs::read_to_string(root.join(id).join("report.md")).unwrap();
    assert!(report.contains("不能据此认定 OOM"));
    let count = std::fs::read_dir(&root).unwrap().count();
    let _ = MemoryGuard::new(root.clone());
    assert_eq!(count, std::fs::read_dir(root).unwrap().count());
}
#[test]
fn record_failure_is_visible_while_samples_still_trigger_protection() {
    let root = std::env::temp_dir().join(format!("abya-memory-file-{}", uuid::Uuid::new_v4()));
    std::fs::write(&root, b"not a directory").unwrap();
    let (_, _, instance) = fixture();
    let guard = MemoryGuard::new(root);
    assert!(guard.start(&instance).is_err());
    guard.sample(&instance.id, sample(5000));
    assert!(guard.status(&instance.id).pressured);
    assert!(!guard.status(&instance.id).error.is_empty());
}
#[test]
fn samples_real_owned_process_and_distinguishes_commit_from_residency() {
    let probe = super::memory_probe::MemoryProbe::new(std::process::id()).unwrap();
    let current = probe.sample().unwrap();
    assert!(current.private_bytes > 0);
    assert!(current.working_set_bytes > 0);
    assert!(current.commit_limit_bytes >= current.commit_bytes);
}

#[test]
fn cold_start_growth_is_not_misclassified_and_later_growth_is_detected() {
    let (_, guard, instance) = fixture();
    guard.sample(&instance.id, sample(50));
    for _ in 0..119 {
        guard.sample(&instance.id, sample(2000));
        assert!(!guard.status(&instance.id).pressured);
    }
    guard.sample(&instance.id, sample(3000));
    assert!(guard.status(&instance.id).pressured);
}

#[test]
fn incident_summaries_do_not_copy_credentials_or_raw_shell_assignments() {
    let secret = "$env:OPENAI_API_KEY=sk-MEMORY_SECRET_SENTINEL";
    assert!(!super::diagnostic_text(secret).contains("SENTINEL"));
    assert!(!crate::modules::development_terminal::diagnostic_summary(secret).contains("SENTINEL"));
    assert_eq!(
        super::diagnostic_text("custom_ui_authoring_get_tree"),
        "custom_ui_authoring_get_tree"
    );
}

#[test]
fn failed_stop_does_not_claim_the_process_has_exited() {
    let (root, guard, instance) = fixture();
    guard.sample(&instance.id, sample(5000));
    guard.record_stop_outcome(&instance.id, json!({"stopError":"timeout"}));
    let identity: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("active").join(format!("{}.json", instance.id))).unwrap(),
    )
    .unwrap();
    assert_eq!(identity["ended"], false);
}
