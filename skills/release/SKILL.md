# Release Skill

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
