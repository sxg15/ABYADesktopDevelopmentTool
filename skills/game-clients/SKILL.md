# Game Clients

## Purpose

Manage sxg15/PlatverseOutput GitHub Releases and install user-selected Windows Unity clients.

## Ownership

Own GitHub account authorization, Release/asset CRUD, downloads, safe ZIP extraction,
installation records, remote selection validation and advisory update checks. All network,
archive and credential work stays in Rust. No desktop CLI/MCP access is added.

## Public Contracts

Typed Tauri commands expose account metadata, releases/assets, installation records,
operation progress/cancellation, and optional update candidates. GitHub login/logout lives
in Settings. An unsigned manager shows the missing-login state and a button that navigates
to and focuses the GitHub settings section; the manager contains no credential form.
Settings refreshes remote choices after login/logout without saving or changing the Player.

Use GitHub OAuth Device Flow with public client ID Ov23li0rDQSvLA8yT7sj and repo scope for
the private repository. No client secret is used or distributed. The UI explains repo's
broader account scope, opens only https://github.com/login/device, and receives only an
opaque attempt ID, user code, verification URI, expiration and minimum polling delay.
Device/access/refresh secrets never cross Tauri, appear in URLs/logs or frontend storage.
Native start/poll/cancel commands enforce polling intervals, slow_down, expiration, denial,
one in-flight exchange and generation checks. Cancel/logout/replaced attempts cannot be
restored by a late exchange. Reopening settings resumes the current in-memory attempt.
Validate /user and repository access before accepting each new authorization.

Before access-token expiry, serialize refreshes and rotate access/refresh credentials in
one encrypted record. Device-flow refresh needs no client secret. Invalid/expired refresh
credentials clear login and request reauthorization; transient errors preserve credentials
and offline installations. Remembered credentials use current-user Windows DPAPI with a
distinct purpose; session-only credentials stay in native memory. Read old encrypted_token
records for compatibility, but expose no manual-token login command or field. Logout removes
ABYA credentials only. GitHub authoritatively checks repository authorization per operation.

Release writes are explicit user actions; immutable releases cannot change. Uploads never
silently replace an asset. Deletion confirms the exact release/asset in the UI and does not
delete a Git tag. Bound JSON and paginate lists; re-fetch asset membership before mutations.
Only fixed GitHub API hosts receive Authorization. Signed asset redirects use an allowlisted
HTTPS GitHub CDN with no Authorization header. Errors contain codes, never response bodies,
credentials, signed URLs or request debug output. Cancellation of uploads may leave a remote
asset; refresh the release before retrying or deleting an incomplete asset.

Each API request has exactly one Accept value: application/vnd.github+json for metadata
and application/octet-stream for asset bytes. Never append binary Accept to a JSON request;
reqwest appends header values and GitHub can return metadata instead of the archive.
Handle direct 200 streams and allowlisted redirects. Reject JSON, XML and text responses
with clientDownloadResponse before passing bytes to the installer; integrity checks remain
mandatory for accepted binary responses.

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
uses a test-only session initializer against an isolated database, performs /user/repository/release
reads, signs out and prints counts only. It does not upload/delete Releases or create keys.
HTTP regression fixtures must assert the complete Accept header list, not substring presence,
and reject non-file response types without exposing bodies. The ignored
live_github_download_and_install test requires ABYA_TEST_GITHUB_INSTALL=1 plus explicit
ABYA_TEST_GITHUB_RELEASE_ID/ABYA_TEST_GITHUB_ASSET_ID. It uses the existing gh credential
only in memory, downloads the real selected ZIP through the production service, checks the
GitHub digest and Unity executable discovery in an isolated temporary database/directory,
verifies installation persistence without changing Player settings, then removes its own
fixture. It performs no remote writes, saves no credentials and never launches the game.
OAuth fixtures cover interval/slow_down, expiry/denial, missing repo permissions, secret
non-disclosure, DPAPI restart, concurrent refresh and cancellation/logout races. The ignored
live_oauth_device_flow_configuration test requires ABYA_TEST_GITHUB_OAUTH=1; it verifies the
public app configuration, then cancels without exposing codes or authorizing a user. Browser
authorization by the account owner is a separate acceptance step; do not automate consent.

## LLM Maintenance Rule

Update this Skill, shared settings/app-shell/desktop-cli Skills and changelog with contract
changes. No automatic release deletion, version switch, local client deletion or forced update.
