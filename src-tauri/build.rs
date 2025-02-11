use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };

    std::fs::copy(
        manifest_dir.join(format!("../tmp/autd3-simulator{}", ext)),
        manifest_dir.join(format!(
            "autd3-simulator-{}{}",
            std::env::var("TARGET")?,
            ext
        )),
    )?;
    std::fs::copy(
        manifest_dir.join(format!("../tmp/autd3-simulator-unity{}", ext)),
        manifest_dir.join(format!(
            "autd3-simulator-unity-{}{}",
            std::env::var("TARGET")?,
            ext
        )),
    )?;

    std::fs::copy(
        manifest_dir.join(format!("../tmp/SOEMAUTDServer{}", ext)),
        manifest_dir.join(format!(
            "SOEMAUTDServer-{}{}",
            std::env::var("TARGET")?,
            ext
        )),
    )?;

    std::fs::copy(
        manifest_dir.join(format!("../tmp/TwinCATAUTDServerLightweight{}", ext)),
        manifest_dir.join(format!(
            "TwinCATAUTDServerLightweight-{}{}",
            std::env::var("TARGET")?,
            ext
        )),
    )?;

    // LICENSE
    let license_path = manifest_dir.join("LICENSE");
    if license_path.exists() {
        std::fs::remove_file(&license_path)?;
    }
    std::fs::copy(manifest_dir.join("../LICENSE"), license_path)?;

    let license_path = manifest_dir.join("LICENSE.txt");
    if license_path.exists() {
        std::fs::remove_file(&license_path)?;
    }
    std::fs::copy(manifest_dir.join("../LICENSE"), license_path)?;

    let license_path = manifest_dir.join("LICENSE.rtf");
    if license_path.exists() {
        std::fs::remove_file(&license_path)?;
    }
    std::fs::copy(manifest_dir.join("../LICENSE"), license_path)?;

    // NOTICE
    let license_path = manifest_dir.join("NOTICE");
    if license_path.exists() {
        std::fs::remove_file(&license_path)?;
    }
    std::fs::copy(manifest_dir.join("../ThirdPartyNotice.txt"), license_path)?;

    tauri_build::build();

    Ok(())
}
