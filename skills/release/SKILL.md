# Release Skill

## Current scope

Workflow 2.9 enforces reviewHandoffVersion=1: unresolved ordinary issues, missing/failed checks and unfinished work stay in Lua with bounded automatic handoff-repair. Only confirmed abya-source diagnosis bound to current Player/evidence may cross; blocked checks link blockedByIssueIds. Core package does not waive this gate. Playtest shows cross-stage source-defect cards and selected diagnostic ZIP; upgrades preserve original IDs, approvals, budgets and packages and return premature legacy handoffs to Lua without starting execution.

Workflow 2.8 adds a scoped completion loop: current-goal dependency closure, evidence-based wake signals, audited work revision, automatic repair plans, per-output finalization and honest repairRequired/externalBlocked outcomes. No candidate completion without a hash-bound playable ZIP and current lua/input/core-loop/save-reload evidence. Fixture validation remains separate. Upgrade preserves budgets, IDs and approvals; explicit resume retains the original goal and cannot replay uncertain writes. Package compression runs outside the database mutex.

Workflow 2.7 release validates design-without-Player, retained human/material gates, isolated authorization expiry/revocation, runtime contract diagnostics, migration and prior transport regressions. Native/model evidence is distinguished from synthetic fixtures. A complete-game 3–4 hour result requires measured game pilots; unit tests do not prove it. Match the Player contract to its actual build before distributing that Player. Preserve the official package until candidate validation and no running destination process.

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
`Publish/build-manifest.json`. The project-root `.codex/` directory is required
and its complete contents are copied into portable output. It must contain
`skills/abya-game-development-task/SKILL.md`; checks and publishing fail when
the bundled task Skill is absent. The published managed Skill
also includes its gameplay architecture reference and versioned JSON template.
The Codex bundle additionally require the Bot workflow/artifact references,
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

The Codex root also ship `abya-import-task-template/SKILL.md` and its
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
both namespaces and resource completeness; importer and task deployment tests cover
both kinds. The comic-arcade-ui bundle includes its complete source artwork,
preview HTML, PNG kit, design tokens, references and optional generation script.

`check:skills` verifies the importer entry and template identities/metadata and runs the importer behavioral tests. Zero templates is valid.
Publishing copies all template resources through the existing complete-root
copy, and fails on missing importer resources or malformed template metadata.

## Dependencies

Workflow content has one canonical source in .codex. check:skills verifies
policy identities, templates, resource links and the complete design trees.
There is no provider-mirroring script or sync:workflow command. Task-service
tests verify real deployment and preserve existing pinned task records.
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
opens, the Codex provider choices render in the packaged application,
the complete project-root `.codex/` contents appear under `Publish/.codex`,
and the published bundle contains the managed ABYA
task Skill and architecture resources, stale published LLM files are removed on repeat builds, no
authentication/session/log/cache content is introduced, and the manifest
checksum matches.

The publish script computes SHA-256 through .NET cryptography APIs so it also
works in Windows PowerShell environments where `Get-FileHash` is unavailable.

## LLM Maintenance Rule

When changing build commands, dependencies, release contents, or validation
policy, update this Skill in the same change.

Pure CLI releases additionally ship abya-desktop.exe, tools/abya and runtime/node.exe (Node 20+). The publish script runs Rust formatting, Clippy and tests, builds the CLI, then builds the existing desktop application. The embedded Abya CLI is a versioned source snapshot; refresh and test it with every runtime CLI change.


## On-demand history

Read the matching section in [historical contracts](../../docs/skill-history/20261026/release.md) only when changing that surface. New current-scope rules override historical versions.

Workflow continuity 2.6 (2026-10-26), Autonomous recovery 2.5 (2026-10-24), Terminal freeze recovery (2026-10-22), Automatic progress and Markdown (2026-10-22), Memory warning release (2026-10-22), v5 workbench (2026-10-21), Approved art recovery release (2026-10-20), Game UI design reference library (2026-10-20), Reference conversion and collage specimen (2026-10-20), Reliable pause/interruption (2026-10-20), Art implementation calibration (2026-10-20), Current-turn interruption (2026-10-19), One-click terminal continuation (2026-10-19), 内存保护与追溯（2026-10-19）, Compact task guidance package (2026-10-19), Codex-only package (2026-10-19), Task detail scrolling (2026-10-19), Contextual visual refinement (2026-10-17), Game client system proxy (2026-10-17), Retained UI refinement and integrated library (2026-10-17), Game art design workflow 2.2 (2026-10-16), Unified art workflow (2026-10-16), Desktop workspace styling (2026-10-16), Remote client update flow (2026-10-16), Task header description (2026-10-16), Runtime instance discovery (2026-10-16), Programmer issue Markdown export (2026-10-15), Required development branch order (2026-10-15 clarification), Art reference upload (2026-10-15), Game client integration into dev (2026-10-15), Long conversation transport release (2026-10-15), Dev acceptance UI fix (2026-10-14), Programmer alert release (2026-10-13), Game client download fix (2026-10-15), Game client manager (2026-10-14), Terminal IME release (2026-10-13), Session recovery release (2026-10-13), Sandbox regression validation, Conversation integration validation, Stage workspace update, Workflow 2.0 release, Workflow 2.1 interaction release, Conversation startup release (2026-10-13), Background task notifications (2026-10-14), Issue repair release (2026-10-13), Task isolation and subprocess resume release (2026-10-22), Workbench pane sizing (2026-10-22), Autonomous implementation 2.4 release (2026-10-23).
