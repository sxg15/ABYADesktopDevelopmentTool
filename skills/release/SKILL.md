# Release Skill

## Art reference upload (2026-10-15)
Default/Validation builds use art-reference-upload-20261015. Validate the real
resources-node upload/paste/preview controls, task/revision isolation and persistent
reference files without approval bypass, then build and verify the complete package.
This task is developed directly on dev and synchronized to dev per its user request.
Preserve remote game client management and all existing terminal/acceptance fixes.

## Game client integration into dev (2026-10-15)

Default/Validation builds now use game-client-dev-20261015. Retain remote client
management, settings OAuth and binary asset downloads together with dev's bounded
Codex history transport, acceptance UI lifetime, issue repair and notifications.
Resolve distribution conflicts by rebuilding from the combined source, never by
selecting one branch's executable. Run all required checks, verify the full candidate
inventory and a relocated package, then promote and verify remote dev contains the
completed game-client branch. The CLI-to-dev policy below remains applicable to CLI work.

## Long conversation transport release (2026-10-15)
Default/Validation builds use codex-large-history-fix-20261015. Validate the bounded
128 MiB Codex WebSocket transport with large single-frame and fragmented responses,
explicit oversize diagnostics, normal checks and the complete portable inventory.
CLI completion automatically includes integration into dev: fetch remote state,
preserve both branches' features, prefer fast-forward, merge divergent work and
rerun affected checks, then push CLI/dev and verify origin/dev contains the completed
CLI commit. Include source, Skills, constraints and verified Publish files. No
additional confirmation is required. Never force-push or update main; report real
conflicts or remote failures. This supersedes all historical dev-only rules below.

## Dev acceptance UI fix (2026-10-14)
Default/Validation builds use acceptance-ui-fix-20261014. Integrate dev notifications
with the prior programmer alerts and AI repair without reverting either. All source
and complete Publish changes now follow the CLI-to-dev policy above. Validate
stage-switch/remount single-card behavior and slow/hidden polling, all normal checks,
the isolated portable package and a fresh dev checkout in a relocated path.

## Programmer alert release (2026-10-13)
Default/Validation builds use programmer-alert-20261013. Sync the updated canonical
production-records reference to Grok and ship both bundles. Verify confirmed/legacy
suspected classifications, ordinary Lua exclusions, resolved alerts and responsive
red guidance. Keep existing task pins and previous terminal fixes intact.

## Game client download fix (2026-10-15)

Default/Validation packages use game-client-download-fix-20261015. Fix binary download
content negotiation and distinguish non-file responses from integrity failures. Run the
strict Accept-header regression and the opt-in real ZIP installation smoke documented in
game-clients, in addition to the required checks. Verify the complete candidate inventory
before promoting; preserve all user settings, credentials and existing game installations.

## Game client manager (2026-10-14)

The game-client-oauth-20261014 build adds settings-based browser OAuth with public client ID
Ov23li0rDQSvLA8yT7sj, automatic credential refresh, cancellation and no client secret.
Validate native manager-to-settings navigation and the one-time-code/browser handoff;
only the user completes GitHub consent. OAuth configuration and race fixtures are documented
in game-clients. The previous game-client-management-20261014 build introduced APP-only GitHub Release management and
local version installation. Build the Validation package first, run the fixture UI and
isolated installation regressions, then distribute the complete verified Publish package.
scripts/game-clients-preview.html mounts the actual client manager/settings components
against explicitly marked in-memory fixtures; it never uses account credentials or makes
GitHub writes. The optional native GitHub read smoke is documented in game-clients.
scripts/test-game-clients-native.mjs init/report uses a fresh marker-owned directory for
native settings migration, offline installed-client selection and restart checks. Its EXEs
are non-runnable sentinels and must never be launched. Use ABYA_TEST_MODE=1, the returned
isolated LOCALAPPDATA/WebView directories, and free ports 43921/43922. Keep USERPROFILE
normal. The report excludes desktop credentials and verifies no GitHub credential row.
Keep Tokens, installed game packages, account databases and signed URLs outside distribution.

## Terminal IME release (2026-10-13)
Default and Validation packages use terminal-ime-fix-20261013 and retain the
session recovery fixes. scripts/ime-preview.html provides an isolated real xterm
fixture comparing old/fixed composition anchors across fragmented cursor redraws,
with one Chinese commit and uninterrupted output. These synthetic events do not
replace manual validation of vendor-specific Windows candidate windows. Verify it,
full checks and a relocated portable checkout before synchronizing distribution.

