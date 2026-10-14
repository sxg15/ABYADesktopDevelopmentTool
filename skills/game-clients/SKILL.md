# Game Clients

## Purpose

Manage sxg15/PlatverseOutput GitHub Releases and install user-selected Windows Unity clients.

## Ownership

Own GitHub account authorization, Release/asset CRUD, downloads, safe ZIP extraction,
installation records, remote selection validation and advisory update checks. All network,
archive and credential work stays in Rust. No desktop CLI/MCP access is added.

## Public Contracts

Typed Tauri commands expose account metadata, releases/assets, installation records,
operation progress/cancellation, and optional update candidates. Login accepts a user's
Token in a password field; never expose it again, log it, or put it in a URL. Read-only
Tokens need Contents read for the fixed private repository; management needs Contents
write and repository write access. Classic Tokens with the repo scope are also accepted;
offer this fallback for collaborators who cannot select the repository in a fine-grained
Token, explain its broader scope and recommend expiration. Repository permissions describe
the account; GitHub still authoritatively checks the Token on every operation.
Validate /user and repository access before login.
Remembered tokens use current-user Windows DPAPI with a distinct purpose; session-only
tokens stay in native memory. Logout removes ABYA credentials only. Browser OAuth remains
future work until a dedicated registered OAuth application is supplied.

Release writes are explicit user actions; immutable releases cannot change. Uploads never
silently replace an asset. Deletion confirms the exact release/asset in the UI and does not
delete a Git tag. Bound JSON and paginate lists; re-fetch asset membership before mutations.
Only fixed GitHub API hosts receive Authorization. Signed asset redirects use an allowlisted
HTTPS GitHub CDN with no Authorization header. Errors contain codes, never response bodies,
credentials, signed URLs or request debug output. Cancellation of uploads may leave a remote
asset; refresh the release before retrying or deleting an incomplete asset.

Installation accepts complete Windows Unity ZIP packages up to 2 GiB, at most 100,000
entries and 16 GiB expanded. Reject traversal, drive/UNC/ADS paths, Windows device names,
case-insensitive collisions, links and reparse-point destinations. Stream bytes, verify
expected length and GitHub SHA-256 when present, extract into a new task-owned staging
directory and atomically rename after success. Only Unity EXEs with matching _Data and
UnityPlayer.dll are selectable; multiple Players require an explicit choice. Keep older
installations; do not overwrite running clients, user directories or task-pinned Players.
Failed operations remove only their own fresh staging directory. An app crash can leave
a hidden staging directory, never a selectable partial installation.

Settings persist local/remote source, previous local path, storage root, installation ID,
relative EXE and resolved gameExecutablePath. Installation records retain their storage
root and use normal drive paths for UI and Unity launches; filesystem checks still canonicalize.
Legacy settings default to local. Remote
selection resolves only a recorded complete installation and is saved explicitly after
download. Existing task-pinned Player and LAN Host selection retain precedence. Installation
alone and update checks never change the active Player or stop instances. Updates compare
published versions on the selected stable/prerelease channel; a replaced asset in the same
release also qualifies. Background checks run at startup and every 30 minutes while remote
source is active. Failures preserve offline installed-client use; manual refresh shows errors.

## Dependencies

Foundation database/settings/DPAPI, reqwest, zip, sha2, native filesystem. React consumes
typed foundation adapters; shell composes views and settings. No dependency on game processes.

## Validation

Run npm run check, cargo fmt --check, Clippy -D warnings and Rust tests. Cover settings
migration, private authentication/errors, pagination, member validation, path/link/bomb
rejection, integrity failure/cancellation, multiple EXEs, persisted selection and update
channel rules. Use isolated files and loopback mock API fixtures for write/install tests;
never create or delete real repository Releases just for testing. Check actual GitHub reads
separately, and label a fixture UI preview distinctly from native end-to-end validation.
The ignored live_github_account_and_release_reads test requires ABYA_TEST_GITHUB_READS=1
and an already authenticated gh CLI. It borrows the credential in native test memory,
uses session-only login against an isolated database, performs /user/repository/release
reads, signs out and prints counts only. It does not upload/delete Releases or create keys.

## LLM Maintenance Rule

Update this Skill, shared settings/app-shell/desktop-cli Skills and changelog with contract
changes. No automatic release deletion, version switch, local client deletion or forced update.
