use super::{
    GameClientService, GitHubAccount, LoginAuthorization, LoginPoll,
    github::{GitHub, error},
    oauth::{Credentials, OAuth, TokenResult},
};
use crate::foundation::{AppResult, secrets};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub(super) const AUTH_KEY: &str = "game-clients/github-account";
pub(super) const SECRET_PURPOSE: &[u8] = b"ABYA GitHub Game Client Releases v1";

#[derive(Serialize, Deserialize)]
pub(super) struct StoredAccount {
    pub account: GitHubAccount,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_credentials: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_token: Option<String>, // Read compatibility with the previous release only.
}

#[derive(Clone)]
pub(super) struct SessionAccount {
    pub account: GitHubAccount,
    pub credentials: Credentials,
}

pub(super) struct PendingLogin {
    pub id: String,
    pub device_code: String,
    pub user_code: String,
    pub expires_at: i64,
    pub interval: u64,
    pub next_poll: Instant,
    pub in_flight: bool,
    pub remember: bool,
}

impl PendingLogin {
    fn dto(&self) -> LoginAuthorization {
        LoginAuthorization {
            id: self.id.clone(),
            user_code: self.user_code.clone(),
            verification_uri: super::oauth::VERIFICATION_URI.into(),
            expires_at: chrono::DateTime::from_timestamp(self.expires_at, 0)
                .unwrap()
                .to_rfc3339(),
            poll_after_seconds: self
                .next_poll
                .saturating_duration_since(Instant::now())
                .as_millis()
                .div_ceil(1000)
                .max(1) as u64,
        }
    }
}

#[derive(Default)]
pub(super) struct AuthState {
    pub loaded: bool,
    pub session: Option<SessionAccount>,
    pub pending: Option<PendingLogin>,
    pub generation: u64,
}

impl GameClientService {
    fn oauth(&self) -> AppResult<OAuth> {
        let oauth = OAuth::new()?;
        #[cfg(test)]
        let oauth = {
            let mut oauth = oauth;
            oauth.base = self.oauth_base.clone();
            oauth
        };
        Ok(oauth)
    }

    fn github_token(&self, token: String) -> AppResult<GitHub> {
        #[cfg(test)]
        if let Some(api) = &self.github_base {
            return GitHub::with_test_api(Some(token), api.clone());
        }
        GitHub::new(Some(token))
    }

    fn clear_auth(&self) -> AppResult<()> {
        self.database.with_connection(|db| {
            db.execute("DELETE FROM settings WHERE key=?1", [AUTH_KEY])?;
            Ok(())
        })
    }

    fn persist(&self, auth: &mut AuthState, session: SessionAccount) -> AppResult<()> {
        if session.account.remembered {
            let plaintext =
                serde_json::to_string(&session.credentials).map_err(|_| error("secretStorage"))?;
            self.write(
                AUTH_KEY,
                &StoredAccount {
                    account: session.account.clone(),
                    encrypted_credentials: Some(secrets::protect(&plaintext, SECRET_PURPOSE)?),
                    encrypted_token: None,
                },
            )?;
        } else {
            self.clear_auth()?;
        }
        auth.session = Some(session);
        auth.loaded = true;
        Ok(())
    }

    pub(super) fn account(&self) -> AppResult<Option<GitHubAccount>> {
        let auth = self.auth.lock();
        if auth.loaded {
            return Ok(auth.session.as_ref().map(|s| s.account.clone()));
        }
        Ok(self.read::<StoredAccount>(AUTH_KEY)?.map(|s| s.account))
    }

    pub(super) fn authorization(&self) -> Option<LoginAuthorization> {
        self.auth.lock().pending.as_ref().map(PendingLogin::dto)
    }

    pub(super) fn token(&self) -> AppResult<Option<String>> {
        // Only one refresh may rotate the credentials; logout never waits for network I/O.
        let _refresh = self.refresh_gate.lock();
        let (session, generation) = {
            let mut auth = self.auth.lock();
            if !auth.loaded {
                if let Some(stored) = self.read::<StoredAccount>(AUTH_KEY)? {
                    let credentials = if let Some(encrypted) = stored.encrypted_credentials {
                        let json = secrets::unprotect(&encrypted, SECRET_PURPOSE)?;
                        serde_json::from_str(&json).map_err(|_| error("clientReauthRequired"))?
                    } else {
                        Credentials {
                            access_token: secrets::unprotect(
                                &stored
                                    .encrypted_token
                                    .ok_or_else(|| error("clientReauthRequired"))?,
                                SECRET_PURPOSE,
                            )?,
                            refresh_token: None,
                            expires_at: None,
                            refresh_expires_at: None,
                        }
                    };
                    auth.session = Some(SessionAccount {
                        account: stored.account,
                        credentials,
                    });
                }
                auth.loaded = true;
            }
            let Some(session) = &auth.session else {
                return Ok(None);
            };
            if session
                .credentials
                .expires_at
                .is_none_or(|expiry| expiry > chrono::Utc::now().timestamp() + 60)
            {
                return Ok(Some(session.credentials.access_token.clone()));
            }
            (session.clone(), auth.generation)
        };
        let result = match session.credentials.refresh_token.as_deref() {
            Some(refresh)
                if session
                    .credentials
                    .refresh_expires_at
                    .is_some_and(|expiry| expiry > chrono::Utc::now().timestamp()) =>
            {
                self.oauth()?.refresh(refresh)
            }
            _ => Err(error("clientReauthRequired")),
        };
        let mut auth = self.auth.lock();
        if auth.generation != generation {
            return Err(error("clientReauthRequired"));
        }
        match result {
            Ok(credentials) => {
                let token = credentials.access_token.clone();
                self.persist(
                    &mut auth,
                    SessionAccount {
                        account: session.account,
                        credentials,
                    },
                )?;
                Ok(Some(token))
            }
            Err(err) => {
                if err.code == "clientReauthRequired" {
                    self.clear_auth()?;
                    auth.session = None;
                    auth.generation += 1;
                }
                Err(err)
            }
        }
    }

