# Game Runtime CLI Bridge

## Current scope

Workflow 2.7: authoring.rs guards agent launch and scoped capability dispatch. Approved requirements/plan authorize implementation/review; design permits catalog reads and cleanup, not architecture/Lua checks. APP-only start_prototype verifies task ownership, copies saved state, launches a dedicated editor and commits a short-lived archive grant. Before prototype writes, read the live archive path and compare its canonical path; archive-switch commands are refused. Grant failure stops only the new instance. No claim of a general filesystem sandbox.
Transport success is distinct from data.valid/ready and runtime acceptance. Preserve delayed-response budgets, exact ownership, operation receipts and unknown-write reconciliation; never auto-replay writes.

## Purpose

Invoke the bundled Abya CLI for desktop-owned game instances.

## Ownership

Own fixed Node/script execution, PID mapping, stable conversation sessions, bounded IO, cancellation and screenshot artifacts.

## Public Contracts

RuntimeBridgeService reads live managed PID from InstanceService and the task workspace from TaskService. Each CLI invocation passes --instance, --json and an operation-owned output directory. ABYA_CLI_DESKTOP_INSTANCE_ID must match the host status. No endpoints or tokens are cached by this module. JSON arguments travel through stdin without shell interpretation. Images are written under artifacts/runtime/<instance>/<operation>. Output is capped at 16 MiB, stderr is drained with a 64 KiB cap. Writes are never automatically retried. Cancellation sends the game cancel request before terminating a lingering child and returns outcome_unknown. External and stopped instances are rejected. Readiness reports CLI connectivity; the task Skill must additionally verify archive/level and required capability availability.

## Dependencies

Foundation, game-instances and development-tasks.

## Validation

CLI process lifecycle, fake CLI host, output/identity rejection, multiline input, Unicode paths, screenshots, session isolation, cancellation and stopped-instance rejection.

## LLM Maintenance Rule

Update this Skill, command documentation and tests whenever these contracts change.

read_artifact canonicalizes both the task runtime artifact root and requested file, rejects path escape and non-PNG/JPEG files, and bounds images to 16 MiB before returning a data URL.

The shared process budget allows eight concurrent runtime CLI calls. Stopping a terminal requests cancellation for its active session. Release builds require bundled CLI and Node files and never fall back to the developer source tree.

Bundled CLI discovery deduplicates identical registration records. Multiple remaining
candidates are checked using authenticated status reads, four at a time with a
1.5-second per-host deadline. PID, project and ABYA_CLI_DESKTOP_INSTANCE_ID must
match. Stale or foreign endpoints are excluded; multiple verified endpoints still
fail with ambiguous_instance. Discovery neither deletes registrations nor invokes
gameplay capabilities. Runtime ID is not the CLI selector. Four independent process
fixtures cover PID routing even when legacy Runtime IDs are identical.


## On-demand history

Read the matching section in [historical contracts](../../docs/skill-history/20261026/game-runtime-bridge.md) only when changing that surface. New current-scope rules override historical versions.

Long response transport (2026-10-26), 内存保护与操作追踪（2026-10-19）, Task acceptance orchestration, Durable runtime receipts and snapshot launch (2026-10-23).
