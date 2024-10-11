use std::path::Path;

use autd3_license_check::license_file;

fn main() -> anyhow::Result<()> {
    let license_file_map = license_file::load_license_file_map(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("license-file.toml"),
    )?;

    let imgui_wgpu = (
        "MIT",
        r"imgui-wgpu-rs 0.24.0 (MIT)
https://github.com/Yatekii/imgui-wgpu-rs
Modification of the original version by Shun Suzuki <suzuki@hapis.k.u-tokyo.ac.jp>",
    );
    let notofont_dep = ("OFL", "Noto Sans v2.012 (OFL)");

    let changed = autd3_license_check::check(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../simulator/Cargo.toml"),
        "ThirdPartyNotice",
        &license_file_map,
        &[imgui_wgpu, notofont_dep],
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
