use serde::{Deserialize, Serialize};

pub const REPOSITORY: &str = "sxg15/PlatverseOutput";
pub const MAX_ASSET_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubAccount {
    pub login: String,
    pub can_manage: bool,
    pub remembered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
    #[serde(default)]
    pub state: String,
}

impl ReleaseAsset {
    pub fn installable(&self) -> bool {
        let name = self.name.to_ascii_lowercase();
        self.state == "uploaded"
            && name.ends_with(".zip")
            && !["macos", "osx", "linux", "android"]
                .iter()
                .any(|s| name.contains(s))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientRelease {
    pub id: u64,
    #[serde(alias = "tag_name")]
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    #[serde(default)]
    pub immutable: bool,
    #[serde(alias = "published_at")]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInstallation {
    pub id: String,
    pub release_id: u64,
    pub tag_name: String,
    pub prerelease: bool,
    pub published_at: Option<String>,
    pub asset_id: u64,
    pub asset_name: String,
    pub asset_updated_at: String,
    pub sha256: String,
    pub digest_verified: bool,
    pub directory: String,
    #[serde(default)]
    pub storage_root: String,
    pub executable_candidates: Vec<String>,
    pub installed_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientOperation {
    pub active: bool,
    pub kind: String,
    pub stage: String,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameClientState {
    pub repository: String,
    pub account: Option<GitHubAccount>,
    pub installations: Vec<ClientInstallation>,
    pub default_storage_root: String,
    pub operation: ClientOperation,
    pub authorization: Option<LoginAuthorization>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginAuthorization {
    pub id: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_at: String,
    pub poll_after_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPoll {
    pub status: String,
    pub authorization: Option<LoginAuthorization>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInput {
    pub id: Option<u64>,
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub draft: bool,
    pub prerelease: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInput {
    pub release_id: u64,
    pub asset_id: u64,
    pub storage_root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUpdate {
    pub release_id: u64,
    pub tag_name: String,
    pub current_tag: String,
}
