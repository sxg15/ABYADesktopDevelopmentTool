# Desktop CLI

## Current scope

Workflow 2.8 adds a scoped completion loop: current-goal dependency closure, evidence-based wake signals, audited work revision, automatic repair plans, per-output finalization and honest repairRequired/externalBlocked outcomes. No candidate completion without a hash-bound playable ZIP and current lua/input/core-loop/save-reload evidence. Fixture validation remains separate. Upgrade preserves budgets, IDs and approvals; explicit resume retains the original goal and cannot replay uncertain writes. Package compression runs outside the database mutex.

Workflow 2.7 agent game_instance_launch delegates to RuntimeBridgeService.launch_scoped. Agent tool calls pass the same stage contract; there is no CLI operation granting prototypes or approvals. Existing task/conversation ownership and unknown-write contracts remain unchanged. Runtime schema must come from the selected Player, not source-tree assumptions.

## Purpose

Expose the existing desktop application through a pure CLI workflow.

## Ownership

Own authenticated loopback command dispatch, schemas, task/conversation validation and semantic activity correlation.

## Public Contracts

`runtime qualify-player` reads completed native test reports and the discovered catalog from an owned running instance, checks the actual complete Player fingerprint before/after, and records only the reported validation scope. `runtime baseline` reads the shared baseline for the task-pinned or selected Player; it never approves a game. No caller-supplied build hash, source path or test verdict is accepted. Workflow 2.11 also exposes register-acceptance-check through existing production mutations; approvals remain APP-only.

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


## On-demand history

Read the matching section in [historical contracts](../../docs/skill-history/20261026/desktop-cli.md) only when changing that surface. New current-scope rules override historical versions.

Candidate and fixture versions (2026-10-26), Workflow 2.5 (2026-10-24), v5 workbench (2026-10-21), Codex-only context (2026-10-19), Art design workflow 2.2 (2026-10-16), Game client settings composition (2026-10-14), Windows sandbox sessions, Stage workspace update, Workflow 2.0, Recreation integration validation (2026-10-13), Background task notifications (2026-10-14), Autonomous workflow CLI contracts (2026-10-23).
