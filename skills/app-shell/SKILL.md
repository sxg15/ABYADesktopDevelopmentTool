# App Shell Skill

## Remote game clients (2026-10-14)

Navigation adds gameClients between archive transfer and logs. The public feature view
owns Release editing, exact-target delete confirmation, asset upload and
installation. GitHub browser login, one-time code, remember-login, retry/cancel and logout
live only in Settings. Unsigned client management shows its missing-login message and
Go to settings to sign in action, which focuses the GitHub account section. The manager's
signed-in account links to the same section. Preserve draft Player selection on auth changes.
Settings offers local/remote source, the selected version/EXE, remote
version download and storage-folder choice. Keep both locales complete, including errors,
busy/cancellation, empty repository and missing installation states. Distinguish a
metadata/web-page download response from a size/SHA-256 mismatch. Failed installation
preserves the selected game. ClientWorkflowProvider stays above navigation and owns the
download/EXE-choice/activation dialog. Download and use is a large primary action in a
highlighted panel before release notes; verified packages activate and save immediately.
For saved remote mode, startup, focus, sign-in and 30-minute checks open a direct optional
update dialog with Update now / Not now. Declining suppresses that exact candidate for
this session; a prominent reminder reopens it. Never auto-download. Ignore stale responses
after source/selection changes; serialize duplicate actions and require ambiguous ZIP/EXE
choices. Show why older task-pinned/running/local/inaccessible packages await cleanup.
Source/local-path/manual settings edits still use Save. Client activation updates the
client fields without resetting unrelated unsaved settings drafts. Preserve mounted task terminals. Layout reflows at narrow widths and
long asset names/paths wrap. Validate fixture screenshots separately from real GitHub writes.

## IME positioning (2026-10-13)
Only .terminal-ime-composing overrides xterm helper left/top/height/line-height
using compositionstart CSS variables. These temporary important rules withstand
xterm's inline cursor updates; remove them at commit/blur/disposal. Do not apply
form-field focus borders or shadows to the invisible terminal textarea. Output
continues rendering and the normal cursor positioning resumes after composition.

## Purpose

Own the desktop navigation, bilingual localization, layout, notifications, and
shared visual primitives.

## Ownership

Owns page composition but not task, process, log, or MCP business logic.

## Public Contracts

IconButtonTooltips renders immediate in-app hover/focus help for SVG-only buttons using their
existing title/accessible label, including disabled controls and modal/inline icon buttons.
It preserves accessible names/descriptions, restores native attributes on exit and dismisses
on pointer down, Escape, scroll or blur. Tooltip placement stays within the WebView viewport.

Codex labels distinguish Pause task, Continue task and Reconnect terminal. Sidebar identity comes
from embedded BuildInfo: version, release/date and validation/test status, never a hardcoded version.
ExitRequested marks native shutdown immediately and runs cleanup once on a worker, while the UI
event loop remains available for IPC teardown. Only completed cleanup requests final exit; do not
repeat blocking cleanup on Exit or hold the UI thread while waiting for native/PTY shutdown.

Navigation view IDs, locale keys, shared UI primitives, and application-level
refresh events.

The first screen is the operational development-task workspace. Navigation has
`tasks`, `archiveTransfer`, `gameClients`, `logs`, and `settings` views. Chinese and English
dictionaries must remain key-complete, and the persisted locale falls back to
the OS locale.
Feature views call typed command adapters and must not construct launch
arguments, parse MCP responses, or write log persistence directly.

The instance launch dialog exposes Editor, Offline, LAN Host, and LAN Client
as localized structured modes. Editor, Offline, and LAN Host select a local
archive/level; LAN Client selects a running Host from the same task. Secret IGP
fields and arbitrary arguments are never shown.

The launch dialog exposes `background` and `visible` as a segmented window
visibility choice and defaults to `background`. Background mode fixes the
rendering style to windowed. Running managed instances expose icon controls to
show without activation or return the Player to background mode from both the
instance table and details modal.

The task workspace remains mounted while another top-level view is selected so
opened task terminals keep their PTY channels and xterm state. Deleting a task
unmounts that task's opened terminals before the delete command so live PTY
output cannot block webview IPC. Within a task,
the `游戏实例` and `终端` panels are hidden rather than discarded after the
terminal is first opened. The conversation workspace uses a definite
`minmax(0,1fr)` row so the xterm host has a measurable height when shown.
Restoring a terminal panel must refit its rows and columns without starting a
second provider process. A first open waits for that laid-out size instead of
sending a collapsed 0×0 grid to the PTY. The terminal viewport keeps a
visible vertical scrollbar with a stable gutter so long output remains
navigable without covering terminal text or changing the surrounding layout.
While output is arriving, user scrolling remains pinned to the selected history
position; the terminal only follows new output when already at the bottom.
The output-history view keeps a rolling recent 2 MiB text window so sustained
terminal redraws cannot make the WebView unresponsive; the complete provider
transcript remains a task-workspace file rather than an unbounded DOM tree.
The terminal panel contains an explicit Codex/Grok segmented provider selector
and a task-local conversation list for the selected provider. Each task retains
the user's last provider choice. Creating or selecting a conversation mounts
its own terminal view while previously opened provider/conversation views
remain mounted and keep their PTY and xterm scrollback.
Conversation rows expose an icon-only rename action with a bounded inline input,
save, and cancel controls.

