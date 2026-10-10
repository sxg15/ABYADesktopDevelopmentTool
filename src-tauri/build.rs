fn main() {
    for key in ["ABYA_RELEASE_ID", "ABYA_BUILD_UTC"] {
        println!("cargo:rerun-if-env-changed={key}");
        println!(
            "cargo:rustc-env={key}={}",
            std::env::var(key).unwrap_or_else(|_| "development".into())
        );
    }
    tauri_build::build()
}
