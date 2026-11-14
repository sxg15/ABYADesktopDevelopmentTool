use crate::foundation::AppResult;
use serde_json::{Value, json};
use std::path::Path;

// Unloaded RPC projections can infer "interrupted" while another client is still
// writing. Observe the explicit native events, preserving incomplete tail lines.
pub(super) fn observe_native_turn(path: &Path, runtime: &mut Value) -> AppResult<()> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let end = f.metadata()?.len();
    let offset = runtime["nativeOffset"]
        .as_u64()
        .filter(|n| *n <= end)
        .unwrap_or(0);
    let start = offset.max(end.saturating_sub(4 * 1024 * 1024));
    f.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    f.take(4 * 1024 * 1024).read_to_end(&mut bytes)?;
    let Some(last) = bytes.iter().rposition(|b| *b == b'\n') else {
        return Ok(());
    };
    for (index, line) in bytes[..=last].split(|b| *b == b'\n').enumerate() {
        if start > offset && index == 0 {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        if v["type"] != "event_msg" {
            continue;
        }
        let p = &v["payload"];
        let Some(turn) = p["turn_id"].as_str() else {
            continue;
        };
        let state = match p["type"].as_str() {
            Some("task_started" | "item_started" | "item_completed") => "inProgress",
            Some("task_complete" | "task_completed") => "completed",
            Some("turn_aborted") => "interrupted",
            Some("task_failed") => "failed",
            _ => continue,
        };
        runtime["nativeTurn"] = json!(turn);
        runtime["nativeStatus"] = json!(state);
    }
    runtime["nativeOffset"] = json!(start + last as u64 + 1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observes_live_and_completed_turns_across_partial_writes() {
        let path =
            std::env::temp_dir().join(format!("abya-mode-event-{}.jsonl", uuid::Uuid::new_v4()));
        let begin = json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"first"}});
        let finish =
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"first"}})
                .to_string();
        std::fs::write(&path, format!("{begin}\n{}", &finish[..8])).unwrap();
        let mut state = json!({});
        observe_native_turn(&path, &mut state).unwrap();
        assert_eq!(state["nativeStatus"], "inProgress");
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(f, "{}", &finish[8..]).unwrap();
        observe_native_turn(&path, &mut state).unwrap();
        assert_eq!(state["nativeStatus"], "completed");
        let same = state.clone();
        observe_native_turn(&path, &mut state).unwrap();
        assert_eq!(state, same);
        drop(f);
        std::fs::remove_file(path).unwrap();
    }
}