The task header is followed by a compact always-visible workflow strip before
the tabs. Its current plan uses fixed-width horizontal nodes and arrows with
horizontal overflow scrolling. Status is conveyed by icon, color, and text.
Mouse hover and keyboard focus show the most recent activities in a portal
popover; activating a node opens a responsive right-side timeline drawer.
Compatibility-mode Codex or Grok terminals remain usable and show an explicit
provider-specific unobservable warning. Long plan text, activity details, and narrow viewports
must not overlap the task header, tabs, terminal, or instance table.

Destructive instance and log-session actions use explicit localized
confirmation text and icon controls. Controls remain disabled while the target
is running or collecting.

Settings includes desktop CLI connection status, restart and reset actions. It does not display credentials or legacy client configuration templates.

Settings also exposes game gateway status, port, private IPv4 adapter
selection, LAN broadcast enablement, advertised endpoints, connected count,
restart control, and an explicit unauthenticated-LAN warning. Tasks display
process and connection states separately. Logs group managed and external
sources; external sources expose no process or MCP controls.

The archive transfer view uses an ordered target/source/action workflow.
Unavailable later steps remain disabled, progress dimensions stay stable, and
active transfers expose an icon cancellation control.

## Dependencies

May depend on foundation and feature public components. Feature modules must not
import shell internals.

## Validation

Run frontend tests, both locale completeness checks, production build, and
viewport screenshot verification, including destructive action layout,
task-tab layout, workflow horizontal overflow, popover/drawer layout,
conversation rename, explicit provider switching, terminal sizing, and
unavailable/compatibility Codex and Grok presentation.

## LLM Maintenance Rule

### Split action panel (2026-10-12)
Task header, action panel, workflow summary and tabs are non-scrolling flex siblings.
Only the remaining tab content scrolls. Notices take a bounded natural-height row,
so a pending answer or saved-operation notice cannot push a 100%-height terminal
below the viewport. Container queries reflow the guide based on actual task-pane
width; title/instruction/button sizes are 24/16/15px at normal size.
Short windows compact the task header and show only the workflow summary with its
existing detail-drawer entry, retaining content space. Pending badges have text and
distinct stage/terminal colors. No absolute-position overlay covers the terminal.

When changing navigation, localization, shared controls, or layout behavior,
update this Skill in the same change.

## Conversation controls
The terminal toolbar includes Copy all conversation history for both Codex and
Grok. Keep its accessible name stable while copying; show localized pending,
success (live status), empty-history and failure feedback. It remains available
in both terminal/history views and after terminal exit.

Codex conversation rows include archive/restore in addition to rename/delete; reserve room for all three controls. Active/archived filters and explicit synchronization sit above the list. Add bilingual clipboard-ready/size errors and archive labels. Grok retains its existing lifecycle controls.


## Stage workspace update

Button descriptions now default above the owner, use measured viewport clamping and support titled text buttons as well as icons. Production stage navigation wraps at narrow widths and stays visible while scrolling. Terminal settings wrap without reducing the terminal to a fixed-height toolbar.

## Acceptance and feedback UI

Production and instance surfaces expose a task-version acceptance button with progress and actionable errors. Visual cards identify reference/mockup/render/gameplay, temporary resources and plan bindings; at most two selected previews compare side by side.
User feedback supports related visuals and bounded screenshot/video uploads. AI recheck and user closure remain separate controls. New human-feedback tasks have no 8+4 progress wording; legacy rounds remain visible.

## Modal interaction update (2026-10-12)
Modal focuses its container once, traps keyboard focus inside the topmost dialog,
handles Escape without submitting, and restores the prior connected focus target.
Nested image previews return to the review dialog. Background terminal attachment
cannot steal focus from an active dialog. Review decisions are never preselected.
Question navigation, sticky action rows and responsive review columns remain usable
in small windows and at increased display scaling. Document text is rendered through
React text nodes, never executable HTML. State guidance uses text plus icons and color.

## Resume error visibility (2026-10-13)
Failed Codex attachment uses a red, wrapping, accessible recovery panel above the
terminal. Keep its diagnostic number readable, cap its height with scrolling, and
reserve the terminal's remaining grid row. Physical connection and task execution
labels appear separately in the toolbar. The retry action reads 接回原对话.

## Workspace preflight warning (2026-10-13)
Connected terminal startup warnings appear as a wrapping amber notice with bounded
scrolling above the task-status text. Keep terminal input and native execution controls
available. A warning describes an earlier probe, not approval or successful tool execution.

## Background task notifications (2026-10-14)

App subscribes before reading pending notification activations and acknowledges consumed IDs; StrictMode/late unmounts cannot lose activation. Test clicks open settings, missing tasks show feedback, other clicks pass an immutable target to TasksView. Only native click activation restores/focuses the app. Background delivery never steals focus. Bilingual settings/reasons are in i18n. Exit cleanup stops the monitor before providers, preserving normal close behavior.
