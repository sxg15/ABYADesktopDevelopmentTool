# Game Clients

## Purpose

Manage sxg15/PlatverseOutput GitHub Releases and install user-selected Windows Unity clients.

## Ownership

Own GitHub account authorization, Release/asset CRUD, downloads, safe ZIP extraction,
installation records, remote selection, single-package retention and optional update prompts. All network,
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
UnityPlayer.dll are selectable; multiple Players require an explicit choice. A download
creates one pending complete installation. The APP workflow stays mounted across navigation,
then calls activate with the sole EXE or the user's explicit choice. Activation saves only
client selection against fresh settings, preserves the local fallback and unrelated values,
and never launches/stops a game. Failure/cancellation retains the previous selection. Cancel
after extraction discards the pending choice; a later maintenance pass removes that unused
package. Reuse an already complete record for the same asset ID/update timestamp.
Failed operations remove only their own fresh staging directory. An app crash can leave
a hidden staging directory, never a selectable partial installation.

Settings persist local/remote source, previous local path, storage root, installation ID,
relative EXE and resolved gameExecutablePath. Installation records retain their storage
root and use normal drive paths for UI and Unity launches; filesystem checks still canonicalize.
Legacy settings default to local. Remote
selection resolves only a recorded complete installation. The explicit Download and use / Update
now action includes activation and saving; no second settings visit or Save is required.
Manual source/local-path edits still use Settings Save. Existing task-pinned Player and LAN
Host selection retain precedence. Checks never change the active Player or stop instances. Updates compare
published versions on the selected stable/prerelease channel; a replaced asset in the same
release also qualifies. Background checks run at startup, window focus, after sign-in and
every 30 minutes while the saved source is remote. Open a dismissible global dialog with
Update now and Not now. A candidate key includes release/asset identity, update time and
digest; suppress repeats within this APP session, but allow manual reopening and changed
candidates. Same-name assets are preferred; multiple unmatched ZIPs need an explicit choice.
Reject late checks after source/selection changes. Failures preserve offline use.

game_client_activate, game_client_discard_pending and game_client_maintain are APP-only typed
commands. Maintenance runs at startup, after activation/cancellation and every minute, keeping
the selected installation even in local mode plus the current pending download. Remove only
unselected recorded UUID directories at <storage>/PlatverseOutput/<release>-<asset>-<uuid>,
after matching the bounded .abya-client.json marker and checking the full tree for links.
Never recursively delete storage roots, arbitrary folders or unmarked partial staging paths.
Collect current task Player pins through TaskService.pinned_player_paths and running paths
through InstanceService.running_executable_paths while holding foundation EXECUTABLE_FILES
exclusively. Launch/version capture hold shared guards. Referenced local fallback EXEs are
also protected. Defer task/running/local/unavailable packages, expose reasons in both views,
and retry without changing task pins. A corrupt usage snapshot prevents all deletion.
Existing unselected installations and abandoned complete downloads are eligible on restart;
arbitrary unmarked directories are not. An unused record is removed only after its owned
directory is gone. This is not a replacement for task-version registration or acceptance.

## Dependencies

Foundation database/settings/DPAPI/executable-file gate, reqwest, zip, sha2, native filesystem.
React consumes typed foundation adapters; shell composes the persistent workflow provider,
views and settings. Tauri composes the public task-pin and running-process snapshots; the
client module never queries task/instance tables or controls their lifecycle.

## Validation

Run npm run check, cargo fmt --check, Clippy -D warnings and Rust tests. Cover settings
migration, private authentication/errors, pagination, member validation, path/link/bomb
rejection, integrity failure/cancellation, multiple EXEs, persisted selection and update
channel rules. Use isolated files and loopback mock API fixtures for write/install tests;
never create or delete real repository Releases just for testing. Check actual GitHub reads
separately, and label a fixture UI preview distinctly from native end-to-end validation.
Retention tests cover cross-root removal, unchanged unrelated settings, task/running/local
and pending protection, explicit EXE selection, restart recovery, tampered markers and
nested junction rejection before deletion. UI tests cover direct consent from the task
workspace, one prompt per candidate, same-release asset replacement, duplicate clicks,
navigation during download, failure, cancellation and stale checks after source changes.
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
changes. No automatic remote Release deletion or forced update. User-authorized selection
includes removal of unused managed older packages, with the usage/ownership guards above.
