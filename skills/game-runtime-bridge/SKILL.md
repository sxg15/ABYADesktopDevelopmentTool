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
