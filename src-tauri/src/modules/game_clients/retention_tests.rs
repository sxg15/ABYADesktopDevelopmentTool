use super::{
    tests::{Temp, installed, package, service},
    *,
};
use std::fs;

#[test]
fn activation_and_token_rotation_cannot_overwrite_each_others_settings() {
    let root = Temp::new();
    let service = service(&root);
    let selected = installed(&service, &root, package(&[]));
    for _ in 0..8 {
        let gate = std::sync::Arc::new(std::sync::Barrier::new(3));
        let selecting = service.clone();
        let id = selected.id.clone();
        let ready = gate.clone();
        let selection = std::thread::spawn(move || {
            ready.wait();
            selecting.activate(&id, "Windows/Game.exe").unwrap()
        });
        let rotating = service.settings.clone();
        let ready = gate.clone();
        let rotation = std::thread::spawn(move || {
            ready.wait();
            rotating.regenerate_token().unwrap()
        });
        gate.wait();
        let chosen = selection.join().unwrap();
        let rotated = rotation.join().unwrap();
        let saved = service.settings.get().unwrap();
        assert_eq!(saved.game_client.installation_id, selected.id);
        assert_eq!(saved.game_executable_path, chosen.game_executable_path);
        assert_eq!(saved.desktop_cli_token, rotated.desktop_cli_token);
    }
}

#[test]
fn activation_preserves_unrelated_settings_and_removes_only_unselected_owned_packages() {
    let root = Temp::new();
    let other_root = Temp::new();
    let service = service(&root);
    let mut before = service.settings.get().unwrap();
    before.game_client.local_executable_path = "C:\\Local\\Game.exe".into();
    before.locale = "en-US".into();
    before.lan_broadcast_enabled = false;
    service.settings.save(&before).unwrap();
    let old = installed(&service, &root, package(&[]));
    service.activate(&old.id, "Windows/Game.exe").unwrap();
    let new = installed(&service, &other_root, package(&[]));
    let unrelated = root.0.join("PlatverseOutput/user-content");
    fs::create_dir(&unrelated).unwrap();
    fs::write(unrelated.join("keep.txt"), "keep").unwrap();
    let after = service.activate(&new.id, "Windows/Game.exe").unwrap();
    assert_eq!(after.game_client.installation_id, new.id);
    assert_eq!(
        after.game_client.local_executable_path,
        before.game_client.local_executable_path
    );
    assert_eq!(after.locale, before.locale);
    assert_eq!(after.desktop_cli_token, before.desktop_cli_token);
    assert!(!after.lan_broadcast_enabled);
    let report = service.maintain(&[], &[]).unwrap();
    assert_eq!(report.removed, 1);
    assert!(report.deferred.is_empty());
    assert!(!PathBuf::from(old.directory).exists());
    assert!(PathBuf::from(&new.directory).is_dir());
    assert_eq!(
        fs::read_to_string(unrelated.join("keep.txt")).unwrap(),
        "keep"
    );
    assert_eq!(service.state().unwrap().installations.len(), 1);
}

#[test]
fn pinned_running_local_and_pending_packages_are_deferred_until_released() {
    let root = Temp::new();
    let service = service(&root);
    let pinned = installed(&service, &root, package(&[]));
    let running = installed(&service, &root, package(&[]));
    let local = installed(&service, &root, package(&[]));
    let pending = installed(&service, &root, package(&[]));
    let selected = installed(&service, &root, package(&[]));
    service.activate(&selected.id, "Windows/Game.exe").unwrap();
    let mut settings = service.settings.get().unwrap();
    settings.game_client.local_executable_path = format!("{}/Windows/Game.exe", local.directory);
    service.settings.save(&settings).unwrap();
    *service.pending_installation.lock() = Some(pending.id.clone());
    let pins = [format!("{}\\Windows\\Game.exe", pinned.directory).to_uppercase()];
    let active = [format!("{}/Windows/Game.exe", running.directory)];
    let report = service.maintain(&pins, &active).unwrap();
    assert_eq!(report.removed, 0);
    assert_eq!(report.deferred.len(), 3);
    assert!(report.deferred.iter().any(|r| r.reason == "task"));
    assert!(report.deferred.iter().any(|r| r.reason == "running"));
    assert!(report.deferred.iter().any(|r| r.reason == "local"));
    service.discard_pending(&pending.id).unwrap();
    settings.game_client.local_executable_path.clear();
    service.settings.save(&settings).unwrap();
    assert_eq!(service.maintain(&[], &[]).unwrap().removed, 4);
    assert_eq!(service.state().unwrap().installations[0].id, selected.id);
}

#[test]
fn failed_executable_selection_keeps_current_client_and_pending_download() {
    let root = Temp::new();
    let service = service(&root);
    let old = installed(&service, &root, package(&[]));
    service.activate(&old.id, "Windows/Game.exe").unwrap();
    let new = installed(
        &service,
        &root,
        package(&[
            ("Windows/Other.exe", b"MZother"),
            ("Windows/Other_Data/test", b"data"),
        ]),
    );
    *service.pending_installation.lock() = Some(new.id.clone());
    assert_eq!(
        service.activate(&new.id, "").unwrap_err().code,
        "clientChooseExecutable"
    );
    assert_eq!(
        service.settings.get().unwrap().game_client.installation_id,
        old.id
    );
    assert_eq!(service.maintain(&[], &[]).unwrap().removed, 0);
    service.discard_pending(&new.id).unwrap();
    assert_eq!(service.maintain(&[], &[]).unwrap().removed, 1);
    assert!(PathBuf::from(old.directory).is_dir());
    assert!(!PathBuf::from(new.directory).exists());
}