## Session recovery release (2026-10-13)
Default and Validation builds use session-recovery-20261013. Ship the complete
tracked Publish package after checks. test-codex-writer-recovery.mjs is an opt-in
two-backend native integration test requiring ABYA_TEST_CODEX and a disposable
ABYA_TEST_WORKSPACE. It creates only a fresh test thread, proves active-writer
rejection then same-ID recovery after its own first backend exits, starts no model
turn and archives only its own test history. Never apply it to a user's thread ID.

## Purpose

Own repeatable checks and portable Windows release output.

## Ownership

Owns dependency scripts, Tauri release configuration, `Publish` layout, build
manifest generation, project LLM content publishing, and repository policy
checks.
Owns the minimal vendored portable-pty 0.9.0 patch and retained MIT license in src-tauri/vendor.
It removes only Windows cursor inheritance for independent embedded terminals. Keep provenance,
review the patch on dependency upgrades, and run the no-frontend-cursor-reply regression test.

## Public Contracts

### Repository distribution (2026-10-13)
Track the complete official Publish package and the root ABYA 开发工具.cmd launcher.
The launcher resolves paths relative to itself, uses bundled Node for a quick package
check and starts the GUI through Explorer. --check verifies every inventory hash.
Do not share developer-specific .lnk files, user databases, account state or credentials.
Publish has a path allowlist in .gitignore; package-files.json lists all runtime files.
portable-package.mjs rejects private/unexpected files, links, traversal, missing files,
hash mismatches and oversized normal-Git payloads. --require-tracked catches ignored
or forgotten release files. Publish bytes use -text attributes to survive clone intact.
All completed CLI source and verified distribution changes must also reach dev,
under the CLI-to-dev policy above (2026-10-15). Do not update main.
Validate a fresh checkout in a different path, including spaces/non-ASCII characters.
The application is ready to open without build tools; per-user AI login, game Player
selection and the Windows WebView2 dependency remain machine prerequisites.
Rebuilds regenerate the inventory. Staging packages, generated logs and superseded
release directories remain disposable; recycle only inspected inactive paths. Retain
database recovery snapshots and current user state outside Git.
Distribute Node's matching upstream LICENSE with its executable. If the installed
Node directory lacks LICENSE, the checked-in runtime license can be reused only
when the package's nodeVersion exactly matches the selected Node executable version.
The initial v24.11.1 license comes from nodejs/node tag v24.11.1, file LICENSE.

publish.ps1 -Validation builds Publish-Staging-Validation without replacing an active package.
Historical staging switches remain mutually exclusive development destinations. Every build embeds
version from the package (currently 0.3.0), release stage-workspace-20261009 and UTC build time; manifest workflow version is
read from the UTF-8 policy (1.3.0). Live processes under the destination block replacement.
After isolated validation, promote-validated.ps1 verifies APP/CLI hashes, stages a complete copy,
backs up Publish, atomically renames the replacement and creates ABYA 开发工具.lnk through Explorer.
Keep release backups; promotion never copies or replaces user databases. The stable user entry is
the shortcut/Publish, not a staging directory. Real-model tests use isolated data/workspaces and
ABYA_TEST_MODE=1. Preserve pinned user Skills unless explicitly upgrading through TaskService with
backups. Real-model test output contains identifiers and metrics only; credentials stay in memory.

`npm run check`, `npm run build:portable`, the release executable, and
`Publish/build-manifest.json`. The project-root `.codex/` and `.grok/`
directories are required and their complete contents are copied to matching
directories in portable output. Both source directories must contain
`skills/abya-game-development-task/SKILL.md`; checks and publishing fail when
either provider's bundled task Skill is absent. The published managed Skill
also includes its gameplay architecture reference and versioned JSON template.
Both provider bundles additionally require the Bot workflow/artifact references,
plan/report schemas and templates, and all four validator modules. Publishing
preflights these files and copies them with the complete provider root.
check:skills runs scripts/bot-workflow.check.mjs: update-scope preservation,
numeric enum readback compatibility, stale/missing/foreign evidence rejection,
independent-client and difficulty coverage, current archive/Player fingerprints,
CLI exit codes and provider resource parity. These synthetic checker tests are
not Bot gameplay or real Player acceptance evidence.
`scripts/publish.ps1 -Staging` runs the same checks/build/copy/manifest pipeline
into the fixed repository-local Publish-Staging-Bot directory, allowing package
validation while the normal Publish executable is in use. The default remains
Publish; both destinations retain the same resolved-path deletion boundary.