    pub fn start_login(&self, remember: bool) -> AppResult<LoginAuthorization> {
        let generation = {
            let mut auth = self.auth.lock();
            auth.generation += 1;
            auth.pending = None;
            auth.generation
        };
        let code = self.oauth()?.start()?;
        let mut auth = self.auth.lock();
        if auth.generation != generation {
            return Err(error("clientOAuthCancelled"));
        }
        let pending = PendingLogin {
            id: uuid::Uuid::new_v4().to_string(),
            device_code: code.device_code,
            user_code: code.user_code,
            expires_at: chrono::Utc::now().timestamp() + code.expires_in as i64,
            interval: code.interval,
            next_poll: Instant::now() + Duration::from_secs(code.interval),
            in_flight: false,
            remember,
        };
        let dto = pending.dto();
        auth.pending = Some(pending);
        Ok(dto)
    }

    pub fn cancel_login(&self, id: &str) {
        let mut auth = self.auth.lock();
        if auth.pending.as_ref().is_some_and(|p| p.id == id) {
            auth.pending = None;
            auth.generation += 1;
        }
    }

    pub fn poll_login(&self, id: &str) -> AppResult<LoginPoll> {
        let (device_code, remember, generation) = {
            let mut auth = self.auth.lock();
            let generation = auth.generation;
            let pending = auth
                .pending
                .as_mut()
                .filter(|p| p.id == id)
                .ok_or_else(|| error("clientOAuthCancelled"))?;
            if pending.expires_at <= chrono::Utc::now().timestamp() {
                auth.pending = None;
                return Err(error("clientOAuthExpired"));
            }
            if pending.in_flight || pending.next_poll > Instant::now() {
                return Ok(LoginPoll {
                    status: "pending".into(),
                    authorization: Some(pending.dto()),
                });
            }
            pending.in_flight = true;
            pending.next_poll = Instant::now() + Duration::from_secs(pending.interval);
            (pending.device_code.clone(), pending.remember, generation)
        };
        let result = self
            .oauth()
            .and_then(|oauth| oauth.poll(&device_code))
            .and_then(|result| {
                if let TokenResult::Authorized(credentials) = result {
                    // Recheck the signed-in GitHub identity and this private repository before accepting login.
                    let account = self
                        .github_token(credentials.access_token.clone())?
                        .account(remember)?;
                    Ok((TokenResult::Authorized(credentials), Some(account)))
                } else {
                    Ok((result, None))
                }
            });
        let mut auth = self.auth.lock();
        if auth.generation != generation || auth.pending.as_ref().is_none_or(|p| p.id != id) {
            return Err(error("clientOAuthCancelled"));
        }
        let pending = auth.pending.as_mut().unwrap();
        pending.in_flight = false;
        match result {
            Ok((TokenResult::Pending, _)) => {}
            Ok((TokenResult::SlowDown(interval), _)) => {
                pending.interval = interval.unwrap_or(0).max(pending.interval + 5).min(900);
                pending.next_poll = Instant::now() + Duration::from_secs(pending.interval);
            }
            Ok((TokenResult::Authorized(credentials), Some(account))) => {
                self.persist(
                    &mut auth,
                    SessionAccount {
                        account,
                        credentials,
                    },
                )?;
                auth.pending = None;
                auth.generation += 1;
                return Ok(LoginPoll {
                    status: "authenticated".into(),
                    authorization: None,
                });
            }
            Ok(_) => return Err(error("clientResponse")),
            Err(err) => {
                if !matches!(err.code.as_str(), "clientNetwork" | "clientRateLimit") {
                    auth.pending = None;
                }
                return Err(err);
            }
        }
        Ok(LoginPoll {
            status: "pending".into(),
            authorization: auth.pending.as_ref().map(PendingLogin::dto),
        })
    }

    pub fn logout(&self) -> AppResult<super::GameClientState> {
        {
            let mut auth = self.auth.lock();
            self.clear_auth()?;
            auth.session = None;
            auth.loaded = true;
            auth.pending = None;
            auth.generation += 1;
        }
        self.state()
    }

    #[cfg(test)]
    pub(super) fn login_for_read_test(&self, token: String) -> AppResult<super::GameClientState> {
        let account = self.github_token(token.clone())?.account(false)?;
        self.persist(
            &mut self.auth.lock(),
            SessionAccount {
                account,
                credentials: Credentials {
                    access_token: token,
                    refresh_token: None,
                    expires_at: None,
                    refresh_expires_at: None,
                },
            },
        )?;
        self.state()
    }
}
