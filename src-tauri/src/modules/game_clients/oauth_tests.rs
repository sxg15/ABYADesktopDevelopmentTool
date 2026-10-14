use super::{
    auth::{AUTH_KEY, PendingLogin, SessionAccount, StoredAccount},
    oauth::Credentials,
    tests::{Temp, mock, service},
    *,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn json(value: serde_json::Value) -> (u16, String, String) {
    (200, String::new(), value.to_string())
}
fn device() -> serde_json::Value {
    serde_json::json!({ "device_code":"secret-device", "user_code":"ABCD-EFGH", "verification_uri":oauth::VERIFICATION_URI, "expires_in":900, "interval":5 })
}
fn token(access: &str, refresh: &str) -> serde_json::Value {
    serde_json::json!({ "access_token":access, "refresh_token":refresh, "token_type":"bearer", "scope":"repo", "expires_in":28800, "refresh_token_expires_in":15897600 })
}
fn tick(service: &GameClientService) {
    service.auth.lock().pending.as_mut().unwrap().next_poll = Instant::now();
}
fn fixture_account(remembered: bool) -> GitHubAccount {
    GitHubAccount {
        login: "fixture".into(),
        can_manage: true,
        remembered,
    }
}
fn expired_credentials() -> Credentials {
    Credentials {
        access_token: "old-access".into(),
        refresh_token: Some("old-refresh".into()),
        expires_at: Some(chrono::Utc::now().timestamp() - 1),
        refresh_expires_at: Some(chrono::Utc::now().timestamp() + 3600),
    }
}
fn set_session(service: &GameClientService, remember: bool) {
    let mut auth = service.auth.lock();
    auth.loaded = true;
    auth.session = Some(SessionAccount {
        account: fixture_account(remember),
        credentials: expired_credentials(),
    });
}
fn pending(service: &GameClientService) {
    service.auth.lock().pending = Some(PendingLogin {
        id: "attempt".into(),
        device_code: "secret-device".into(),
        user_code: "ABCD-EFGH".into(),
        expires_at: chrono::Utc::now().timestamp() + 900,
        interval: 5,
        next_poll: Instant::now(),
        in_flight: false,
        remember: true,
    });
}

#[test]
fn device_flow_throttles_polls_accepts_identity_and_persists_encrypted_rotating_credentials() {
    let root = Temp::new();
    let mut service = service(&root);
    let (base, handle) = mock(vec![
        json(device()),
        json(serde_json::json!({"error":"authorization_pending"})),
        json(serde_json::json!({"error":"slow_down","interval":7})),
        json(token("new-access", "new-refresh")),
        json(serde_json::json!({"login":"fixture"})),
        json(serde_json::json!({"permissions":{"push":true}})),
    ]);
    service.oauth_base = Some(base.clone());
    service.github_base = Some(base);
    let start = service.start_login(true).unwrap();
    assert_eq!(start.user_code, "ABCD-EFGH");
    assert!(
        !serde_json::to_string(&service.state().unwrap())
            .unwrap()
            .contains("secret-device")
    );
    assert!(service.read::<StoredAccount>(AUTH_KEY).unwrap().is_none());
    // No network request is allowed before GitHub's minimum interval, even for multiple callers.
    assert_eq!(service.poll_login(&start.id).unwrap().status, "pending");
    tick(&service);
    assert_eq!(service.poll_login(&start.id).unwrap().status, "pending");
    tick(&service);
    let slowed = service.poll_login(&start.id).unwrap();
    assert_eq!(slowed.authorization.unwrap().poll_after_seconds, 10);
    tick(&service);
    assert_eq!(
        service.poll_login(&start.id).unwrap().status,
        "authenticated"
    );
    assert_eq!(service.state().unwrap().account.unwrap().login, "fixture");
    let stored: serde_json::Value = service.read(AUTH_KEY).unwrap().unwrap();
    for secret in ["new-access", "new-refresh", "secret-device"] {
        assert!(!stored.to_string().contains(secret));
        assert!(
            !serde_json::to_string(&service.state().unwrap())
                .unwrap()
                .contains(secret)
        );
    }
    assert!(stored["encrypted_token"].is_null());
    let restarted = GameClientService::new(
        service.database.clone(),
        service.settings.clone(),
        root.0.clone(),
    );
    assert_eq!(restarted.token().unwrap().as_deref(), Some("new-access"));
    let requests = handle.join().unwrap();
    assert!(requests[0].contains("client_id=Ov23li0rDQSvLA8yT7sj&scope=repo"));
    assert!(
        !requests
            .iter()
            .any(|request| request.contains("client_secret"))
    );
    assert!(
        requests[1].contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code")
    );
}

#[test]
fn denied_expired_or_wrong_repository_login_never_saves_credentials() {
    for (reply, expected) in [
        (
            serde_json::json!({"error":"access_denied"}),
            "clientOAuthDenied",
        ),
        (
            serde_json::json!({"error":"expired_token"}),
            "clientOAuthExpired",
        ),
        (
            token("not-allowed", "not-allowed-refresh"),
            "clientNotFound",
        ),
    ] {
        let root = Temp::new();
        let mut service = service(&root);
        pending(&service);
        let mut replies = vec![json(reply)];
        if expected == "clientNotFound" {
            replies.extend([
                json(serde_json::json!({"login":"stranger"})),
                (404, String::new(), "{}".into()),
            ]);
        }
        let (base, handle) = mock(replies);
        service.oauth_base = Some(base.clone());
        service.github_base = Some(base);
        assert_eq!(service.poll_login("attempt").unwrap_err().code, expected);
        assert!(service.state().unwrap().account.is_none());
        assert!(service.authorization().is_none());
        assert!(service.read::<StoredAccount>(AUTH_KEY).unwrap().is_none());
        handle.join().unwrap();
    }
}

#[test]
fn cancellation_expiry_and_a_replaced_attempt_cannot_be_polled() {
    let root = Temp::new();
    let mut service = service(&root);
    pending(&service);
    service.cancel_login("another-attempt");
    assert!(service.authorization().is_some());
    service.cancel_login("attempt");
    assert_eq!(
        service.poll_login("attempt").unwrap_err().code,
        "clientOAuthCancelled"
    );
    pending(&service);
    service.auth.lock().pending.as_mut().unwrap().expires_at = 1;
    assert_eq!(
        service.poll_login("attempt").unwrap_err().code,
        "clientOAuthExpired"
    );
    let (base, handle) = mock(vec![json(device()), json(device())]);
    service.oauth_base = Some(base);
    let first = service.start_login(true).unwrap();
    let second = service.start_login(false).unwrap();
    assert_ne!(first.id, second.id);
    service.cancel_login(&first.id);
    assert_eq!(service.authorization().unwrap().id, second.id);
    handle.join().unwrap();
}

#[test]
fn refresh_is_serialized_rotates_both_credentials_and_survives_restart() {
    let root = Temp::new();
    let mut service = service(&root);
    set_session(&service, true);
    let (base, handle) = mock(vec![json(token("rotated-access", "rotated-refresh"))]);
    service.oauth_base = Some(base);
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let service = service.clone();
            thread::spawn(move || service.token().unwrap().unwrap())
        })
        .collect();
    for reader in readers {
        assert_eq!(reader.join().unwrap(), "rotated-access");
    }
    let requests = handle.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].contains("grant_type=refresh_token&refresh_token=old-refresh"));
    assert!(!requests[0].contains("client_secret"));
    let restarted = GameClientService::new(
        service.database.clone(),
        service.settings.clone(),
        root.0.clone(),
    );
    assert_eq!(
        restarted.token().unwrap().as_deref(),
        Some("rotated-access")
    );
    assert_eq!(
        restarted
            .auth
            .lock()
            .session
            .as_ref()
            .unwrap()
            .credentials
            .refresh_token
            .as_deref(),
        Some("rotated-refresh")
    );
    service.logout().unwrap();
    assert!(service.read::<StoredAccount>(AUTH_KEY).unwrap().is_none());
}

