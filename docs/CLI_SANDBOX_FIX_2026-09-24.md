# Windows sandbox CLI repair — 2026-09-24

## Confirmed failure

Desktop runs as Indiegamespass; Codex shell commands run as CodexSandboxOffline.
The shell retained the original USERPROFILE/LOCALAPPDATA values. Consequently,
the old CLI found the original user's descriptor but could not decrypt its
user-scoped DPAPI token. Same-user doctor/capabilities succeeded.

The original task directory existed and was accessible after starting a shell
from C:\, but starting the sandbox runner directly there produced
CreateProcessWithLogonW error 267. ASCII paths, short paths and junction aliases
did not fix this machine's AppData startup case. A physical workspace under
USERPROFILE/ABYA Desktop Development ToolWorkspaces passed. This is a verified
compatibility remedy, not a claim that all AppData paths fail on every machine.

## Implementation

- Managed terminals receive a random local named-pipe endpoint, Desktop PID,
  and an in-memory 12-hour session capability through their execution environment.
- The pipe allows only the Desktop user and installed named Codex sandbox users.
  Sandbox identities have data access, without creating extra server instances.
- CLI verifies the pipe server PID before sending the capability. Desktop
  checks the client's Windows SID and the bound provider/task/conversation.
- Explicit stop, natural terminal exit and Desktop service restart revoke
  access. Invalid/expired capabilities fail closed; no DPAPI HTTP fallback.
- Desktop retains its long-term credential. Same-user standalone CLI access
  remains available through the existing protected HTTP descriptor.
- Requests, response size, concurrency and waits are bounded. Existing command
  validation, duplicate-request checks and cancellation remain in use.
- New default workspaces are outside AppData. On terminal open, former default
  workspaces are copied through staging before their database path is updated.
  Original directories remain as recovery copies. IDs and conversation metadata
  are preserved. Custom roots, active task terminals and reparse points are not
  silently migrated. Conflicts preserve data and return an error.
- Codex terminal startup runs a hidden sandbox doctor preflight with ordinary
  permissions. Failed directory startup, connection and timeout are distinguished.
- No login credentials, global trust configuration or sandbox policy were changed.

## Validation

- npm run check: passed (19 frontend tests, 12 Node CLI tests, 2 importer tests,
  module/Skill checks, changelog checks, TypeScript and frontend build).
- Rust formatting and Clippy with -D warnings: passed.
- Standard Rust suite: 77 library tests plus 2 CLI tests passed; 2 opt-in tests
  are excluded from the ordinary run.
- sandbox_pipe_authentication_smoke: explicitly run and passed. Exercises
  real CodexSandboxOffline commands, doctor, capabilities, wrong credentials,
  wrong server PID, cross-task/provider isolation, duplicate IDs, revocation,
  and stopped service handling.
- Model-backed test using installed Codex 0.156.1 and its actual app-server:
  completed; one real commandExecution, sandboxIdentity=true, doctor=true,
  capabilities=true, errors=false. No permission override or auto-approval.
  The temporary native test thread was archived.
- Workspace migration regression: binary content and conversation history
  preserved, original source retained, database updated, repeated calls
  idempotent, custom root unchanged.

The test did not launch a game or modify a gameplay archive. Real Player
gameplay acceptance from the earlier migration is separate from this repair.
Grok's environment injection and provider isolation are tested; no live Grok
model inference was run in this repair.

## Operator check

Open the existing task's terminal after updating Desktop. Its old default
workspace is repaired on this first open; native conversation ID is retained.
Run doctor and capabilities, then continue the game-development task. Supply
the actual test archive name/path in place of the earlier prompt placeholder.
If a capability expires, stop and reopen the terminal to obtain a new one.

Rollback: restore the backed-up Desktop and CLI executables together. Original
workspace directories remain available; do not overwrite either copy if work
has continued in the migrated directory.

Diagnostic cleanup note: automatic approval review blocked deletion of the
temporary probe directories/junctions with `blocked by policy`. They were left
in place. Production workspaces and archives were not deleted.

## Deployed artifact

Updated the original Publish executables and build-manifest.json, then gracefully
restarted Desktop at 2026-09-24 21:12 (local time). Startup doctor succeeded;
capabilities returned 30 desktop commands. The existing test task remains intact;
its workspace migration occurs when its terminal is next opened.

Backup: Publish/.backups/sandbox-cli-20260924-211209.
GUI SHA256: A600E91F2D515F6170F7528DF7A975EF1E1C35F75A3DBA5070342E6542B8668C.
CLI SHA256: D9DD18A1A76C30B3E0CE7A2093E480F21DF0E3360D26831BE6FAE851BC6D4584.
