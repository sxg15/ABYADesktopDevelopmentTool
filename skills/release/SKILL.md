# Release Skill

## Purpose

Own repeatable checks and portable Windows release output.

## Ownership

Owns dependency scripts, Tauri release configuration, `Publish` layout, build
manifest generation, project LLM content publishing, and repository policy
checks.

## Public Contracts

`npm run check`, `npm run build:portable`, the release executable, and
`Publish/build-manifest.json`. The project-root `.codex/` and `.grok/`
directories are required and their complete contents are copied to matching
directories in portable output. Both source directories must contain
`skills/abya-game-development-task/SKILL.md`; checks and publishing fail when
either provider's bundled task Skill is absent. The published managed Skill
also includes its gameplay architecture reference and versioned JSON template.

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
task font requirements and the post-feasibility art choice. Checks validate
both namespaces and provider parity; importer and task deployment tests cover
both kinds. The comic-arcade-ui bundle includes its complete source artwork,
preview HTML, PNG kit, design tokens, references and optional generation script.

`check:skills` verifies both importer entries and template identities/provider
metadata parity and runs the importer behavioral tests. Zero templates is valid.
Publishing copies all template resources through the existing complete-root
copy, and fails on missing importer resources or malformed template metadata.

## Dependencies

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
