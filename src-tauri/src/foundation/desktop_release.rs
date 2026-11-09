use super::{AppError, AppResult};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};

const REPO: &str = "sxg15/ABYADesktopDevelopmentTool";
const MANIFEST: &str = "Publish/build-manifest.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRelease {
    release_number: u64,
    version: String,
    notes: String,
    commit: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    product: String,
    version: String,
    release_number: Option<u64>,
    release_notes: Option<String>,
}

fn invalid() -> AppError {
    AppError::validation("Invalid desktop release metadata")
}

fn release(bytes: &[u8], sha: &str) -> AppResult<Option<DesktopRelease>> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let manifest: Manifest = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if manifest.product != "ABYA Desktop Development Tool" || manifest.version.is_empty() {
        return Err(invalid());
    }
    let Some(number) = manifest.release_number else {
        return Ok(None); // Legacy dev packages predate desktop update notifications.
    };
    let notes = manifest
        .release_notes
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(invalid)?;
    if number == 0 || number > (1 << 53) - 1 {
        return Err(invalid());
    }
    Ok(Some(DesktopRelease {
        release_number: number,
        version: manifest.version,
        notes,
        commit: sha.to_owned(),
    }))
}

pub fn latest() -> AppResult<Option<DesktopRelease>> {
    let client = Client::builder()
        .user_agent("ABYA-Desktop-Update")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(network)?;
    let refs = get(
        &client,
        format!("https://github.com/{REPO}.git/info/refs?service=git-upload-pack"),
        1024 * 1024,
    )?;
    let sha = dev_commit(&refs)?;
    let bytes = get(
        &client,
        format!("https://raw.githubusercontent.com/{REPO}/{sha}/{MANIFEST}"),
        65536,
    )?;
    release(&bytes, sha)
}

// Git smart HTTP v0 advertises each ref in a length-prefixed packet.
fn dev_commit(mut data: &[u8]) -> AppResult<&str> {
    while !data.is_empty() {
        let header = data.get(..4).ok_or_else(invalid)?;
        let text = std::str::from_utf8(header).map_err(|_| invalid())?;
        let size = usize::from_str_radix(text, 16).map_err(|_| invalid())?;
        if size == 0 {
            data = &data[4..];
            continue;
        }
        let packet = data.get(4..size).ok_or_else(invalid)?;
        data = &data[size..];
        let line = packet
            .split(|b| *b == 0 || *b == b'\n')
            .next()
            .unwrap_or_default();
        if let Some(sha) = line.strip_suffix(b" refs/heads/dev")
            && sha.len() == 40
            && sha.iter().all(u8::is_ascii_hexdigit)
        {
            return std::str::from_utf8(sha).map_err(|_| invalid());
        }
    }
    Err(invalid())
}

fn get(client: &Client, url: String, limit: u64) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();
    client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(network)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid());
    }
    Ok(bytes)
}

fn network(_: reqwest::Error) -> AppError {
    AppError::new(
        "desktopUpdateUnavailable",
        "GitHub release check unavailable",
        "",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dev_ref_is_exact_and_truncated_packets_fail() {
        let sha = "abcdef0123456789abcdef0123456789abcdef01";
        let packet = |name: &str| {
            let data = format!("{sha} refs/heads/{name}\n");
            format!("{:04x}{data}", data.len() + 4)
        };
        let bytes = format!(
            "001e# service=git-upload-pack\n0000{}{}0000",
            packet("dev-old"),
            packet("dev")
        );
        assert_eq!(dev_commit(bytes.as_bytes()).unwrap(), sha);
        assert!(dev_commit(packet("dev-old").as_bytes()).is_err());
        assert!(dev_commit(b"0003").is_err());
        assert!(dev_commit(b"0099short").is_err());
    }
    #[test]
    fn supports_legacy_and_numbered_manifests_with_bom() {
        let legacy = br#"{"product":"ABYA Desktop Development Tool","version":"0.4.0"}"#;
        assert!(release(legacy, "x").unwrap().is_none());
        let bytes = b"\xef\xbb\xbf{\"product\":\"ABYA Desktop Development Tool\",\"version\":\"0.4.0\",\"releaseNumber\":2,\"releaseNotes\":\"updated\"}";
        assert_eq!(release(bytes, "x").unwrap().unwrap().release_number, 2);
    }
    #[test]
    #[ignore = "Reads the actual public dev ref and its manifest"]
    fn live_desktop_release_check() {
        latest().unwrap();
    }
}