#[test]
fn cleanup_refuses_tampered_manifests_and_paths_outside_the_recorded_root() {
    let root = Temp::new();
    let service = service(&root);
    let one = installed(&service, &root, package(&[]));
    let mut two = installed(&service, &root, package(&[]));
    fs::write(
        PathBuf::from(&one.directory).join(".abya-client.json"),
        b"{}",
    )
    .unwrap();
    two.storage_root = root.0.join("elsewhere").to_string_lossy().into_owned();
    service
        .write(&format!("{INSTALL_PREFIX}{}", two.id), &two)
        .unwrap();
    let report = service.maintain(&[], &[]).unwrap();
    assert_eq!(report.removed, 0);
    assert_eq!(report.deferred.len(), 2);
    assert!(
        PathBuf::from(one.directory)
            .join("Windows/Game.exe")
            .is_file()
    );
    assert!(
        PathBuf::from(two.directory)
            .join("Windows/Game.exe")
            .is_file()
    );
}

#[cfg(windows)]
#[test]
fn cleanup_refuses_nested_junctions_before_deleting_any_files() {
    let root = Temp::new();
    let outside = Temp::new();
    let service = service(&root);
    let record = installed(&service, &root, package(&[]));
    fs::write(outside.0.join("keep.txt"), "outside").unwrap();
    let link = PathBuf::from(&record.directory).join("linked");
    // Creating a junction needs no elevation and never executes a downloaded program.
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&outside.0)
        .output()
        .unwrap();
    assert!(status.status.success());
    let report = service.maintain(&[], &[]).unwrap();
    assert_eq!(report.removed, 0);
    assert_eq!(report.deferred[0].reason, "unavailable");
    assert!(
        PathBuf::from(record.directory)
            .join("Windows/Game.exe")
            .is_file()
    );
    assert_eq!(
        fs::read_to_string(outside.0.join("keep.txt")).unwrap(),
        "outside"
    );
    fs::remove_dir(link).unwrap();
}

#[test]
fn restart_reclaims_unselected_abandoned_download_but_keeps_saved_selection() {
    let root = Temp::new();
    let service = service(&root);
    let selected = installed(&service, &root, package(&[]));
    service.activate(&selected.id, "Windows/Game.exe").unwrap();
    let abandoned = installed(&service, &root, package(&[]));
    *service.pending_installation.lock() = Some(abandoned.id.clone());
    let restarted = GameClientService::new(
        service.database.clone(),
        service.settings.clone(),
        root.0.clone(),
    );
    assert_eq!(restarted.maintain(&[], &[]).unwrap().removed, 1);
    assert_eq!(restarted.state().unwrap().installations[0].id, selected.id);
    assert!(!PathBuf::from(abandoned.directory).exists());
}

#[test]
fn service_download_activate_cleanup_and_exact_asset_reuse_share_one_persisted_selection() {
    use sha2::{Digest, Sha256};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    let root = Temp::new();
    let mut service = service(&root);
    let old = installed(&service, &root, package(&[]));
    service.activate(&old.id, "Windows/Game.exe").unwrap();
    let bytes = package(&[("Windows/Game_Data/version", b"next")]);
    let asset = ReleaseAsset {
        id: 20,
        name: "Windows.zip".into(),
        size: bytes.len() as u64,
        digest: Some(format!("sha256:{:x}", Sha256::digest(&bytes))),
        updated_at: "2026-10-16".into(),
        state: "uploaded".into(),
    };
    let release = ClientRelease {
        id: 2,
        tag_name: "v2".into(),
        name: None,
        body: None,
        draft: false,
        prerelease: false,
        immutable: false,
        published_at: Some("2026-10-16".into()),
        assets: vec![asset.clone()],
    };
    let metadata = serde_json::to_vec(&release).unwrap();
    let assets = serde_json::to_vec(&vec![asset]).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    service.github_base = Some(format!("http://{}", listener.local_addr().unwrap()));
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for (content_type, body) in [
            ("application/json", metadata.clone()),
            ("application/json", assets.clone()),
            ("application/octet-stream", bytes),
            ("application/json", metadata),
            ("application/json", assets),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            requests.push(String::from_utf8(request).unwrap());
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
        }
        requests
    });
    let download = || InstallInput {
        release_id: 2,
        asset_id: 20,
        storage_root: String::new(),
    };
    let next = service.install(download()).unwrap();
    assert!(next.digest_verified);
    assert_eq!(
        fs::canonicalize(&next.storage_root).unwrap(),
        fs::canonicalize(root.0.join("GameClients")).unwrap()
    );
    assert_eq!(
        service.settings.get().unwrap().game_client.installation_id,
        old.id
    );
    assert_eq!(service.maintain(&[], &[]).unwrap().removed, 0);
    service.activate(&next.id, "Windows/Game.exe").unwrap();
    assert_eq!(service.maintain(&[], &[]).unwrap().removed, 1);
    let again = service.install(download()).unwrap();
    assert_eq!(again.id, next.id);
    assert_eq!(service.state().unwrap().installations.len(), 1);
    service.discard_pending(&again.id).unwrap();
    assert_eq!(service.maintain(&[], &[]).unwrap().removed, 0);
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 5);
    assert!(requests[2].starts_with("GET /repos/sxg15/PlatverseOutput/releases/assets/20 "));
    let accepts: Vec<_> = requests[2]
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("accept:"))
        .collect();
    assert_eq!(accepts, vec!["accept: application/octet-stream"]);
}
