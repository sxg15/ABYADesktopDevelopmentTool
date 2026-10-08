use super::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub version: String,
    pub release: String,
    pub built_at: String,
    pub verification: String,
    pub data_directory: String,
    pub executable: String,
    pub test_environment: bool,
}

pub fn read(paths: &AppPaths) -> AppResult<BuildInfo> {
    let exe = std::env::current_exe()?;
    let manifest = exe.parent().unwrap().join("build-manifest.json");
    let mut verification = "missing";
    if let Ok(bytes) = std::fs::read(&manifest) {
        let bytes = bytes.strip_prefix(&[239, 187, 191]).unwrap_or(&bytes);
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
            verification = "mismatch";
            let mut file = std::fs::File::open(&exe)?;
            let mut hash = Sha256::new();
            let mut block = [0; 65536];
            loop {
                let n = file.read(&mut block)?;
                if n == 0 {
                    break;
                }
                hash.update(&block[..n]);
            }
            let actual = format!("{:x}", hash.finalize());
            if value["releaseLabel"] == env!("ABYA_RELEASE_ID")
                && value["version"] == env!("CARGO_PKG_VERSION")
                && value["sha256"]
                    .as_str()
                    .is_some_and(|h| h.eq_ignore_ascii_case(&actual))
            {
                verification = "verified";
            }
        }
    }
    Ok(BuildInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        release: env!("ABYA_RELEASE_ID").into(),
        built_at: env!("ABYA_BUILD_UTC").into(),
        verification: verification.into(),
        data_directory: paths.data_dir.to_string_lossy().into(),
        executable: exe.to_string_lossy().into(),
        test_environment: std::env::var("ABYA_TEST_MODE").as_deref() == Ok("1"),
    })
}
