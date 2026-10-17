//! GitHub device authorization. Secrets never cross the native service boundary.
use super::github::{error, network_error};
use crate::foundation::AppResult;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{io::Read, time::Duration};

pub(super) const CLIENT_ID: &str = "Ov23li0rDQSvLA8yT7sj";
pub(super) const VERIFICATION_URI: &str = "https://github.com/login/device";

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Credentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub refresh_expires_at: Option<i64>,
}

#[derive(Deserialize)]
pub(super) struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    refresh_token_expires_in: Option<u64>,
    error: Option<String>,
    interval: Option<u64>,
}

pub(super) enum TokenResult {
    Pending,
    SlowDown(Option<u64>),
    Authorized(Credentials),
}

pub(super) struct OAuth {
    client: Client,
    #[cfg(test)]
    pub base: Option<String>,
}

impl OAuth {
    #[cfg(test)]
    pub fn with_test_base(base: String) -> AppResult<Self> {
        Ok(Self {
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(network_error)?,
            base: Some(base),
        })
    }

    pub fn new() -> AppResult<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent("ABYA-Desktop-GameClients/0.4.0")
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(network_error)?,
            #[cfg(test)]
            base: None,
        })
    }

    fn post<T: DeserializeOwned>(&self, path: &str, form: &[(&str, &str)]) -> AppResult<T> {
        let base = "https://github.com";
        #[cfg(test)]
        let base = self.base.as_deref().unwrap_or(base);
        let response = self
            .client
            .post(format!("{base}{path}"))
            .header(reqwest::header::ACCEPT, "application/json")
            .form(form)
            .send()
            .map_err(network_error)?;
        if !response.status().is_success() {
            return Err(error(if response.status().as_u16() == 429 {
                "clientRateLimit"
            } else {
                "clientOAuthUnavailable"
            }));
        }
        let mut bytes = Vec::new();
        response
            .take(32_769)
            .read_to_end(&mut bytes)
            .map_err(|_| error("clientNetwork"))?;
        if bytes.len() > 32_768 {
            return Err(error("clientResponse"));
        }
        // Never return a parser error containing response data.
        serde_json::from_slice(&bytes).map_err(|_| error("clientResponse"))
    }

    pub fn start(&self) -> AppResult<DeviceCode> {
        let value: serde_json::Value = self.post(
            "/login/device/code",
            &[("client_id", CLIENT_ID), ("scope", "repo")],
        )?;
        if let Some(code) = value["error"].as_str() {
            return Err(oauth_error(code));
        }
        let code: DeviceCode =
            serde_json::from_value(value).map_err(|_| error("clientResponse"))?;
        if !valid_secret(&code.device_code)
            || code.user_code.len() > 32
            || code.user_code.is_empty()
            || !code
                .user_code
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'-')
            || code.verification_uri != VERIFICATION_URI
            || !(1..=900).contains(&code.expires_in)
            || !(1..=300).contains(&code.interval)
        {
            return Err(error("clientResponse"));
        }
        Ok(code)
    }

    pub fn poll(&self, device_code: &str) -> AppResult<TokenResult> {
        let response: TokenResponse = self.post(
            "/login/oauth/access_token",
            &[
                ("client_id", CLIENT_ID),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ],
        )?;
        match response.error.as_deref() {
            Some("authorization_pending") => Ok(TokenResult::Pending),
            Some("slow_down") => Ok(TokenResult::SlowDown(response.interval)),
            Some(code) => Err(oauth_error(code)),
            None => credentials(response).map(TokenResult::Authorized),
        }
    }

    pub fn refresh(&self, refresh_token: &str) -> AppResult<Credentials> {
        let response: TokenResponse = self.post(
            "/login/oauth/access_token",
            &[
                ("client_id", CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ],
        )?;
        if let Some(code) = response.error.as_deref() {
            return Err(match code {
                "bad_refresh_token" | "invalid_grant" | "expired_token" | "access_denied" => {
                    error("clientReauthRequired")
                }
                _ => oauth_error(code),
            });
        }
        let value = credentials(response)?;
        if value.refresh_token.is_none() || value.expires_at.is_none() {
            return Err(error("clientResponse"));
        }
        Ok(value)
    }
}

fn valid_secret(value: &str) -> bool {
    !value.is_empty() && value.len() <= 8192 && value.bytes().all(|c| c.is_ascii_graphic())
}

fn credentials(response: TokenResponse) -> AppResult<Credentials> {
    let access_token = response
        .access_token
        .filter(|s| valid_secret(s))
        .ok_or_else(|| error("clientResponse"))?;
    if !response
        .token_type
        .is_some_and(|s| s.eq_ignore_ascii_case("bearer"))
        || !response
            .scope
            .is_some_and(|s| s.split([',', ' ']).any(|scope| scope == "repo"))
    {
        return Err(error("clientOAuthScope"));
    }
    let expires_at = expiry(response.expires_in)?;
    let refresh_expires_at = expiry(response.refresh_token_expires_in)?;
    if let Some(refresh) = &response.refresh_token {
        if !valid_secret(refresh) || expires_at.is_none() || refresh_expires_at.is_none() {
            return Err(error("clientResponse"));
        }
    } else if expires_at.is_some() {
        return Err(error("clientResponse"));
    }
    Ok(Credentials {
        access_token,
        refresh_token: response.refresh_token,
        expires_at,
        refresh_expires_at,
    })
}

fn expiry(seconds: Option<u64>) -> AppResult<Option<i64>> {
    seconds
        .map(|seconds| {
            if seconds == 0 || seconds > 366 * 24 * 3600 {
                return Err(error("clientResponse"));
            }
            Ok(chrono::Utc::now().timestamp() + seconds as i64)
        })
        .transpose()
}

fn oauth_error(code: &str) -> crate::foundation::AppError {
    error(match code {
        "access_denied" => "clientOAuthDenied",
        "expired_token" | "incorrect_device_code" => "clientOAuthExpired",
        "device_flow_disabled" | "incorrect_client_credentials" | "invalid_client" => {
            "clientOAuthUnavailable"
        }
        "incorrect_scope" => "clientOAuthScope",
        _ => "clientOAuthUnavailable",
    })
}
