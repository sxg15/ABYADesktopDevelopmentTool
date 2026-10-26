# Desktop CLI

## Candidate and fixture versions (2026-10-26)
production version requires purpose candidate|fixture when the task enables versionPurposeRequired.
Older policies retain their default. Fixture snapshots never select the current candidate.
production activate-version maps to development_production_activate_version with expectedRevision,
sourceRevision and versionId under the existing authenticated conversation/task context. It only
selects verified frozen history within the current cycle and approved scope; no paths/hashes or
approval decisions are accepted from callers. register-evidence may select a known fixture version.

## Workflow 2.5 (2026-10-24)
save-work-unit accepts kind implementation/deployment/validation/recovery; recovery binds
issueIds. save-issue accepts optional diagnosisStatus and bound diagnosis material; repairTarget
alone never proves platform causality. production batch supports register-visual without
changing approval or file-validation rules. The task-bundled runtime-scenario.mjs uses the
existing runtime run CLI with durable request IDs, bounded read-only polling and assertions;
unknown writes are never repeated. It introduces no new transport, permission or approval API.

## v5 workbench (2026-10-21)
production update adds update-workbench-request {id,status,summary}, status in-progress/completed. It updates only an existing user goal. Request creation, draft editing, deferral/restoration, attachments and exports are APP-only; no new agent approval route.

game_instance_get 追加只读 memory 投影，包含最近样本、峰值、压力状态和异常报告路径。
没有增加代理可调用的阈值修改或自动停止授权接口。runtimeInstanceBusy 与
instanceMemoryPressure 的 not_executed 说明操作未派发；outcome_unknown 仍须核对原请求，
不能盲目重放写操作。请求身份与输入摘要进入有界异常证据，认证信息不复制。

## Codex-only context (2026-10-19)
CLI conversation/Skill provider schemas accept only codex. Removed provider inputs
fail before binding or recording activity, even if they name an existing Codex ID.
GrokTerminalService is absent from dependencies, bootstrap and shutdown. Existing
Codex command names, session capabilities and ownership checks remain unchanged.
The opt-in Player test is real_player_cli_pipeline_with_codex_context; distinguish
its execution from normal tests that do not launch a Player. Regress unsupported
provider rejection and intact Codex activity/plan binding.

## Art design workflow 2.2 (2026-10-16)
Authenticated `art list` / `art get` map to development_production_art_library_list/get,
use the caller's task binding and expose only curated library data. Latest accepted
search is read-only; fixed-version read preserves imported/withdrawn provenance.
production update supports record-art-input, save-art-requirement, review-art-source,
analyze-art-reference, save-art-direction, save-art-coverage, save-art-realization,
save-art-case-draft, inspect-art-case and use-art-case. Exact contracts are in bundled art-design-records.
inspect-art-case copies candidate references into the task before visual inspection;
it does not adopt a case, invalidate a confirmed design or approve anything.
use-art-case copies verified frozen media into the task and records borrow/adapt/avoid;
it does not approve a design. Collection, requirement withdrawal, library import/export
and recommendation withdrawal remain APP-only. Never authenticate AI-transcribed text
as a human source or allow CLI operations to alter existing planner requirement scope.
Validate the Codex context, stale revisions, schema discovery and no approval bypass.

## Game client settings composition (2026-10-14)

SettingsView composes game-clients' GitHubAccountSettings and source/installation picker.
Browser OAuth belongs to the settings account section; auth changes refresh remote choices
without saving the Player draft. Manager login navigation focuses that section.
SettingsView saves the source metadata
and resolved launch path together through the existing settings adapter. Download, login,
Release management and update checks remain APP-only; no CLI command accepts GitHub tokens.
Download and use now activates/saves a verified remote client directly through the
game-clients workflow provider. Manual source/local-path changes still use Save. Split
the client draft synchronization from other fields so activation cannot reset unsaved
workspace, language or gateway edits. The selected package replaces the historical picker;
show deferred-cleanup reasons. No new CLI commands or task-version migration are added.
Existing CLI launch defaults continue using gameExecutablePath and preserve task-pinned
Player precedence. The gateway/CLI fixtures explicitly use the default local selection.

## Purpose

Expose the existing desktop application through a pure CLI workflow.

## Ownership

Own authenticated loopback command dispatch, schemas, task/conversation validation and semantic activity correlation.

## Public Contracts

Standalone loopback HTTP calls use foundation's no-proxy transport, including when Windows
proxy settings lack a localhost exclusion. Managed named-pipe transport remains unchanged.
Local HTTP server fixtures also disable proxies so tests never depend on a running proxy.

production update accepts publish-questions and configure playerMode. The dispatcher overwrites
question provider/conversation/nativeSessionId from authenticated context; callers cannot select
another answer destination. Agent mutations never save drafts or submit user answers; those are
APP-only. Schema and production-records document limits and version semantics. The opt-in pipe
model smoke also restarts the backend, revokes/reissues its scoped lease and resumes the same
native ID, checking actual doctor/capabilities execution and rejection of old credentials.

POST /api/v1/command accepts version=1, UUID requestId, command, object arguments and optional provider/taskId/conversationId context. No MCP routes, sessions or fallback exist. Authentication uses a DPAPI-protected descriptor in current-user application data. Origin-bearing requests are rejected. Requests are limited to 4 MiB; duplicate IDs are rejected. Command contexts are validated through the owning provider service on every call. Instance operations require matching task ownership. Bootstrap task list/create, doctor and capabilities may omit conversation context. Commands retain existing domain IDs except deleted protocol aliases. See docs/DESKTOP_CLI_COMMANDS.md.

