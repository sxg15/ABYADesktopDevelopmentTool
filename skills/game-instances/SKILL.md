# Game Instances Skill

## Purpose

Build visual launch profiles and own managed ABYA game process lifecycles.

## Ownership

Owns executable validation, argument generation, multiplayer port and identity
allocation, Windows Job Objects, metrics, and stop behavior. Archive parsing
and discovery belong to `game-archives`.

## Public Contracts

`LaunchProfile`, `LaunchMode`, `ArchiveSelection`, `GameInstance`,
`InstanceOrigin`, `ProcessState`, `ConnectionState`, `InstanceStopResult`,
`LaunchReportResult`, `WindowVisibilityMode`, and instance lifecycle commands.

Launch profiles expose only typed choices. They generate user IDs, user names,
reports, bootstrap log paths, per-instance Runtime MCP endpoints, and the
selected LAN gateway endpoint. Canonical modes are `editor`, `offline`,
`lan-host`, `lan-client`, and `igp-hosted`; legacy `normal`, `host`, and
`clientOnly` values deserialize as compatibility aliases. Ordinary desktop
launch does not expose IGP secrets and rejects `igp-hosted` without an
independently secured in-memory IGP context.

Managed launches use production `--abya-launch-*` archive, level, report, and
LAN parameters. They also pass the desktop WebSocket connection arguments and
a random Runtime MCP port, 43-character URL-safe token, forced MCP autostart,
and `--abya-mcp-auto-approve=true` so Internal MCP HighImpact tools skip the
in-game confirmation dialog. The auto-approve flag is process-local and is not
persisted into game settings. The token lives only in process memory and the
child command line; persisted `sanitizedArgs` contains `[REDACTED]`.

New launches default to `background`: the Unity Player remains windowed and
rendering, while a Windows PID-scoped controller preserves each visible
top-level window's bounds and extended style, removes it from normal task
switching, and moves it outside the display area without deactivating Unity's
D3D presentation path. It restores the latest non-game foreground window if
Unity attempts to take focus. `visible` restores the saved bounds/style and
uses `SW_SHOWNOACTIVATE`. Background mode is valid only with windowed
rendering. The desired visibility is persisted in the launch profile, while
HWND values, saved window state, and controller threads remain in memory and
stop with the process.

`lan-client` must select a live `lan-host` in the same task and inherits its
archive, level, Host address, and Host port. Process handles are held in memory
and wrapped in a Windows Job Object. Stop first requests process-tree exit,
waits five seconds, force-terminates when necessary, waits another ten seconds,
and returns the observed result.

Managed stdout and stderr share the instance diagnostic log. The process
monitor checks it every 500 ms and truncates it with an explicit marker when it
exceeds 64 MiB, including while the child append handles remain open, so noisy
Unity output cannot exhaust the system drive.

External instances are created when a user-started game connects without a
desktop instance ID. They have no task, process, or MCP control, but may
provide logs and capability-gated archive transfer. Finished managed instances
and disconnected external history may be deleted; active processes and
connected sources are protected. SQLite cascades associated structured log
sessions, events, and archive transfer history.

Read `docs/ABYA_GAME_RUNTIME_CONTRACT.md` before changing launch arguments,
archive parsing, identity allocation, or local multiplayer behavior.

## Dependencies

Depends on foundation, task contracts, and the game-connections endpoint.
Runtime MCP and log protocols remain outside this module.

## Validation

Test exact production launch and desktop-connection arguments, MCP auto-approve,
token redaction,
distinct Host/MCP ports, archive parsing, LAN client Host selection, launch
report bounds, wait timeouts, background launch defaults, legacy visible
profile compatibility, PID-scoped off-screen/restore behavior, foreground
preservation, hidden task-switching state, continued background rendering,
process-tree termination, bounded instance stdout/stderr logs while append
handles remain open,
managed/external registration, external process-control rejection,
active-instance delete protection, and log cascade deletion.

## LLM Maintenance Rule

The instance service exposes storage repair for truncating oversized files
under the authoritative InstanceLogs directory.

When changing launch options, instance data, process behavior, or validation,
update this Skill in the same change.
