use crate::foundation::AppResult;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const TRANSCRIPT_EVENT_BYTES: usize = 64 * 1024;
const TRANSCRIPT_REPLAY_MAX_BYTES: u64 = 256 * 1024;

// 复制读取完整磁盘快照，不经过终端回放和前端历史窗口的截断。
pub(crate) fn read_transcript_text(path: &Path) -> AppResult<String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.into()),
    };
    let length = file.metadata()?.len();
    let mut reader = file.take(length);
    let mut buffer = [0_u8; TRANSCRIPT_EVENT_BYTES];
    let mut text = TranscriptText::default();
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        text.push(&buffer[..count]);
    }
    Ok(String::from_utf8_lossy(&text.output).into_owned())
}

#[derive(Default)]
enum EscapeState {
    #[default]
    Text,
    Escape,
    Intermediate,
    Csi,
    String,
    StringEscape,
}

#[derive(Default)]
struct TranscriptText {
    state: EscapeState,
    carriage_return: bool,
    output: Vec<u8>,
}

impl TranscriptText {
    // 跨读取块保留 ANSI 状态，避免颜色、标题和超链接控制串混入复制文本。
    fn push(&mut self, bytes: &[u8]) {
        use EscapeState::*;
        for &byte in bytes {
            match self.state {
                Text => match byte {
                    0x1b => self.state = Escape,
                    b'\r' => {
                        self.output.push(b'\n');
                        self.carriage_return = true;
                    }
                    b'\n' => {
                        if !self.carriage_return {
                            self.output.push(b'\n');
                        }
                        self.carriage_return = false;
                    }
                    b'\t' | 0x20..=0x7e | 0x80..=0xff => {
                        self.output.push(byte);
                        self.carriage_return = false;
                    }
                    _ => {}
                },
                Escape => {
                    self.state = match byte {
                        b'[' => Csi,
                        b']' | b'P' | b'X' | b'^' | b'_' => String,
                        0x20..=0x2f => Intermediate,
                        0x1b => Escape,
                        _ => Text,
                    }
                }
                Intermediate => {
                    if (0x30..=0x7e).contains(&byte) {
                        self.state = Text;
                    }
                }
                Csi => {
                    if (0x40..=0x7e).contains(&byte) {
                        self.state = Text;
                    }
                }
                String => match byte {
                    0x07 => self.state = Text,
                    0x1b => self.state = StringEscape,
                    _ => {}
                },
                StringEscape => {
                    self.state = match byte {
                        b'\\' | 0x07 => Text,
                        0x1b => StringEscape,
                        _ => String,
                    }
                }
            }
        }
    }
}

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
    // Historical device queries/mode changes must never drive the new PTY handshake.
    // The live TUI paints its current screen; replay is an output-only text snapshot.
    let mut text = TranscriptText::default();
    text.push(&bytes);
    let plain = String::from_utf8_lossy(&text.output).replace('\n', "\r\n");
    append_utf8_chunks(&plain, &mut events);
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

    #[cfg(windows)]
    #[test]
    fn embedded_pty_starts_without_a_frontend_cursor_reply() {
        use portable_pty::{CommandBuilder, PtySize, native_pty_system};
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut writer = pair.master.take_writer().unwrap();
        let mut command = CommandBuilder::new("cmd.exe");
        command.args(["/d", "/c", "echo ABYA_PTY_READY"]);
        let mut child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                if send.send(buffer[..count].to_vec()).is_err() {
                    break;
                }
            }
        });
        let started = std::time::Instant::now();
        let mut output = String::new();
        while started.elapsed() < std::time::Duration::from_secs(5)
            && !output.contains("ABYA_PTY_READY")
        {
            match receive.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(bytes) => output.push_str(&String::from_utf8_lossy(&bytes)),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(_) => {}
            }
        }
        // Release a regressed cursor wait before cleaning up and reporting the failed assertion.
        if !output.contains("ABYA_PTY_READY") {
            use std::io::Write;
            let _ = writer.write_all(b"\x1b[1;1R");
        }
        let _ = child.kill();
        let _ = child.wait();
        drop(writer);
        drop(pair.master);
        assert!(
            output.contains("ABYA_PTY_READY"),
            "PTY stalled before normal output"
        );
        assert!(
            !output.contains("\x1b[6n"),
            "Unexpected parent cursor query"
        );
    }

    #[test]
    fn full_copy_keeps_history_beyond_replay_and_ui_limits() {
        let path = std::env::temp_dir().join(format!("abya-copy-{}.log", Uuid::new_v4()));
        let text = format!(
            "最早的问题\r\n{}\r\n最后的回答🙂",
            "中间记录\n".repeat(200_000)
        );
        fs::write(&path, &text).unwrap();
        assert_eq!(
            read_transcript_text(&path).unwrap(),
            text.replace("\r\n", "\n")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn full_copy_handles_empty_and_missing_transcripts() {
        let path = std::env::temp_dir().join(format!("abya-copy-{}.log", Uuid::new_v4()));
        assert_eq!(read_transcript_text(&path).unwrap(), "");
        fs::write(&path, "").unwrap();
        assert_eq!(read_transcript_text(&path).unwrap(), "");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn full_copy_strips_control_sequences_across_arbitrary_chunks() {
        let input = "\x1b]0;window title\x07\x1b[32m你好🙂\x1b[0m\r\n\x1b]8;;https://example.com\x1b\\链接\x1b]8;;\x1b\\\r下一行\t完成\x1b(B\x1bPignored\x1b\\\x00\x07\x1b[";
        for size in 1..=input.len() {
            let mut text = TranscriptText::default();
            for chunk in input.as_bytes().chunks(size) {
                text.push(chunk);
            }
            assert_eq!(
                String::from_utf8(text.output).unwrap(),
                "你好🙂\n链接\n下一行\t完成"
            );
        }
    }

    #[test]
    fn replay_cannot_send_old_queries_or_change_live_terminal_modes() {
        let path = std::env::temp_dir().join(format!("abya-replay-{}.log", Uuid::new_v4()));
        fs::write(
            &path,
            "\x1b[?2004l\x1b[?1004lold\r\n\x1b[6n\x1b[c\x1b]11;?\x07\x1bP$qm\x1b\\done",
        )
        .unwrap();
        let events = read_transcript_replay(&path).unwrap();
        assert_eq!(events.concat(), "old\r\ndone");
        assert!(!events.concat().contains('\x1b'));
        fs::remove_file(path).unwrap();
    }

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
