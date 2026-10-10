# Codex Terminal Skill

## Purpose
Provide task-owned Codex conversations with recoverable native identity and an embedded TUI.

## Ownership
Owns CLI discovery, PTYs, native conversation metadata, app-server lifecycle, recovery,
conversation naming/archive state, and mapping native activity to development-terminal.
Conversation metrics expose native turn timestamps, answer waiting intervals and latest token
snapshots. Intervals may overlap; never label them pure model computation or sum them blindly.
Rebinding backs up and resets old workflow/connection projections to avoid mixing identities.

## Public Contracts
Conversation CRUD, rename/archive, availability, open/write/resize/stop, workflow reads,
project workspace, recovery candidates/binding repair, and continue_questions.

Resolve Codex from process and persisted Windows PATH plus APPDATA/npm. Command scripts
run through ComSpec; fixed workspace and no-alt-screen are not user-editable arguments.
CLI settings, login, sandbox, approvals and trust remain user-owned. Never copy provider
credentials into task workspaces or expose them through DTOs.

The `app-server --help` capability probe runs without a Windows console, with all
standard streams disconnected. It must not open Windows Terminal during availability
checks; use the same CREATE_NO_WINDOW flag as the managed app-server process.

Each APP conversation owns one runtime app-server and at most one live TUI. A separate
cached app-server handles metadata only. TUI uses the authenticated loopback endpoint
and exact saved native ID. Runtime isolation permits replacing a stopped executor without
interrupting other tasks or reusing its revoked ABYA environment. A live reattachment
reuses the PTY; closing the TUI detaches without revoking access for a still-running turn.

Persist a newly allocated native ID before instruction injection or other fallible RPCs.
Preparation failure preserves that ID and fails visibly; no silent standalone/new-thread
fallback. Production requires app-server support. Only synthetic PTY tests bypass that check.
Legacy recovery binds automatically only when one local conversation and one unused native
candidate remain. Ambiguous histories require explicit repair, never nearest-time guessing.

stop interrupts the actual native turn and waits for acknowledgement, closes the PTY and
owned executor, then revokes ABYA access. Failure to acknowledge preserves access and reports
failure. Dead executors can be cleaned up. Continue recreates that conversation's executor,
resumes its original ID with fresh scoped environment, and checks returned identity.
A detached turn still running must be paused before replacing its executor.

Fresh start/resume injects ABYA_DESKTOP_CLI, task/provider/conversation/workspace and the
in-memory pipe capability/PID into the tool-executing backend, not just the remote TUI.
No implicit permission override or permanent token; explicit user settings are described below. Reopen runs the existing sandbox doctor
preflight; the managed instructions require actual tool-context doctor before operations.
A preflight alone is not proof that a later model executor received the environment.

Persist developer instructions in new threads without hidden inference. Agent turns publish
plans for multi-step work. Native plan/item/turn events update the shared workflow, using
its sanitized detail whitelist; never persist raw commands, patches, results or credentials.
Bridge responses must have no method: server requests with colliding numeric IDs are not
responses. Native terminal approval UI remains responsible for sensitive confirmations.

Question submission saves answers and registers a continuation intent in the same service call.
continuation.json and continuation-events.jsonl persist intent/dispatch transitions; process epoch
prevents automatic execution after restart. Busy native turns queue answer/document-revision
intents until successful completion; plain Continue while running does not interrupt or enqueue.
Merge duplicate unsent intents, replace superseded answer versions, and check active task,
exact provider/conversation/native binding and current human gates before sending. Pause marks
cancellation before taking the lifecycle lock; late events/connection callbacks cannot unpause.
Flush consumes only the existing request ID; it never creates user intent. Failed/interrupted
producer turns park their queues. Unknown send results require review, not automatic retries.
Shutdown detaches all UI channels before waiting for native cleanup. Queue tests use controlled
RPC completion and a dispatch barrier, including ten fast-answer cycles and pause races.

Question continuation resolves the submitted answer revision from TaskService, verifies its
provider/conversation/native ID and a connected terminal, checks idle state and starts a turn
with clientUserMessageId. Receipt delivery is separate from execution completion: read the
recorded native turn before deciding whether to resume. Running/completed/later manual turns
return running/completed/superseded without another model call. Interrupted/failed latest turns
create a new attempt and message ID, preserving the previous receipt; the recovery instruction
requires checking completed operations before continuing. Persist pending before RPC. Unknown,
missing or foreign history fails closed without replay. Return needsConnection for an otherwise
eligible attempt with no live terminal. The typed result never calls an old receipt newly sent.
This does not approve documents. Test legacy receipts, status branches and pending durability.

Conversation metadata, transcript.log, workflow.json and bounded workflow-events.jsonl live
under the task conversations directory. connection-events.jsonl contains only event labels,
IDs and timestamps; usage-<native-id>.json stores the latest native token totals, not deltas.
Do not sum replayed total snapshots. Missing usage is unavailable, not zero.

Recovery lists exact-workspace native histories. Repair requires a stopped APP conversation,
validates candidate ownership/archive/idle state and uniqueness, backs up metadata, and
records old/new association. It never merges/deletes native history. Preserve later user
corrections in a sourced recovery note. It does not upgrade pinned Skills automatically.

Native project APIs associate recorded IDs and synchronize names. Registration does not
guarantee Codex desktop sidebar visibility. Show exact ID/workspace and compatibility
limitations; do not write Codex databases or invent navigation deep links.
Archive/open/delete/repair are serialized. Archive interrupts owned work before closing,
unsubscribing and archiving; deletion archives native history first. Native sync failures
retain local records. Verify archive state from supported fields/storage layout; unknown
state fails closed. An archived conversation never resumes implicitly.

Transcript replay is output-only, bounded to 256 KiB in 64 KiB chunks, ordered before live
output on reattachment. Strip historical ANSI controls, device queries and input modes; a new
ConPTY handshake must receive replies only to its live queries. Input and resize command adapters
run blocking pipe/Windows work in spawn_blocking rather than the UI or async worker threads.
Full transcript copying uses the shared streaming normalizer and
never returns the entire file to WebView. Stop drops UI/PTY handles before bounded Windows
process-tree cleanup. Per-conversation history remains after stop.

## Dependencies
Foundation, development-tasks public services and development-terminal. No game or CLI
dispatch dependency. Metadata backend replacement must not clear running-thread routing.

## Validation
Run unit checks for argument construction, native matching, ambiguity, persistence, redaction
and identity. Opt-in real_terminal_archive_restore_and_project_registration verifies the
actual TUI after terminal handshake, native identity, independent executors and archive/restore.
Sandbox/model recovery test verifies revoked credentials fail and original native history
executes doctor/capabilities with renewed credentials. Never replace these with spawn-only tests.

## LLM Maintenance Rule
Update this Skill with discovery, transport, lifecycle, contracts, diagnostics or test changes.

## Stage workspace update

Explicit user-selected per-conversation execution settings use native thread/settings/update, with model/list advertised effort choices. AI review maps to on-request + auto_review + task workspaceWrite; full consent maps to never + dangerFullAccess. Native managed-policy rejection remains visible. Save only acknowledged model/effort/reviewer/policy fields in execution-settings.json; capture native settings events, restore on executor recreation, and leave the global config unchanged. Both TUI and queued turns share native thread settings. Running changes target future turns; pause-and-apply interrupts and awaits acknowledgement before updating and user-requested continuation. Never start a model turn just to inspect settings. CLI plan reporting updates the same sanitized workflow under its write lock. New instructions require contextual plain-language progress and provide the plan-report fallback.
