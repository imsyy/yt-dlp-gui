fn main() {
    println!("cargo::rustc-check-cfg=cfg(mobile)");
    tauri_build::build();

    #[cfg(feature = "prebuilt-ytdlp")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-env-changed=PREBUILT_YTDLP_PATH");

        use std::env;
        use std::fs;
        use std::path::PathBuf;

        let configured_path = env::var_os("PREBUILT_YTDLP_PATH").map(PathBuf::from);
        let prebuilt_path = if let Some(path) = configured_path {
            path
        } else {
            let target_os = env::var("CARGO_CFG_TARGET_OS").expect("Cargo target OS is set");
            let target_arch =
                env::var("CARGO_CFG_TARGET_ARCH").expect("Cargo target architecture is set");
            let (file_name, asset_name) = match (target_os.as_str(), target_arch.as_str()) {
                ("windows", "x86_64") => ("yt-dlp.exe", "yt-dlp.exe"),
                ("windows", "aarch64") => ("yt-dlp.exe", "yt-dlp.exe"),
                ("macos", "x86_64") => ("yt-dlp_macos", "yt-dlp_macos"),
                ("macos", "aarch64") => ("yt-dlp_macos", "yt-dlp_macos"),
                ("linux", "x86_64") => ("yt-dlp_linux", "yt-dlp_linux"),
                ("linux", "aarch64") => ("yt-dlp_linux_aarch64", "yt-dlp_linux_aarch64"),
                _ => panic!("Unsupported prebuilt yt-dlp target: {target_os}-{target_arch}"),
            };
            let out_path =
                PathBuf::from(env::var("OUT_DIR").expect("Cargo OUT_DIR is set")).join(file_name);
            let url =
                format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{asset_name}");
            let response = reqwest::blocking::get(url)
                .and_then(reqwest::blocking::Response::error_for_status)
                .expect("Failed to download prebuilt yt-dlp");
            let bytes = response
                .bytes()
                .expect("Failed to read prebuilt yt-dlp bytes");
            fs::write(&out_path, bytes).expect("Failed to write prebuilt yt-dlp");

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&out_path, fs::Permissions::from_mode(0o755))
                    .expect("Failed to set prebuilt yt-dlp permissions");
            }
            out_path
        };

        let prebuilt_path = prebuilt_path
            .canonicalize()
            .expect("Failed to resolve prebuilt yt-dlp path");
        if !prebuilt_path.is_file() {
            panic!(
                "PREBUILT_YTDLP_PATH is not a file: {}",
                prebuilt_path.display()
            );
        }

        println!(
            "cargo:rustc-env=PREBUILT_YTDLP_PATH={}",
            prebuilt_path.display()
        );
    }
}
