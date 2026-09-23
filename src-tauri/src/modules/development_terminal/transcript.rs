use crate::foundation::AppResult;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const TRANSCRIPT_EVENT_BYTES: usize = 64 * 1024;
const TRANSCRIPT_REPLAY_MAX_BYTES: u64 = 256 * 1024;

pub(crate) fn read_transcript_replay(path: &Path) -> AppResult<Vec<String>> {
    read_transcript_replay_with_limit(path, TRANSCRIPT_REPLAY_MAX_BYTES)
}

fn read_transcript_replay_with_limit(path: &Path, max_bytes: u64) -> AppResult<Vec<String>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let start = file_len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))?;

    // Take a stable snapshot. A live PTY may append after metadata() while an
    // existing conversation is being attached to a new UI subscriber.
    let mut bytes = Vec::with_capacity((file_len - start) as usize);
    file.take(file_len - start).read_to_end(&mut bytes)?;

    let mut omitted_bytes = start;
    if start > 0
        && let Some(line_end) = bytes.iter().position(|byte| *byte == b'\n')
    {
        bytes.drain(..=line_end);
        omitted_bytes += line_end as u64 + 1;
    }

    let mut events = Vec::new();
    if omitted_bytes > 0 {
        events.push(format!(
            "\r\n[Earlier terminal output omitted ({omitted_bytes} bytes); showing recent output.]\r\n"
        ));
    }
    append_utf8_chunks(&String::from_utf8_lossy(&bytes), &mut events);
    Ok(events)
}

fn append_utf8_chunks(value: &str, output: &mut Vec<String>) {
    let mut start = 0;
    while start < value.len() {
        let mut end = (start + TRANSCRIPT_EVENT_BYTES).min(value.len());
        while end > start && !value.is_char_boundary(end) {
            end -= 1;
        }
        output.push(value[start..end].to_string());
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn replay_keeps_only_a_bounded_recent_tail() {
        let path =
            std::env::temp_dir().join(format!("abya-terminal-transcript-{}.log", Uuid::new_v4()));
        fs::write(&path, "discard this line\nrecent output").unwrap();

        let events = read_transcript_replay_with_limit(&path, 20).unwrap();

        assert!(events[0].contains("Earlier terminal output omitted"));
        assert_eq!(events[1..].concat(), "recent output");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn replay_chunks_without_splitting_utf8_characters() {
        let path =
            std::env::temp_dir().join(format!("abya-terminal-transcript-{}.log", Uuid::new_v4()));
        let content = "界".repeat(TRANSCRIPT_EVENT_BYTES);
        fs::write(&path, &content).unwrap();

        let events = read_transcript_replay_with_limit(&path, content.len() as u64).unwrap();

        assert_eq!(events.concat(), content);
        let _ = fs::remove_file(path);
    }
}
