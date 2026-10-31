use super::*;
fn ui(offset: i32, script: &str) -> Value {
    json!({"Version":1,"DocumentId":"document","Pages":[{"PageId":"page","Nodes":[{
        "Id":"node","Layout":{"Offset":{"x":offset,"y":0}},"Parameters":[{"N":"$B$event","V":script}]
    }]}]})
}
#[test]
fn layout_projection_preserves_bindings_and_rejects_unknown_schema() {
    let before = validation_impact::projection("UI/Documents/Main.PBUI", ui(0, "original-script"));
    let moved = validation_impact::projection("UI/Documents/Main.PBUI", ui(10, "original-script"));
    let rebound = validation_impact::projection("UI/Documents/Main.PBUI", ui(10, "changed-script"));
    assert_eq!(before, moved);
    assert_ne!(before, rebound);
    let mut unknown = ui(0, "original-script");
    unknown["Version"] = json!(2);
    assert_eq!(
        validation_impact::projection("Main.PBUI", unknown),
        Value::Null
    );
}
#[test]
fn only_parsed_edit_time_is_excluded_from_archive_content() {
    let first = json!({"Guid":"fa3ea760-50af-494f-a1a0-d7ec98f4a036","LastEditTime":"2026-10-30T00:00:00Z","GlobalBehaviors":["a"]});
    let mut timed = first.clone();
    timed["LastEditTime"] = json!("2026-10-30T01:00:00Z");
    assert_eq!(
        validation_impact::projection("Main.PBArc", first.clone()),
        validation_impact::projection("Main.PBArc", timed.clone())
    );
    timed["GlobalBehaviors"] = json!(["b"]);
    assert_ne!(
        validation_impact::projection("Main.PBArc", first),
        validation_impact::projection("Main.PBArc", timed)
    );
}
#[test]
fn layout_only_does_not_repeat_core_loop_but_deleted_ui_still_requires_full_recheck() {
    let old = json!({"files":[{"path":"UI/Main.PBUI","sha256":"raw-a","kind":"unknown","nonLayoutSha256":"bindings-a"}]});
    let new = json!({"files":[{"path":"UI/Main.PBUI","sha256":"raw-b","kind":"unknown","nonLayoutSha256":"bindings-a"}]});
    let result = validation_impact::difference(&old, &new, false);
    assert_eq!(result["unknownImpact"], false);
    assert_eq!(
        result["mustRetest"],
        json!(["visual", "input", "save-reload"])
    );
    assert_eq!(result["changedFiles"][0]["sha256"], "raw-b");
    assert_eq!(
        validation_impact::difference(&old, &json!({"files":[]}), false)["unknownImpact"],
        true
    );
    assert_eq!(
        validation_impact::difference(&old, &new, true)["unknownImpact"],
        true
    );
}
