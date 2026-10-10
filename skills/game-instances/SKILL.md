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
reports, bootstrap log paths, per-instance CLI launch identity, and the
selected LAN gateway endpoint. Canonical modes are `editor`, `offline`,
`lan-host`, `lan-client`, and `igp-hosted`; legacy `normal`, `host`, and
`clientOnly` values deserialize as compatibility aliases. Ordinary desktop
launch does not expose IGP secrets and rejects `igp-hosted` without an
independently secured in-memory IGP context.

Managed launches use production `--abya-launch-*` archive, level, report, and
LAN parameters. They pass the desktop WebSocket connection arguments and --abya-cli-autostart=true. The child receives ABYA_CLI_DEVELOPMENT=1 in its process environment. The game generates the runtime endpoint and token; the desktop retains no runtime connection secret. Runtime commands resolve the live managed PID and verify the desktop launch identity.

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
desktop instance ID. They have no task, process, or CLI control, but may
provide logs and capability-gated archive transfer. Finished managed instances
and disconnected external history may be deleted; active processes and
connected sources are protected. SQLite cascades associated structured log
sessions, events, and archive transfer history.

Read `docs/ABYA_GAME_RUNTIME_CONTRACT.md` before changing launch arguments,
archive parsing, identity allocation, or local multiplayer behavior.

## Dependencies

Depends on foundation, task contracts, and the game-connections endpoint.
Runtime CLI and log protocols remain outside this module.

## Validation

The window recorder uses the bundled pinned FFmpeg LGPL shared build. Start selects
the largest non-minimized window owned by the managed Player PID and captures that
HWND through FFmpeg gfxcapture / Windows.Graphics.Capture at 15 fps to H.264 MP4, without audio. GDI is not a fallback: it may capture occluding windows over GPU-rendered content. A zero HWND is rejected; no monitor, title or executable matching is exposed. It temporarily restores and raises the
game window without activation, then restores the prior visibility on stop, early
failure or maximum duration. Occlusion must not replace the target content; minimization and resize can invalidate visual
evidence. Metadata records task, instance, version, frames, times and final state.
A Windows Job Object owns the encoder; process/APP stop finalizes or marks interruption.
The native smoke test uses an owned brief non-activating fixture, checks actual
encoding/dimensions and is run explicitly. Visual QA must inspect decoded frames;
an earlier off-screen probe generated black frames and is not a passing capture.

Test exact production launch and desktop-connection arguments, CLI autostart and absence of legacy launch arguments,
Host port allocation, archive parsing, LAN client Host selection, launch
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

Rust test builds append an isolated --abya-data-root for real Player validation. Production builds do not change the game data root.

A wall-clock watchdog bounds a stalled capture to maxSeconds plus five seconds of startup allowance, marks it failed and restores prior visibility. The occlusion regression decodes two video frames and checks target pixels under a separate topmost cover; static fixtures must animate to provide compositor updates.

## Stage workspace update

The stage workspace release retains the existing Windows Graphics Capture recorder fix. Verification must keep file completion separate from frame review and reuse the owned-window occlusion regression.

## Acceptance launch consistency

LAN clients inherit the selected live same-task Host's executable as well as archive/level/port. Validate the inherited executable before spawning so generic UI and CLI cannot mix Player builds within one pair.
RuntimeBridge owns acceptance orchestration and calls this module's existing lifecycle APIs. Failed acceptance launches stop only instances created by that attempt; unrelated and reused instances are preserved.
The generic launch dialog prefers the selected task's captured Player over global settings. The dedicated acceptance button additionally validates fingerprints and readiness. Test inherited executable/archive and task ownership.
