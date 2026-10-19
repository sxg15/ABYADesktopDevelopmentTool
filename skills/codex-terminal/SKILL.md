# Codex Terminal Skill

## Interrupt current turn (2026-10-19)
interrupt_turn(task, conversation) cancels queued work before the lifecycle lock,
validates the exact native binding and interrupts through the existing app-server.
After acknowledged native idle, keep the TUI, executor, history and scoped access
for new user input. Do not detach, revoke or send a new model turn. Pause still
closes/revokes normally. Park queue intents; confirmed interruption marks them
interrupted, except uncertain sending/needsReview receipts remain for review.
Native snapshots must have the matching thread ID and turns array. Failed or
unknown acknowledgements do not claim success. Record requested/failed/confirmed
events using fixed labels and IDs only. Connected interrupted conversations project
awaitingInput, after higher-priority human gates and active native work.
Test retained handles/input/native ID, canceled queues/late events, retry via explicit
Continue, acknowledgement failures and foreign/malformed identity rejection.

内存事件通过公开的 memory_incident_context 读取已存在的工作流元数据；最多三会话、
每会话三轮、每轮32项活动。只保留身份、类型、状态、时间和过滤后的摘要，不复制原始
终端、参数或认证材料。异常上下文采集不阻塞实例保护；缺失/中断不能补造历史或归因。

## Persisted case request continuation (2026-10-17)
The fixed generic continuation prompt also tells the model to inspect APP-authored
artDesign.caseRequests by cycle/version, reconcile already processed work and then
inspect/adapt or prepare drafts. This supports older pinned Skills without rewriting
them. Domain requests are saved before queuing; native identity, wait/approval gates,
delivery receipts and targeted repair prompts are unchanged. Requests never imply
adoption, planner satisfaction or permission to skip an existing confirmation.

The fixed generic continuation prompt instructs AI to read active plannerReference
artifact paths and notes, view the images and use them as art inputs. Withdrawn
references are excluded; inputs never imply acceptance. This also supports pinned
legacy workspaces without rewriting their Skills. Existing native binding, queued
delivery, human gates and targeted repair prompts remain unchanged.

## Purpose
Provide task-owned Codex conversations with recoverable native identity and an embedded TUI.

## Ownership
Owns CLI discovery, PTYs, native conversation metadata, app-server lifecycle, recovery,
conversation naming/archive state, and mapping native activity to development-terminal.
Conversation metrics expose native turn timestamps, answer waiting intervals and latest token
snapshots. Intervals may overlap; never label them pure model computation or sum them blindly.
Rebinding backs up and resets old workflow/connection projections to avoid mixing identities.

## Public Contracts
The app-server HTTP readiness probe explicitly disables system/environment proxies because
its endpoint is always 127.0.0.1. Remote GitHub proxy support must not affect local startup.

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

Both metadata and runtime app-server bridges explicitly bound incoming WebSocket
frames and complete (including fragmented) messages at 128 MiB. Native thread/read
and thread/resume history can exceed the library's default 16 MiB frame limit.
Keep limits finite; do not truncate histories or create replacement conversations.
Capacity failures return codex_response_too_large with fixed safe guidance rather
than a generic disconnection. Never expose raw payloads or credentials in errors.
Loopback transport tests cover an 11-turn response over 16 MiB, a fragmented response
over 64 MiB, continued RPC use, and rejection above the configured bound.

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

## Native interaction visibility (2026-10-12)
thread/status/changed status.activeFlags waitingOnApproval/waitingOnUserInput sets
workflow terminalInteraction. Other flags/states clear it. A live runtime projects
waitingForTerminal before running/queued; the UI directs users to the original terminal.
Only the existing native terminal handles the request; no approval is auto-answered.
Contract verified with the locally installed app-server JSON schema. Persist only the
boolean, not native approval request contents, commands or user input.

## Resume conflict recovery (2026-10-13)
Native RPC failures are classified before returning through AppError: active writer,
timeout, missing history, archived history, transport and other native failures.
Only fixed user guidance, method and numeric native code leave the RPC boundary;
raw messages/data may contain credentials and must never be displayed or logged.
Each fresh attachment has a diagnostic request ID, stage, duration, backend PID and
exact task/conversation/native IDs in connection-events.jsonl. Failed attachment
stops only the executor created by that attempt and revokes its scoped access;
failure before replacement must preserve an existing active executor.
notLoaded is server-local, never proof of globally unowned history. External writer
conflicts require explicit release in the owning client and a manual same-ID retry.
No automatic archive/unarchive, thread/start fallback, process-name kill or turn replay.
A connected needsConnection queue projects awaitingContinue without mutating its
request, receipt or approvals. Connection and execution remain separate DTO fields.
Regression tests cover conflict-to-retry identity, sanitized RPC diagnostics, failed
executor cleanup and connected pending continuation without new turn/start calls.

## Workspace startup recovery (2026-10-13)
preflight first performs a bounded authenticated pipe doctor request using the new
conversation's live capability. Missing/unhealthy service, CLI binary or workspace
remain fatal. The separate codex sandbox doctor only probes workspace execution;
its failure is a classified warning after Desktop health has passed, allowing native
terminal/permission controls to open. This is not a sandbox bypass: runtime thread
config receives the current scoped environment, native permissions remain unchanged,
and each actual tool command still faces native sandbox/approval and CLI authorization.
Never fall back to HTTP or standalone credentials. Never auto-start a model turn.
Warnings expose only fixed classifications, exit/OS codes and safe guidance in the
workflow and correlated connection diagnostics; raw stdout/stderr remain private.
Deleting a conversation also clears queue task/cancellation/stopping indexes. Native
history is still archived first. Test clean delete/recreate plus an injected failed
workspace probe with a healthy service and a hard failure after the service stops.

## Background task notifications (2026-10-14)

attention_states scans only task-local valid UUID metadata/workflow/continuation files, skips archived conversations and reuses project_control with queue tracking disabled. It performs no native sync, workspace/Skill writes, RPC or session startup. Native waitingOnApproval/waitingOnUserInput transitions generate a persisted interactionId and clear it on leaving the wait; repeated status events retain the same ID. Same-turn subsequent waits are distinct. The monitor observes all conversations without taking the lifecycle RPC lock.
Readonly workflow reads use try_lock on the write lock, returning unavailable during replacement rather than interpreting a temporary missing snapshot as a resolved wait.

## Targeted issue repair queue (2026-10-13)
repair_issue creates a typed RepairIntent (issueId, fingerprint) in the existing
continuation queue. Validate the selected issue and native identity; recheck its
fingerprint before turn/start. Changes/resolution/programmer classification cancel
stale delivery. Busy turns defer it without interruption. Reconnection only flushes
the saved request ID. Never overwrite another pending continuation. Plain Continue
may resume a paused repair intent; no automatic replay on process restart.
Per-fingerprint issue-repair receipts retain deduplication even after the active
queue changes. Unknown sending outcomes require review. Receipt paths accept only
64 hexadecimal characters. Human gates pause repair; answer/approval operations keep
their existing workflows and the user can explicitly request repair again afterward.
Only fixed backend prompts go to turn/start, not arbitrary UI-provided commands.
Tests cover exact issue/native identity, duplicate clicks, busy dispatch, changed
ownership, competing intents and no automatic issue-resolution mutations.