The abya-desktop binary reads JSON from stdin or a file and emits one JSON result. The terminal supplies its absolute path as ABYA_DESKTOP_CLI. Runtime commands use the existing Abya CLI through runtime-bridge. Content retains all text and image file paths. Credentials and raw outputs never enter semantic workflow events. Settings shows connection state and reset/restart actions; no client configuration templates or tokens are exposed.

## Dependencies

Foundation plus task, provider terminal, instances, runtime bridge, archives, transfers, connections and logs public services.

## Validation

production get/update/version/report and skill list/read bind to the authenticated
task context. Mutation requires expectedRevision; no approve/accept CLI exists.
production version resolves an owned instance's saved launch archive and Player,
then delegates fingerprinting to TaskService; agent-supplied paths/hashes are refused.
recording tools/start/get/stop delegates to game-instances, with existing instance
ownership checks. Reports and videos are task-owned artifacts; neither their
creation nor command success marks a task complete. Recording is video-only,
bounded to two hours and never records a desktop-wide source.

Authenticated command integration, Origin rejection, old endpoint absence, schema uniqueness, task ownership, Codex, activity updates, runtime failures and command parsing.

## LLM Maintenance Rule

Update this Skill, command documentation and tests whenever these contracts change.

Managed log-session and transfer IDs are resolved to their instance before ownership checks. External sources retain logs/archive-transfer access only. The real Player opt-in test verifies CreatorEditor readiness, HighImpact pure Lua, screenshots, the Codex context and independent LAN Host/Client readiness without launching model inference.
The opt-in test saves runtime-catalog.json and checks essential capabilities plus
retired log-server exclusions instead of fixing the total catalog size. Inspect
that captured catalog for Bot integration prerequisites; transport readiness with
an older Player is not evidence of Bot gameplay or runtime_bot_diagnostics support.
Set ABYA_CLI_SMOKE_BOTS=1 for the opt-in real Player test to require Bot definition
get/set and diagnostics in the catalog, then read definitions, diagnostics and the
exact receive/submit_action Lua API under the Codex context. It saves separate
bot-<provider>-<check>.json evidence without authoring or saving any Bot definitions.

## Windows sandbox sessions
Managed terminals use a random local-only named pipe with explicit owner and available CodexSandboxOffline/Online SID ACLs. After reading each bounded request, inspect peer identity without retaining impersonation. Validate the in-memory capability against the entire provider/task/conversation context before dispatch. A managed session cannot create/list global tasks. Requests are capped at 4 MiB, responses at 32 MiB, concurrent connections at 32, initial reads at 5 seconds and operations at 720 seconds. Duplicate IDs share the HTTP registry. Service stop revokes all capabilities and closes pipes. Native provider session configuration receives ephemeral credentials; ABYA never writes them to its logs or project files. The standalone DPAPI descriptor is not read by managed CLIs. Run sandbox_pipe_authentication_smoke alone with --ignored; ABYA_TEST_MODEL=1 also runs the model-backed test script.


Pipe clients request explicit data rights without FILE_CREATE_PIPE_INSTANCE and verify ABYA_DESKTOP_PID before transmitting their session credential. Sandbox SIDs cannot create additional server instances. Invalid sessions and server identity mismatches use CLI exit code 4.

Recorder capability metadata reports windows-graphics-capture. The input remains a managed instance ID; arbitrary HWND/monitor selection is not exposed to callers. GPU window capture uses the existing pinned FFmpeg runtime, without a GDI fallback.

## Stage workspace update

conversation report accepts optional plan objects with step and pending/inProgress/completed/failed status. Provider identity comes from the authenticated binding. production update additionally accepts register-artifact, update-stage and register-approved-template. Stage questions and issue affectedStages/blockedOperations are documented in production-records; user answer/approval operations remain APP-only.

## Workflow 2.0

production start-acceptance / acceptance-status expose development_production_start_acceptance / development_production_acceptance_status, with authenticated task binding and the same RuntimeBridge service used by APP.
production update adds configure-acceptance, register-visual, save-self-test, reuse-evidence and update-feedback. Contracts are in production-records; old save-round/set-milestone remain for pinned 1.x tasks only.
No Agent command can close human feedback, upload via the APP-only attachment action, or approve documents. User feedback operations are Tauri-only. Acceptance start changes process state but never accepts a game.

## Recreation integration validation (2026-10-13)
The opt-in delete_then_recreate_conversation_sandbox_smoke requires a built debug CLI,
installed Codex and ABYA_TEST_WORKSPACE pointing to disposable local storage. It
creates a task, checks sandbox doctor, opens/deletes/recreates native conversations,
and verifies independent IDs. A failed workspace probe remains a warning only with
authenticated healthy Desktop service; stopping the service must still block opening.
No model inference or game actions. Run alone because managed leases are process-global.

## Background task notifications (2026-10-14)

SettingsView composes the task-notifications settings panel (default-on background action reminders, availability reason, explicit test). These controls use typed Tauri adapters only; no desktop CLI/MCP commands, scopes, credentials or command-line inputs are introduced.

## Autonomous workflow CLI contracts (2026-10-23)
production current/receipt/batch map to development_production_current/receipt/batch.
Scoped conversation binding and task checks apply. Batch IDs are UUIDs; items have operation
and data, with a restricted mutation allowlist. Receipt action IDs are batchId:index.
production update adds save-work-unit/report-work-unit/record-issue-handling,
conclude-implementation and record-validation-run; exact fields are in the bundled
references/autonomous-implementation.md. No agent approval or user-feedback closure added.
runtime receipt maps to game_runtime_operation_receipt and enforces instance ownership;
returns status/hash/times/errorCode, not raw runtime output. Reuse --request-id only with
identical intent; unknown outcomes are reconciled, never retried under a new ID blindly.
