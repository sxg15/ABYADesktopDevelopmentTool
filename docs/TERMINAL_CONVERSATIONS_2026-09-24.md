# Terminal paste and Codex conversation organization — 2026-09-24

## Changes

- Ctrl+V and Ctrl+Shift+V are intercepted in the embedded terminal. The Desktop
  reads Windows clipboard formats only on the explicit paste gesture. Plain text
  is preferred, including mixed HTML/text formats. Text uses xterm bracketed
  paste, removes terminal control sequences, and never adds an Enter key.
- The native image-paste shortcut is forwarded only for an actual image format.
  Ctrl+Shift+V is text-only. Context-menu paste follows the same routing.
- Text is bounded to 1 MiB and serialized into Unicode-safe IPC chunks below
  the existing backend input limit. Disposed terminals discard pending reads.
- Codex projects are registered/reused through installed 0.156.1 experimental
  project/list/create/update APIs, with stable task idempotency keys and metadata.
  A task remains a separate working directory. Only locally recorded native
  conversation IDs are assigned to the task project; no Codex database editing.
- Native conversation names are synchronized as ABYA · task · conversation.
  Redundant metadata writes are avoided. User-created matching-root projects
  may be reused without overwriting their names.
- Codex conversation rows have archive/restore controls and an archived filter.
  Native archival interrupts the owned active turn when necessary, stops its
  terminal, unsubscribes and archives. Local state is committed only after success.
  Restore preserves the original native ID and history. Local deletion first
  archives native history. Open/rename/archive/delete operations are serialized.
- Opening an archived conversation is rejected until explicit restoration.
  Refocusing the app or manually refreshing reconciles native archive/restore.
  Local records remain visible if native synchronization fails.

## Compatibility finding

The installed schema includes native project APIs even though these are
experimental and not all appear in the general app-server reference. They were
verified with a real app-server before integration.

The current Codex desktop sidebar's saved-project list did not expose projects
created by the CLI app-server, even though native project assignment succeeded.
The app therefore includes a one-time read-only task-folder guide for adding a
saved project in Codex. Existing visible historical threads were placed in the
separate ABYA 开发 sidebar section through the Codex app's supported tools.
This section is distinct from native project registration, and does not claim
to automatically enroll future threads in the desktop sidebar.

Empty native threads can be omitted from thread/list, including its archived
filter and database-only mode. Archive state therefore uses thread/read's explicit
archived value when available, otherwise the verified sessions/archived_sessions
layout in its current path field. This field is marked unstable by the schema:
unknown layouts fail closed rather than silently resuming an archived thread.

## Validation

- npm run check passed: 28 frontend tests, 12 Node CLI tests, 2 importer tests,
  TypeScript, Vite, module-Skill and changelog checks.
- Rust formatting and Clippy -D warnings passed.
- Standard Rust tests: 78 library tests and 2 CLI tests passed; 3 opt-in
  integration tests are excluded from the normal run.
- Explicitly ran real_terminal_archive_restore_and_project_registration:
  starts real Codex PTYs, verifies project idempotency and naming, archives and
  restores the same native thread, rejects implicit resurrection, reconciles
  native archive/restore, and preserves native history when deleting local data.
- scripts/test-codex-conversation-lifecycle.mjs passed against installed Codex.
- Clipboard routing tests cover Chinese multiline text, single delivery, image
  versus plain-text gestures, right-click paste, control-sequence removal,
  readiness and long Unicode input order/chunk limits.

Physical keyboard/image-paste behavior in the packaged WebView remains a manual
smoke check. No user's clipboard contents were overwritten by these tests.
No gameplay archive or login/trust/permission configuration was modified.

## Use

Open a task's Codex terminal to register/reuse its native project and synchronize
its existing conversations. Use the row's archive icon to archive it; use the
archive filter above the list to find and restore archived conversations.
The refresh icon explicitly synchronizes changes made in Codex itself.

For historical organization without opening a model turn,
scripts/sync-codex-projects.mjs reads Desktop-listed task metadata and uses only
native project/name APIs. It does not resume, archive or delete user threads.
It also accepts explicitly supplied historical workspace roots and matches
native threads by exact cwd. It does not recreate missing Desktop task records.
Two visible historical test threads were organized. One stale metadata reference
to a native thread with unavailable rollout was skipped, without deleting data.

## Deployed result

Updated the original Publish directory and restarted Desktop at 22:45 local time.
Baseline backup: Publish/.backups/terminal-ui-20260924-223408.
Intermediate backup: Publish/.backups/terminal-ui-guide-20260924-224532.
GUI SHA256: 61A5D15FD308A56CB07C9475177985E4D7ED50260201D519D34CF88A63895A4A.
CLI SHA256: F26068EF1159FFF72572D96EDC0286E6D6A6B1B9FEFBE5500F075F1340D8B7BA.

The two visible historical test threads are now in the Codex app's ABYA 开发
section (verified through the app's sidebar listing). Desktop returned no task
records during historical organization; only explicitly supplied retained
workspace metadata and exact native cwd matches were used. No task was recreated.
