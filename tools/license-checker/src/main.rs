use std::path::Path;

use autd3_license_check::license_file;

fn main() -> anyhow::Result<()> {
    let license_file_map = license_file::load_license_file_map(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("license-file.toml"),
    )?;

    let changed = autd3_license_check::check(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../simulator/Cargo.toml"),
        "ThirdPartyNotice",
        &license_file_map,
        &[],
    )?;

    let changed = autd3_license_check::check(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../SOEMAUTDServer/Cargo.toml"),
        "ThirdPartyNotice",
        &license_file_map,
        &[],
    )? || changed;

    let changed = autd3_license_check::check(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../TwinCATAUTDServerLightweight/Cargo.toml"),
        "ThirdPartyNotice",
        &license_file_map,
        &[],
    )? || changed;

    let changed = autd3_license_check::check(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/Cargo.toml"),
        "ThirdPartyNotice",
        &license_file_map,
        &[],
    )? || changed;

    let changed = autd3_license_check::check_npm(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../node_modules"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../package-lock.json"),
        "ThirdPartyNotice",
    )? || changed;

    if changed {
        return Err(anyhow::anyhow!(
            "Some ThirdPartyNotice.txt files have been updated. Manuall check is required.",
        ));
    }

    Ok(())
}