Both provider roots also ship `abya-import-task-template/SKILL.md` and its
standalone Node `scripts/import-template.mjs`. Import accepts an original Skill
folder, destination tool root, slug, display name and summary. It preserves
resources and optional frontmatter, rewrites name/description and `$old-name`
references, and writes an `abya-task-template.json` v1 identity marker.
Template IDs use `abya-task-template-<slug>` (at most 64 lowercase ASCII
letters/digits/hyphens). Both destinations are preflighted and staged before
installation, with rollback on installation failure. Source links, overlapping
paths, nested provider/git/dependency folders and unmarked collisions fail.
Same-source imports update idempotently; a different source needs `--replace`.
The original source is never modified or executed. Import into repository
roots for durable releases; update an existing Publish separately when needed.
Direct portable imports survive until a subsequent source-based release.

The importer additionally accepts `--kind art` (default remains `task`) for
`abya-art-template-<slug>` and `abya-art-template.json`, kind
`abya-art-template`. Both kinds share preflight, identity protection, staging,
rollback and binary resource preservation. Imported art instructions defer to
task font requirements and the art choice confirmed in the execution plan. Checks validate
both namespaces and provider parity; importer and task deployment tests cover
both kinds. The comic-arcade-ui bundle includes its complete source artwork,
preview HTML, PNG kit, design tokens, references and optional generation script.

`check:skills` verifies both importer entries and template identities/provider
metadata parity and runs the importer behavioral tests. Zero templates is valid.
Publishing copies all template resources through the existing complete-root
copy, and fails on missing importer resources or malformed template metadata.

## Dependencies

First-batch workflow content has a canonical source in .codex. npm run sync:workflow
updates only the listed repository .grok resources, including importer routing and
the two existing templates, while keeping Grok-specific entry metadata. It does
not update installed Publish directories or user task workspaces. check:skills
rejects stale copies and checks policy identities, all pilot rounds/questions,
blank initial acceptance/evidence and relative resource links. The task-service
tests verify real create/refresh deployment and preserve existing task records.
Production resources in assets/production and conditional references ship
through the existing complete-directory copy. These checks do not run an AI model
or establish gameplay acceptance. Backend production gates have separate Rust
tests; UI approval and record browsing have frontend tests.

scripts/setup-recorder.ps1 downloads the pinned BtbN LGPL shared FFmpeg archive,
checks its published SHA-256 and preserves the complete license/source manifest.
The runtime is an ignored reproducible dependency under tools/ffmpeg/runtime;
publishing requires it and copies it with source.json into the portable tools
directory. -WorkflowStaging writes Publish-Staging-Workflow, preserving the
in-use normal Publish executable and its resources. It is mutually exclusive
with the older -Staging switch. Validate the bundled ffmpeg/ffprobe and the
native owned-window smoke test separately from a real Unity gameplay task.

May invoke frontend and Rust checks. It must not contain product business logic.
Cargo dependency changes for the LAN WebSocket gateway must remain compatible
with the portable no-bundle release. The embedded terminal adds the
`portable-pty` Rust dependency and `@xterm/xterm` plus `@xterm/addon-fit`
frontend packages. Windows Codex discovery also uses `winreg` to read current
persisted PATH values when the launching shell has a stale environment. These
dependencies and frontend assets must remain bundled into the same single
portable executable.

## Validation

Verify the release build succeeds, no installer is generated, the executable
opens, the Codex and Grok provider choices render in the packaged application,
the complete project-root `.codex/` and `.grok/` contents appear under matching
`Publish` directories, both published provider roots contain the managed ABYA
task Skill and architecture resources, stale published LLM files are removed on repeat builds, no
authentication/session/log/cache content is introduced, and the manifest
checksum matches.

The publish script computes SHA-256 through .NET cryptography APIs so it also
works in Windows PowerShell environments where `Get-FileHash` is unavailable.

## LLM Maintenance Rule

When changing build commands, dependencies, release contents, or validation
policy, update this Skill in the same change.

Pure CLI releases additionally ship abya-desktop.exe, tools/abya and runtime/node.exe (Node 20+). The publish script runs Rust formatting, Clippy and tests, builds the CLI, then builds the existing desktop application. The embedded Abya CLI is a versioned source snapshot; refresh and test it with every runtime CLI change.

## Sandbox regression validation
The release includes the named-pipe CLI and Windows security API dependencies. scripts/test-codex-pipe.mjs is an opt-in model-backed test launched by sandbox_pipe_authentication_smoke; it uses existing permissions, refuses approval requests, archives its native test thread, and prints only sanitized pass/fail evidence. Supply ABYA_TEST_CODEX and ABYA_TEST_WORKSPACE; do not store session credentials in scripts or manifests.


## Conversation integration validation
scripts/test-codex-conversation-lifecycle.mjs validates installed native project APIs and archive/restore using temporary test data, without model inference. scripts/sync-codex-projects.mjs performs explicit historical organization using only Desktop-listed tasks and locally recorded native IDs; it never resumes, archives or deletes user conversations. Neither script writes Codex databases or credentials. Validate native clipboard feature dependencies in the Windows release.


