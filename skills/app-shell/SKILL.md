# App Shell Skill

## Purpose

Own the desktop navigation, bilingual localization, layout, notifications, and
shared visual primitives.

## Ownership

Owns page composition but not task, process, log, or MCP business logic.

## Public Contracts

Navigation view IDs, locale keys, shared UI primitives, and application-level
refresh events.

The first screen is the operational development-task workspace. Navigation has
`tasks`, `archiveTransfer`, `logs`, and `settings` views. Chinese and English
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

Settings includes a localized MCP client preset selector, read-only
configuration text area, configuration-path hint, and copy command. Client
brand names and configuration syntax remain in the owning desktop-mcp feature;
shell localization owns only generic labels and notifications.

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

When changing navigation, localization, shared controls, or layout behavior,
update this Skill in the same change.