#[test]
fn failed_refresh_requires_reauthorization_but_network_failures_preserve_the_session() {
    for (response, expected, preserves) in [
        (
            json(serde_json::json!({"error":"bad_refresh_token"})),
            "clientReauthRequired",
            false,
        ),
        (
            (503, String::new(), "private diagnostic body".into()),
            "clientOAuthUnavailable",
            true,
        ),
    ] {
        let root = Temp::new();
        let mut service = service(&root);
        set_session(&service, false);
        let (base, handle) = mock(vec![response]);
        service.oauth_base = Some(base);
        assert_eq!(service.token().unwrap_err().code, expected);
        assert_eq!(service.state().unwrap().account.is_some(), preserves);
        assert!(service.read::<StoredAccount>(AUTH_KEY).unwrap().is_none());
        handle.join().unwrap();
    }
}

fn blocked_response(
    body: String,
) -> (
    String,
    mpsc::Receiver<()>,
    mpsc::Sender<()>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (arrived, received) = mpsc::channel();
    let (resume, wait) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut one = [0];
        while !bytes.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut one).unwrap();
            bytes.push(one[0]);
        }
        let header = String::from_utf8(bytes).unwrap();
        let length = header
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|value| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        stream.read_exact(&mut vec![0; length]).unwrap();
        arrived.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    (base, received, resume, handle)
}

