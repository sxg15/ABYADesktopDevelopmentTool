# Game Runtime CLI Bridge

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

## Task acceptance orchestration

start_acceptance resolves TaskService's fingerprinted version specification, launches a visible single-player or Host/Client group through InstanceService, and rechecks every seat after joining.
Check exact launch archive/level, CLI capabilities, initialized game context/network readiness, failed Lua traces and optional CustomUI roots. This is startup readiness, not human acceptance or a full gameplay test.
A task-scoped launch lease prevents concurrent duplicate groups. An intact same-version group is rechecked and shown; partial or old groups require closing first. APP restart checks persisted session IDs against live owned processes.
Progress distinguishes version checking, Host start, joining, gameplay checks, ready, failed and stopped. Failure records the reason and cleanup failures, stops only newly created instances, and retains task/user decisions. No inference calls or automatic gameplay input occur.
Required tools and seats are version-bound, with at most eight seats. Readiness operations are bounded read-only CLI calls; runtime secrets remain in memory. Unit tests reject connected-but-failed Lua and mismatched launch reports; real Player validation is separate.
