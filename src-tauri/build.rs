use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };

    std::fs::copy(
        manifest_dir.join(format!("../tmp/autd3-simulator{ext}")),
        manifest_dir.join(format!(
            "autd3-simulator-{}{}",
            std::env::var("TARGET")?,
            ext
        )),
    )?;
    std::fs::copy(
        manifest_dir.join(format!("../tmp/autd3-simulator-unity{ext}")),
        manifest_dir.join(format!(
            "autd3-simulator-unity-{}{}",
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

    // NOTICE
    let license_path = manifest_dir.join("NOTICE");
    if license_path.exists() {
        std::fs::remove_file(&license_path)?;
    }
    std::fs::copy(manifest_dir.join("../NOTICE"), license_path)?;

    // bundle license file
    let license_path = manifest_dir.join("LICENSE.rtf");
    let license_file = File::create(license_path)?;
    let mut writer = BufWriter::new(license_file);

    {
        let mut content = String::new();
        let f = File::open(manifest_dir.join("LICENSE"))?;
        let mut reader = BufReader::new(f);
        reader.read_to_string(&mut content)?;
        writer.write_all(content.as_bytes())?;
    }
    writeln!(
        writer,
        "
---------------------------------------------------------"
    )?;
    {
        let mut content = String::new();
        let f = File::open(manifest_dir.join("NOTICE"))?;
        let mut reader = BufReader::new(f);
        reader.read_to_string(&mut content)?;
        writer.write_all(content.as_bytes())?;
    }

    writer.flush()?;

    tauri_build::build();

    Ok(())
}