#[test]
fn logout_and_cancel_win_over_in_flight_authorization_and_refresh() {
    for refreshing in [false, true] {
        let root = Temp::new();
        let mut service = service(&root);
        if refreshing {
            set_session(&service, true);
        } else {
            pending(&service);
        }
        let (base, arrived, resume, handle) =
            blocked_response(token("late-access", "late-refresh").to_string());
        service.oauth_base = Some(base);
        let (api, identity) = mock(if refreshing {
            vec![]
        } else {
            vec![
                json(serde_json::json!({"login":"fixture"})),
                json(serde_json::json!({"permissions":{"push":true}})),
            ]
        });
        service.github_base = Some(api);
        let worker = {
            let service = service.clone();
            thread::spawn(move || {
                if refreshing {
                    service.token().map(|_| ())
                } else {
                    service.poll_login("attempt").map(|_| ())
                }
            })
        };
        arrived.recv_timeout(Duration::from_secs(5)).unwrap();
        if refreshing {
            service.logout().unwrap();
        } else {
            service.cancel_login("attempt");
        }
        resume.send(()).unwrap();
        assert!(worker.join().unwrap().is_err());
        handle.join().unwrap();
        identity.join().unwrap();
        assert!(service.state().unwrap().account.is_none());
        assert!(service.read::<StoredAccount>(AUTH_KEY).unwrap().is_none());
    }
}

#[test]
fn rejects_untrusted_verification_urls_and_never_exposes_oauth_error_details() {
    let root = Temp::new();
    let mut service = service(&root);
    let mut value = device();
    value["verification_uri"] = "https://example.org/login".into();
    let (base, handle) = mock(vec![
        json(value),
        json(
            serde_json::json!({"error":"device_flow_disabled","error_description":"sensitive-server-content"}),
        ),
    ]);
    service.oauth_base = Some(base);
    assert_eq!(
        service.start_login(true).unwrap_err().code,
        "clientResponse"
    );
    let error = service.start_login(true).unwrap_err();
    assert_eq!(error.code, "clientOAuthUnavailable");
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("sensitive-server-content")
    );
    handle.join().unwrap();
}

#[test]
#[ignore = "Public OAuth app/device-flow configuration smoke; opt-in, no user authorization"]
fn live_oauth_device_flow_configuration() {
    assert_eq!(std::env::var("ABYA_TEST_GITHUB_OAUTH").as_deref(), Ok("1"));
    let root = Temp::new();
    let service = service(&root);
    let authorization = service
        .start_login(false)
        .expect("OAuth app must have Device Flow enabled");
    assert_eq!(authorization.verification_uri, oauth::VERIFICATION_URI);
    assert!(authorization.poll_after_seconds >= 1);
    service.cancel_login(&authorization.id);
    assert!(service.state().unwrap().account.is_none());
    assert!(service.read::<StoredAccount>(AUTH_KEY).unwrap().is_none());
    println!(
        "Public OAuth application accepts device-flow requests; cancelled without user authorization or stored credentials."
    );
}