Historical organization may also use explicitly supplied ABYA_HISTORY_WORKSPACES; it verifies retained task IDs, matches older native threads by exact cwd, and skips unavailable rollouts without recreating task records.

Recorder-only validation builds may use publish.ps1 -RecorderStaging. It uses the same checks/package pipeline in Publish-Staging-Recorder with release label window-recorder-fix-20261009, leaving Publish and existing Publish-Staging-Validation intact. Do not run promote-validated.ps1 against this destination: it promotes the separate Validation package. Switch to the recorder candidate explicitly only after closing the active APP; keep the normal data directory and approval history.

## Stage workspace update

Release 0.3.0 ships workflow 1.3.0 with stage workspace, native conversation settings and updated shared skills. Retain the terminal-input and recorder corrections already in 0.2.2. Use the Validation staging build and verify manifest hashes, frontend interaction, additive historical record compatibility and native settings before promotion.

## Workflow 2.0 release

Release 0.4.0 / acceptance-feedback-20261010 ships workflow 2.0.0, task-version acceptance launch, visual previews, user feedback and necessary self-tests. Explicit legacy upgrades retain approved requirements/plans and history; ordinary package updates do not upgrade pinned tasks.
Use Validation staging, run all standard checks, and verify real APP controls plus the known CatTrapClockFix Player. Human mouse acceptance and the 3–4 hour next-game target must not be inferred from automated validation.

## Workflow 2.1 interaction release
UI staging builds carry guidance-action-panel-20261012 so their manifest and embedded
release identity distinguish them from the prior acceptance-feedback package.
scripts/guidance-preview.html mounts real TasksView against isolated browser fixtures
for status, route and viewport checks; it never opens native sessions or user tasks.
Sync/check bundles include art-requirements.template.md and four approval gates
(requirements/resources/plan/delivery). Frontend, Rust and workflow tests cover art
confirmation and legacy policy compatibility. Source changes do not upgrade running
user task workspaces or replace the installed Publish package implicitly.

## Conversation startup release (2026-10-13)
Default/Validation packages use conversation-startup-fix-20261013, retaining recovery
and IME fixes. Run clean native delete/recreate and failed-probe isolation in addition
to regular checks. Preserve complete tracked Publish and validate a fresh checkout.

## Background task notifications (2026-10-14)

Default/Validation builds use background-notifications-20261014. Validate with ABYA_TEST_MODE=1 and isolated LOCALAPPDATA, workspace and WebView data directories plus free gateway/CLI ports; notification AUMID/CLSID/shortcut are distinct per test data directory. Keep USERPROFILE at its real value for native shell acceptance: redirecting it also redirects the Windows Programs known folder and can prevent shortcut registration. Do not start provider sessions in these notification fixtures. Native acceptance must cover actual toast and Action Center activation, name/icon, foreground/minimized/other-task waits, dedupe/restart, resolved/deleted/disabled/unreadable state, and relocation to spaces/Chinese paths. Test cold COM activation must exit before touching real data. Publish-Staging-Validation is an isolated output directory, not an automatic data sandbox; always supply the test environment when running acceptance. Preserve live Publish while acceptance is running.
scripts/test-task-notifications.mjs creates only a fresh marker-owned isolated SQLite fixture and supports versioned question/review, resolution/archive/deletion, toggle/corruption and sanitized receipt reports. It never accesses real data or provider accounts. Use a disposable logs directory, Node 24, and ports 43781/43782 after checking they are free.
Windows native opt-in acceptance: ABYA_TEST_WINDOWS_TOAST=1 cargo test isolated_windows_toast_registration_history_and_com --lib -- --ignored --nocapture. This displays one separate-identity test toast and verifies COM/history/removal/registration cleanup; it is not a substitute for actual shell-click UI acceptance.
isolated_portable_notification_activation is an ignored native client test restricted to the marker-owned ABYA_TEST_NOTIFICATION_FIXTURE. It reads that test identity history and optionally invokes its COM callback with ABYA_TEST_NOTIFICATION_KEY (test or an opaque key), allowing end-to-end portable navigation verification without posting task actions. Distinguish this from physical shell clicks.

## Issue repair release (2026-10-13)
Default/Validation use issue-auto-repair-20261013. Verify issue/native binding,
queued/deduplicated delivery, stale/programmer/resolved rejection and UI feedback.
The button uses the selected Codex conversation and never auto-resolves a defect.
Keep the complete portable distribution and earlier startup/IME/programmer alerts.
