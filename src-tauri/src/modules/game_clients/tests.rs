use super::auth::{AUTH_KEY, SECRET_PURPOSE, StoredAccount};
use super::*;
use crate::foundation::secrets;
use crate::foundation::{AppPaths, GameClientSelection};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    net::TcpListener,
    thread,
};

pub(super) struct Temp(pub PathBuf);
impl Temp {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("abya-clients-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn service(root: &Temp) -> GameClientService {
    let paths = AppPaths {
        data_dir: root.0.clone(),
        database_path: root.0.join("test.db"),
        bootstrap_logs_dir: root.0.join("logs"),
        reports_dir: root.0.join("reports"),
        archive_transfers_dir: root.0.join("transfers"),
        default_archive_root: root.0.join("archives"),
        default_workspace_root: root.0.join("workspaces"),
    };
    let database = Database::open(&paths).unwrap();
    let settings = SettingsService::new(database.clone(), paths.default_workspace_root);
    GameClientService::new(database, settings, root.0.clone())
}

pub(super) fn package(extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.add_directory("Windows/Game_Data/", options).unwrap();
    for (name, bytes) in [
        ("Windows/Game.exe", &b"MZfake-player"[..]),
        ("Windows/UnityPlayer.dll", &b"dll"[..]),
    ]
    .into_iter()
    .chain(extra.iter().copied())
    {
        zip.start_file(name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn release(id: u64, date: &str, prerelease: bool) -> ClientRelease {
    ClientRelease {
        id,
        tag_name: format!("v{id}"),
        name: None,
        body: None,
        draft: false,
        prerelease,
        immutable: false,
        published_at: Some(date.into()),
        assets: vec![ReleaseAsset {
            id: id * 10,
            name: "Windows.zip".into(),
            size: 1,
            digest: None,
            updated_at: date.into(),
            state: "uploaded".into(),
        }],
    }
}

pub(super) fn installed(
    service: &GameClientService,
    root: &Temp,
    bytes: Vec<u8>,
) -> ClientInstallation {
    let mut release = release(1, "2026-10-01T00:00:00Z", false);
    release.assets[0].size = bytes.len() as u64;
    release.assets[0].digest = Some(format!("sha256:{:x}", Sha256::digest(&bytes)));
    install::install(
        Cursor::new(bytes),
        &release,
        &release.assets[0],
        root.0.to_str().unwrap(),
        &Progress::default(),
        |record| service.write(&format!("{INSTALL_PREFIX}{}", record.id), record),
    )
    .unwrap()
}

#[test]
fn legacy_settings_remain_local_and_remote_selection_survives_restart_without_login() {
    let root = Temp::new();
    let service = service(&root);
    service
        .write(
            "application",
            &serde_json::json!({"gameExecutablePath":"C:\\Game\\Old.exe"}),
        )
        .unwrap();
    let legacy = service.settings.get().unwrap();
    assert_eq!(legacy.game_client.source, GameClientSource::Local);
    assert_eq!(
        legacy.game_client.local_executable_path,
        "C:\\Game\\Old.exe"
    );
    let record = installed(&service, &root, package(&[]));
    let mut input = PublicSettingsUpdate {
        game_executable_path: "untrusted.exe".into(),
        workspace_root_path: legacy.workspace_root_path.clone(),
        locale: "zh-CN".into(),
        game_gateway_port: 47610,
        preferred_adapter_id: String::new(),
        lan_broadcast_enabled: true,
        game_client: GameClientSelection {
            source: GameClientSource::Remote,
            local_executable_path: legacy.game_executable_path.clone(),
            installation_id: record.id.clone(),
            executable_relative_path: "Windows/Game.exe".into(),
            storage_root: root.0.to_string_lossy().into_owned(),
        },
    };
    service.resolve_settings(&mut input).unwrap();
    assert!(!input.game_executable_path.starts_with(r"\\?\"));
    assert!(!record.directory.starts_with(r"\\?\"));
    assert_eq!(record.storage_root, root.0.to_string_lossy());
    assert!(
        input.game_executable_path.ends_with("Windows\\Game.exe")
            || input.game_executable_path.ends_with("Windows/Game.exe")
    );
    service.settings.update_public(input).unwrap();
    let restarted = GameClientService::new(
        service.database.clone(),
        service.settings.clone(),
        root.0.clone(),
    );
    assert_eq!(restarted.state().unwrap().installations.len(), 1);
    assert!(restarted.state().unwrap().account.is_none());
    assert_eq!(
        restarted
            .settings
            .get()
            .unwrap()
            .game_client
            .installation_id,
        record.id
    );
    assert!(record.digest_verified);
    assert!(restarted.check_update().unwrap().is_none());
}

#[test]
fn package_install_preserves_versions_and_requires_explicit_executable_selection() {
    let root = Temp::new();
    let service = service(&root);
    let one = installed(&service, &root, package(&[]));
    let two = installed(
        &service,
        &root,
        package(&[
            ("Windows/Other.exe", b"MZother"),
            ("Windows/Other_Data/data", b"data"),
        ]),
    );
    assert_ne!(one.directory, two.directory);
    assert!(PathBuf::from(&one.directory).is_dir());
    assert_eq!(two.executable_candidates.len(), 2);
    let mut selection = GameClientSelection::default();
    assert_eq!(
        install::resolve(&two, &selection).unwrap_err().code,
        "clientChooseExecutable"
    );
    selection.executable_relative_path = "Windows/Other.exe".into();
    assert!(install::resolve(&two, &selection).is_ok());
    fs::remove_file(PathBuf::from(&two.directory).join("Windows/Other.exe")).unwrap();
    assert_eq!(
        install::resolve(&two, &selection).unwrap_err().code,
        "clientInstallationMissing"
    );
    assert!(
        service
            .settings
            .get()
            .unwrap()
            .game_executable_path
            .is_empty()
    );
}

#[test]
fn rejects_windows_paths_traversal_devices_ads_and_case_collisions() {
    for name in [
        "../evil.exe",
        "C:/evil",
        "/absolute",
        "\\\\server\\file",
        "safe/../../bad",
        "safe\\..\\bad",
        "safe/x:stream",
        "CON",
        "aux.txt",
        "LPT9.exe",
        "COM¹",
        "trailing.",
        "trailing ",
        "a//b",
    ] {
        assert!(install::member_path(name).is_err(), "{name}");
    }
    assert_eq!(
        install::member_path("包/游戏.exe").unwrap(),
        PathBuf::from("包/游戏.exe")
    );
    let root = Temp::new();
    let bytes = package(&[("windows/game.EXE", b"evil")]);
    let mut release = release(1, "2026-10-01", false);
    release.assets[0].size = bytes.len() as u64;
    let result = install::install(
        Cursor::new(bytes),
        &release,
        &release.assets[0],
        root.0.to_str().unwrap(),
        &Progress::default(),
        |_| panic!("Must not persist"),
    );
    assert_eq!(result.unwrap_err().code, "clientUnsafeArchive");
    assert_eq!(
        fs::read_dir(root.0.join("PlatverseOutput"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn size_hash_cancellation_and_persistence_failure_leave_no_partial_install() {
    let root = Temp::new();
    let bytes = package(&[]);
    for failure in ["length", "hash", "cancel", "persistence"] {
        let mut release = release(1, "2026-10-01", false);
        release.assets[0].size = bytes.len() as u64;
        if failure == "length" {
            release.assets[0].size += 1;
        }
        if failure == "hash" {
            release.assets[0].digest = Some("sha256:bad".into());
        }
        let progress = Progress::default();
        if failure == "cancel" {
            progress.cancelled.store(true, Ordering::Relaxed);
        }
        let result = install::install(
            Cursor::new(bytes.clone()),
            &release,
            &release.assets[0],
            root.0.to_str().unwrap(),
            &progress,
            |_| Err(error("testPersistence")),
        );
        assert!(result.is_err(), "{failure}");
        assert_eq!(
            fs::read_dir(root.0.join("PlatverseOutput"))
                .unwrap()
                .count(),
            0,
            "{failure}"
        );
    }
}

#[test]
fn zip_symlinks_and_packages_without_a_unity_player_are_rejected() {
    let root = Temp::new();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.add_symlink(
        "escape",
        "../../outside",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    let symlink = zip.finish().unwrap().into_inner();
    let empty = zip::ZipWriter::new(Cursor::new(Vec::new()))
        .finish()
        .unwrap()
        .into_inner();
    for (bytes, code) in [
        (symlink, "clientUnsafeArchive"),
        (empty, "clientNoExecutable"),
    ] {
        let mut release = release(1, "2026-10-01", false);
        release.assets[0].size = bytes.len() as u64;
        let result = install::install(
            Cursor::new(bytes),
            &release,
            &release.assets[0],
            root.0.to_str().unwrap(),
            &Progress::default(),
            |_| panic!("Must not persist"),
        );
        assert_eq!(result.unwrap_err().code, code);
    }
}

#[test]
fn updates_are_advisory_respect_stable_channel_and_detect_replaced_assets() {
    let root = Temp::new();
    let service = service(&root);
    let current = installed(&service, &root, package(&[]));
    let old = release(2, "2026-09-01T00:00:00Z", false);
    let mut draft = release(3, "2026-12-01T00:00:00Z", false);
    draft.draft = true;
    let preview = release(4, "2026-11-01T00:00:00Z", true);
    assert!(update_candidate(&current, &[old.clone(), draft.clone(), preview.clone()]).is_none());
    let next = release(5, "2026-10-14T00:00:00Z", false);
    assert_eq!(
        update_candidate(&current, &[old, draft, preview, next])
            .unwrap()
            .id,
        5
    );
    let mut replaced = release(1, "2026-10-01T00:00:00Z", false);
    replaced.assets[0].id += 1;
    assert!(update_candidate(&current, &[replaced]).is_some());
    assert!(
        service
            .settings
            .get()
            .unwrap()
            .game_executable_path
            .is_empty()
    );
}

pub(super) fn mock(
    responses: Vec<(u16, String, String)>,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        responses.into_iter().map(|(status, headers, body)| {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut bytes = Vec::new(); let mut buffer = [0; 1];
        while !bytes.ends_with(b"\r\n\r\n") { stream.read_exact(&mut buffer).unwrap(); bytes.extend_from_slice(&buffer); }
        let header = String::from_utf8(bytes.clone()).unwrap();
        let length = header.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|value| value.trim().parse::<usize>().unwrap())).unwrap_or(0);
        let mut body_bytes = vec![0; length]; stream.read_exact(&mut body_bytes).unwrap(); bytes.extend_from_slice(&body_bytes);
        let content_type = if headers.lines().any(|line| line.to_ascii_lowercase().starts_with("content-type:")) {
            ""
        } else {
            "Content-Type: application/json\r\n"
        };
        write!(stream, "HTTP/1.1 {status} Test\r\n{content_type}Content-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}", body.len()).unwrap();
        String::from_utf8(bytes).unwrap()
    }).collect()
    });
    (address, handle)
}

#[test]
fn github_paginates_and_errors_do_not_include_secrets_or_response_bodies() {
    let first = serde_json::to_string(&vec![release(1, "2026-10-01", false); 100]).unwrap();
    let second = serde_json::to_string(&vec![release(2, "2026-10-02", false)]).unwrap();
    let (api, handle) = mock(vec![
        (200, String::new(), first),
        (200, String::new(), second),
    ]);
    let github = GitHub::with_test_api(Some("test-only-credential".into()), api).unwrap();
    assert_eq!(github.releases().unwrap().len(), 101);
    let requests = handle.join().unwrap();
    assert!(requests[0].contains("page=1"));
    assert!(requests[1].contains("page=2"));
    let (api, handle) = mock(vec![(
        401,
        String::new(),
        "secret-response-test-only-credential".into(),
    )]);
    let err = GitHub::with_test_api(Some("test-only-credential".into()), api)
        .unwrap()
        .releases()
        .unwrap_err();
    assert_eq!(err.code, "clientUnauthorized");
    assert!(!format!("{err:?}").contains("test-only-credential"));
    handle.join().unwrap();
}

#[test]
fn github_checks_asset_membership_and_rejects_untrusted_download_redirects() {
    let release_json = serde_json::to_string(&release(1, "2026-10-01", false)).unwrap();
    let (api, handle) = mock(vec![
        (200, String::new(), release_json),
        (200, String::new(), "[]".into()),
    ]);
    let err = GitHub::with_test_api(Some("test".into()), api)
        .unwrap()
        .delete_asset(1, 99)
        .unwrap_err();
    assert_eq!(err.code, "clientAssetMissing");
    assert_eq!(handle.join().unwrap().len(), 2);
    let (api, handle) = mock(vec![(
        302,
        "Location: https://example.com/steal\r\n".into(),
        String::new(),
    )]);
    assert_eq!(
        GitHub::with_test_api(Some("test".into()), api)
            .unwrap()
            .download(1)
            .unwrap_err()
            .code,
        "clientDownloadHost"
    );
    handle.join().unwrap();
    for url in [
        "http://release-assets.githubusercontent.com/x",
        "https://release-assets.githubusercontent.com.evil/x",
        "https://user@objects.githubusercontent.com/x",
        "https://objects.githubusercontent.com:444/x",
    ] {
        assert!(!github::safe_download_url(
            &reqwest::Url::parse(url).unwrap()
        ));
    }
    assert!(github::safe_download_url(
        &reqwest::Url::parse("https://release-assets.githubusercontent.com/a?sig=hidden").unwrap()
    ));
}

#[test]
fn github_release_crud_uses_expected_methods_and_asset_membership() {
    let wire = serde_json::json!({"id":1,"tag_name":"v1","name":"Game","body":"notes","draft":false,"prerelease":false,"published_at":"2026-10-14T00:00:00Z","assets":[]}).to_string();
    let asset = serde_json::json!({"id":10,"name":"Windows.zip","size":3,"updated_at":"2026-10-14","state":"uploaded"});
    let assets = serde_json::json!([asset]).to_string();
    let (api, handle) = mock(vec![
        (201, String::new(), wire.clone()),
        (200, String::new(), wire.clone()),
        (200, String::new(), assets.clone()),
        (200, String::new(), wire.clone()),
        (200, String::new(), wire.clone()),
        (200, String::new(), assets),
        (204, String::new(), String::new()),
        (200, String::new(), wire),
        (200, String::new(), "[]".into()),
        (204, String::new(), String::new()),
    ]);
    let github = GitHub::with_test_api(Some("test-token".into()), api).unwrap();
    let input = |id| ReleaseInput {
        id,
        tag_name: "v1".into(),
        name: "Game".into(),
        body: "notes".into(),
        draft: false,
        prerelease: false,
    };
    assert_eq!(github.save_release(input(None)).unwrap().tag_name, "v1");
    github.save_release(input(Some(1))).unwrap();
    github.delete_asset(1, 10).unwrap();
    github.delete_release(1).unwrap();
    let requests = handle.join().unwrap();
    assert!(requests[0].starts_with("POST /repos/sxg15/PlatverseOutput/releases "));
    assert!(requests[0].contains("\"tag_name\":\"v1\""));
    assert!(requests[3].starts_with("PATCH /repos/sxg15/PlatverseOutput/releases/1 "));
    assert!(requests[6].starts_with("DELETE /repos/sxg15/PlatverseOutput/releases/assets/10 "));
    assert!(requests[9].starts_with("DELETE /repos/sxg15/PlatverseOutput/releases/1 "));
}

#[test]
fn github_upload_stream_and_binary_download_preserve_bytes() {
    let wire = serde_json::to_string(&release(1, "2026-10-14", false)).unwrap();
    let uploaded = serde_json::to_string(&release(1, "2026-10-14", false).assets[0]).unwrap();
    let (api, handle) = mock(vec![
        (200, String::new(), wire),
        (200, String::new(), "[]".into()),
        (201, String::new(), uploaded),
        (
            200,
            "Content-Type: application/octet-stream\r\n".into(),
            "download-fixture".into(),
        ),
    ]);
    let github = GitHub::with_test_api(Some("test-token".into()), api).unwrap();
    let payload = b"zip-fixture";
    github
        .upload(
            1,
            "Windows.zip",
            reqwest::blocking::Body::sized(Cursor::new(payload.to_vec()), payload.len() as u64),
        )
        .unwrap();
    let mut downloaded = Vec::new();
    github
        .download(10)
        .unwrap()
        .read_to_end(&mut downloaded)
        .unwrap();
    assert_eq!(downloaded, b"download-fixture");
    let requests = handle.join().unwrap();
    assert!(
        requests[2]
            .starts_with("POST /repos/sxg15/PlatverseOutput/releases/1/assets?name=Windows.zip ")
    );
    assert!(
        requests[2]
            .to_lowercase()
            .contains("content-type: application/zip")
    );
    assert!(requests[2].ends_with("zip-fixture"));
    for (index, request) in requests.iter().enumerate() {
        let accept: Vec<_> = request
            .lines()
            .filter_map(|line| line.split_once(':'))
            .filter(|(name, _)| name.eq_ignore_ascii_case("accept"))
            .map(|(_, value)| value.trim())
            .collect();
        assert_eq!(
            accept,
            [if index == 3 {
                "application/octet-stream"
            } else {
                "application/vnd.github+json"
            }],
            "Each request must negotiate exactly one response type"
        );
    }
}

#[test]
fn github_rejects_metadata_and_web_pages_before_installing() {
    for content_type in [
        "application/json; charset=utf-8",
        "Application/Problem+JSON",
        "text/html; charset=utf-8",
        "text/plain",
        "application/xml",
        "application/problem+xml",
    ] {
        let (api, handle) = mock(vec![(
            200,
            format!("Content-Type: {content_type}\r\n"),
            "private-response-body".into(),
        )]);
        let err = GitHub::with_test_api(Some("test-only-credential".into()), api)
            .unwrap()
            .download(10)
            .unwrap_err();
        assert_eq!(err.code, "clientDownloadResponse");
        let displayed = format!("{err:?}");
        assert!(!displayed.contains("private-response-body"));
        assert!(!displayed.contains("test-only-credential"));
        handle.join().unwrap();
    }
}

#[test]
fn remembered_account_is_encrypted_and_logout_preserves_installations() {
    let root = Temp::new();
    let service = service(&root);
    installed(&service, &root, package(&[]));
    let token = "test-only-not-a-real-github-token";
    service
        .write(
            AUTH_KEY,
            &StoredAccount {
                account: GitHubAccount {
                    login: "fixture".into(),
                    can_manage: true,
                    remembered: true,
                },
                encrypted_token: Some(secrets::protect(token, SECRET_PURPOSE).unwrap()),
                encrypted_credentials: None,
            },
        )
        .unwrap();
    let stored: serde_json::Value = service.read(AUTH_KEY).unwrap().unwrap();
    assert!(!stored.to_string().contains(token));
    assert!(
        !serde_json::to_string(&service.state().unwrap())
            .unwrap()
            .contains(token)
    );
    assert_eq!(service.token().unwrap().unwrap(), token);
    service.logout().unwrap();
    assert!(service.token().unwrap().is_none());
    assert_eq!(service.state().unwrap().installations.len(), 1);
}

#[test]
#[ignore = "Read-only live GitHub smoke using the developer's existing gh credential; opt-in only"]
fn live_github_account_and_release_reads() {
    assert_eq!(std::env::var("ABYA_TEST_GITHUB_READS").as_deref(), Ok("1"));
    let root = Temp::new();
    let service = service(&root);
    let output = std::process::Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .output()
        .expect("gh is required");
    assert!(
        output.status.success(),
        "GitHub CLI must already be authenticated"
    );
    let token = String::from_utf8(output.stdout).expect("Token encoding");
    let state = service
        .login_for_read_test(token.trim().into())
        .expect("Read-only login validation");
    assert!(state.account.is_some());
    assert!(service.read::<StoredAccount>(AUTH_KEY).unwrap().is_none());
    let releases = service.releases().expect("Live release list");
    for item in releases.iter().take(1) {
        service.release(item.id).expect("Live asset list");
    }
    service.logout().unwrap();
    println!(
        "Live account and release reads passed; {} releases; no remote writes or saved credentials.",
        releases.len()
    );
}

#[test]
#[ignore = "Downloads and installs a selected real GitHub ZIP in an isolated temporary fixture; opt-in only"]
fn live_github_download_and_install() {
    assert_eq!(
        std::env::var("ABYA_TEST_GITHUB_INSTALL").as_deref(),
        Ok("1")
    );
    let release_id = std::env::var("ABYA_TEST_GITHUB_RELEASE_ID")
        .expect("Select the release explicitly")
        .parse::<u64>()
        .expect("Release ID must be numeric");
    let asset_id = std::env::var("ABYA_TEST_GITHUB_ASSET_ID")
        .expect("Select the asset explicitly")
        .parse::<u64>()
        .expect("Asset ID must be numeric");
    let root = Temp::new();
    let service = service(&root);
    let output = std::process::Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .output()
        .expect("gh is required");
    assert!(output.status.success(), "GitHub CLI must be authenticated");
    let token = String::from_utf8(output.stdout).expect("Token encoding");
    service
        .login_for_read_test(token.trim().into())
        .expect("Read-only login validation");
    let original_settings = serde_json::to_value(service.settings.get().unwrap()).unwrap();
    let release = service.release(release_id).expect("Read selected release");
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .expect("Asset must belong to the release");
    assert!(asset.installable());
    let expected_digest = asset.digest.as_ref().expect("Live smoke requires a digest");
    let storage = root.0.join("中文 安装目录");
    let installer = service.clone();
    let worker = thread::spawn(move || {
        installer.install(InstallInput {
            release_id,
            asset_id,
            storage_root: storage.to_string_lossy().into_owned(),
        })
    });
    while !worker.is_finished() {
        let progress = service.operation();
        println!(
            "Live install: {} {}/{} bytes",
            progress.stage, progress.completed_bytes, progress.total_bytes
        );
        thread::sleep(std::time::Duration::from_secs(15));
    }
    let record = worker
        .join()
        .expect("Installer worker")
        .expect("Live download, integrity verification and extraction");
    assert!(record.digest_verified);
    assert_eq!(format!("sha256:{}", record.sha256), *expected_digest);
    assert!(!record.executable_candidates.is_empty());
    let selection = GameClientSelection {
        source: GameClientSource::Remote,
        installation_id: record.id.clone(),
        executable_relative_path: record.executable_candidates[0].clone(),
        ..Default::default()
    };
    let executable = install::resolve(&record, &selection).expect("Installed Unity executable");
    assert!(PathBuf::from(&executable).is_file());
    assert_eq!(
        serde_json::to_value(service.settings.get().unwrap()).unwrap(),
        original_settings
    );
    assert_eq!(service.state().unwrap().installations.len(), 1);
    service.logout().unwrap();
    assert!(service.read::<StoredAccount>(AUTH_KEY).unwrap().is_none());
    drop(service);
    let restored = self::service(&root);
    let state = restored.state().unwrap();
    assert!(state.account.is_none());
    assert_eq!(state.installations.len(), 1);
    assert_eq!(state.installations[0].sha256, record.sha256);
    println!(
        "Live install passed: {} bytes, SHA-256 {}, {} Unity EXE(s); persisted offline, unchanged settings, no remote writes or saved credentials.",
        asset.size,
        record.sha256,
        record.executable_candidates.len()
    );
}
