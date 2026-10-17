use super::models::*;
use crate::foundation::{AppError, AppResult};
use reqwest::{
    Method,
    blocking::{Body, Client, Response},
    header,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{io::Read, time::Duration};

const API: &str = "https://api.github.com";

#[test]
#[ignore = "Public HEAD requests through an existing local system proxy; opt-in only"]
fn live_system_proxy_route() {
    let expected: std::net::SocketAddr = std::env::var("ABYA_TEST_EXPECTED_PROXY")
        .expect("Set the expected local system proxy address")
        .parse()
        .unwrap();
    assert!(expected.ip().is_loopback());
    let github = GitHub::new(None).unwrap();
    for url in [API, "https://release-assets.githubusercontent.com"] {
        let response = github.client.head(url).send().expect("Public HEAD request");
        assert_eq!(response.remote_addr(), Some(expected));
        println!("System proxy route verified: {url} via {expected}");
    }
}

pub(super) fn error(code: &str) -> AppError {
    AppError::new(code, "Game client operation failed.", "")
}

pub(super) fn network_error(_: reqwest::Error) -> AppError {
    // Do not expose request URLs, signed download query strings, or server bodies.
    error("clientNetwork")
}

pub(super) struct GitHub {
    client: Client,
    token: Option<String>,
    #[cfg(test)]
    pub(super) test_api: Option<String>,
}

impl GitHub {
    pub fn new(token: Option<String>) -> AppResult<Self> {
        let client = Client::builder()
            .user_agent("ABYA-Desktop-GameClients/0.4.0")
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(network_error)?;
        Ok(Self {
            client,
            token,
            #[cfg(test)]
            test_api: None,
        })
    }

    #[cfg(test)]
    pub fn with_test_api(token: Option<String>, api: String) -> AppResult<Self> {
        let mut github = Self::new(token)?;
        // 模拟服务器必须独立于开发机的系统代理和环境变量。
        github.client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(network_error)?;
        github.test_api = Some(api);
        Ok(github)
    }

    fn api(&self) -> &str {
        #[cfg(test)]
        if let Some(api) = &self.test_api {
            return api;
        }
        API
    }

    fn request(&self, method: Method, url: String) -> reqwest::blocking::RequestBuilder {
        self.request_with_accept(method, url, "application/vnd.github+json")
    }

    fn request_with_accept(
        &self,
        method: Method,
        url: String,
        accept: &str,
    ) -> reqwest::blocking::RequestBuilder {
        let request = self
            .client
            .request(method, url)
            // RequestBuilder::header appends, so choose the response type once.
            .header(header::ACCEPT, accept)
            .header("X-GitHub-Api-Version", "2022-11-28");
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    fn repo_url(&self, suffix: &str) -> String {
        format!("{}/repos/{REPOSITORY}/{suffix}", self.api())
    }

    pub fn account(&self, remembered: bool) -> AppResult<GitHubAccount> {
        let user: Value = json_response(
            self.request(Method::GET, format!("{}/user", self.api()))
                .send()
                .map_err(network_error)?,
        )?;
        let repo: Value = json_response(
            self.request(Method::GET, format!("{}/repos/{REPOSITORY}", self.api()))
                .send()
                .map_err(network_error)?,
        )?;
        let login = user["login"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| error("clientResponse"))?
            .to_string();
        Ok(GitHubAccount {
            login,
            remembered,
            can_manage: ["push", "maintain", "admin"]
                .iter()
                .any(|key| repo["permissions"][key].as_bool() == Some(true)),
        })
    }

    fn pages<T: DeserializeOwned>(&self, suffix: &str) -> AppResult<Vec<T>> {
        let mut all = Vec::new();
        for page in 1..=100 {
            let items: Vec<T> = json_response(
                self.request(Method::GET, self.repo_url(suffix))
                    .query(&[("per_page", 100), ("page", page)])
                    .send()
                    .map_err(network_error)?,
            )?;
            let last = items.len() < 100;
            all.extend(items);
            if last {
                return Ok(all);
            }
        }
        Err(error("clientTooManyResults"))
    }

    pub fn releases(&self) -> AppResult<Vec<ClientRelease>> {
        self.pages("releases")
    }

    pub fn release(&self, id: u64) -> AppResult<ClientRelease> {
        let mut release: ClientRelease = json_response(
            self.request(Method::GET, self.repo_url(&format!("releases/{id}")))
                .send()
                .map_err(network_error)?,
        )?;
        // Never rely on the embedded asset preview for membership or management.
        release.assets = self.pages(&format!("releases/{id}/assets"))?;
        Ok(release)
    }

    pub fn save_release(&self, input: ReleaseInput) -> AppResult<ClientRelease> {
        if input.tag_name.trim().is_empty()
            || input.tag_name.len() > 128
            || input.name.len() > 256
            || input.body.len() > 65_536
        {
            return Err(error("clientReleaseInput"));
        }
        if let Some(id) = input.id {
            let existing = self.release(id)?;
            if existing.immutable {
                return Err(error("clientImmutable"));
            }
            if existing.tag_name != input.tag_name {
                return Err(error("clientReleaseInput"));
            }
        }
        let (method, suffix) = input
            .id
            .map(|id| (Method::PATCH, format!("releases/{id}")))
            .unwrap_or((Method::POST, "releases".into()));
        json_response(self.request(method, self.repo_url(&suffix)).json(&json!({
            "tag_name": input.tag_name.trim(), "name": input.name.trim(), "body": input.body,
            "draft": input.draft, "prerelease": input.prerelease
        })).send().map_err(network_error)?)
    }

    pub fn delete_release(&self, id: u64) -> AppResult<()> {
        if self.release(id)?.immutable {
            return Err(error("clientImmutable"));
        }
        checked(
            self.request(Method::DELETE, self.repo_url(&format!("releases/{id}")))
                .send()
                .map_err(network_error)?,
        )?;
        Ok(())
    }

    pub fn delete_asset(&self, release_id: u64, asset_id: u64) -> AppResult<()> {
        let release = self.release(release_id)?;
        if release.immutable {
            return Err(error("clientImmutable"));
        }
        if !release.assets.iter().any(|asset| asset.id == asset_id) {
            return Err(error("clientAssetMissing"));
        }
        checked(
            self.request(
                Method::DELETE,
                self.repo_url(&format!("releases/assets/{asset_id}")),
            )
            .send()
            .map_err(network_error)?,
        )?;
        Ok(())
    }

    pub fn upload(&self, release_id: u64, name: &str, body: Body) -> AppResult<ReleaseAsset> {
        let release = self.release(release_id)?;
        if release.immutable {
            return Err(error("clientImmutable"));
        }
        if release.assets.iter().any(|asset| asset.name == name) {
            return Err(error("clientDuplicateAsset"));
        }
        let url =
            format!("https://uploads.github.com/repos/{REPOSITORY}/releases/{release_id}/assets");
        #[cfg(test)]
        let url = self
            .test_api
            .as_ref()
            .map(|_| self.repo_url(&format!("releases/{release_id}/assets")))
            .unwrap_or(url);
        json_response(
            self.request(Method::POST, url)
                .query(&[("name", name)])
                .header(
                    header::CONTENT_TYPE,
                    if name.to_ascii_lowercase().ends_with(".zip") {
                        "application/zip"
                    } else {
                        "application/octet-stream"
                    },
                )
                .timeout(Duration::from_secs(7200))
                .body(body)
                .send()
                .map_err(network_error)?,
        )
    }

    pub fn download(&self, asset_id: u64) -> AppResult<Response> {
        let mut response = self
            .request_with_accept(
                Method::GET,
                self.repo_url(&format!("releases/assets/{asset_id}")),
                "application/octet-stream",
            )
            .timeout(Duration::from_secs(7200))
            .send()
            .map_err(network_error)?;
        for _ in 0..3 {
            if !response.status().is_redirection() {
                return download_response(response);
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| error("clientDownloadHost"))?;
            let url = reqwest::Url::parse(location).map_err(|_| error("clientDownloadHost"))?;
            if !safe_download_url(&url) {
                return Err(error("clientDownloadHost"));
            }
            // Signed asset CDN requests NEVER receive the GitHub Authorization header.
            response = self
                .client
                .get(url)
                .timeout(Duration::from_secs(7200))
                .send()
                .map_err(network_error)?;
        }
        Err(error("clientDownloadHost"))
    }
}

fn download_response(response: Response) -> AppResult<Response> {
    let response = checked(response)?;
    for value in response.headers().get_all(header::CONTENT_TYPE).iter() {
        let media_type = value
            .to_str()
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if media_type.starts_with("text/")
            || matches!(media_type.as_str(), "application/json" | "application/xml")
            || media_type.ends_with("+json")
            || media_type.ends_with("+xml")
        {
            return Err(error("clientDownloadResponse"));
        }
    }
    Ok(response)
}

pub(super) fn safe_download_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && matches!(
            url.host_str(),
            Some(
                "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
                    | "github-releases.githubusercontent.com"
            )
        )
}

fn checked(response: Response) -> AppResult<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let code = match status.as_u16() {
        401 => "clientUnauthorized",
        403 | 429
            if response.headers().contains_key("retry-after")
                || response
                    .headers()
                    .get("x-ratelimit-remaining")
                    .is_some_and(|v| v == "0") =>
        {
            "clientRateLimit"
        }
        403 => "clientForbidden",
        404 => "clientNotFound",
        409 | 422 => "clientConflict",
        429 => "clientRateLimit",
        _ => "clientServer",
    };
    Err(error(code))
}

fn json_response<T: DeserializeOwned>(response: Response) -> AppResult<T> {
    let mut bytes = Vec::new();
    checked(response)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("clientNetwork"))?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(error("clientResponse"));
    }
    serde_json::from_slice(&bytes).map_err(|_| error("clientResponse"))
}
