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
        manifest_dir.join(format!("../target/release/simulator{}", ext)),
        manifest_dir.join(format!("simulator-{}{}", std::env::var("TARGET")?, ext)),
    )?;
    std::fs::copy(
        manifest_dir.join(format!("../target/release/simulator-unity{}", ext)),
        manifest_dir.join(format!(
            "simulator-unity-{}{}",
            std::env::var("TARGET")?,
            ext
        )),
    )?;

    std::fs::copy(
        manifest_dir.join(format!("../target/release/SOEMAUTDServer{}", ext)),
        manifest_dir.join(format!(
            "SOEMAUTDServer-{}{}",
            std::env::var("TARGET")?,
            ext
        )),
    )?;

    std::fs::copy(
        manifest_dir.join(format!(
            "../target/release/TwinCATAUTDServerLightweight{}",
            ext
        )),
        manifest_dir.join(format!(
            "TwinCATAUTDServerLightweight-{}{}",
            std::env::var("TARGET")?,
            ext
        )),
    )?;

    // NOTICE
    let notice_path = manifest_dir.join("NOTICE");
    if notice_path.exists() {
        std::fs::remove_file(&notice_path)?;
    }
    let mut writer = BufWriter::new(File::create(&notice_path)?);

    {
        let mut file_content = String::new();
        File::open(manifest_dir.join("../ThirdPartyNotice.txt"))
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;
        writer.write_all(file_content.as_bytes())?;
    }
    {
        let mut file_content = String::new();
        File::open(manifest_dir.join("ThirdPartyNotice.txt"))
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;
        writeln!(writer)?;
        writeln!(
            writer,
            "========================================================="
        )?;
        writeln!(writer)?;
        writer.write_all(file_content.as_bytes())?;
    }
    {
        let mut file_content = String::new();
        File::open(manifest_dir.join("../simulator/ThirdPartyNotice.txt"))
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;
        writeln!(writer)?;
        writeln!(
            writer,
            "========================================================="
        )?;
        writeln!(writer)?;
        write!(writer, "AUTD SIMULATOR ")?;
        writer.write_all(file_content.as_bytes())?;
    }
    {
        let mut file_content = String::new();
        File::open(manifest_dir.join("../SOEMAUTDServer/ThirdPartyNotice.txt"))
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;
        writeln!(writer)?;
        writeln!(
            writer,
            "========================================================="
        )?;
        writeln!(writer)?;
        write!(writer, "SOEMAUTDServer ")?;
        writer.write_all(file_content.as_bytes())?;
    }

    {
        let mut file_content = String::new();
        File::open(manifest_dir.join("../TwinCATAUTDServerLightweight/ThirdPartyNotice.txt"))
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;
        writeln!(writer)?;
        writeln!(
            writer,
            "========================================================="
        )?;
        writeln!(writer)?;
        write!(writer, "TwinCATAUTDServerLightweight ")?;
        writer.write_all(file_content.as_bytes())?;
    }

    writer.flush()?;

    // LICENSE
    let license_path = manifest_dir.join("LICENSE");
    if license_path.exists() {
        std::fs::remove_file(&license_path)?;
    }
    std::fs::copy(manifest_dir.join("../LICENSE"), license_path)?;

    // Installer
    let license_path = manifest_dir.join("LICENSE.txt");
    if license_path.exists() {
        std::fs::remove_file(&license_path)?;
    }
    let mut writer = BufWriter::new(File::create(&license_path)?);
    {
        let mut file_content = String::new();
        File::open(manifest_dir.join("../LICENSE"))
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;
        writer.write_all(file_content.as_bytes())?;
    }
    {
        let mut file_content = String::new();
        File::open(manifest_dir.join("NOTICE"))
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;
        writeln!(writer)?;
        writeln!(
            writer,
            "========================================================="
        )?;
        writeln!(writer)?;
        writer.write_all(file_content.as_bytes())?;
    }

    // Wix
    let mut file_content = String::new();
    File::open(manifest_dir.join("LICENSE.txt"))
        .map(BufReader::new)?
        .read_to_string(&mut file_content)?;
    let mut writer = BufWriter::new(File::create(manifest_dir.join("LICENSE.rtf"))?);
    writer.write_all(file_content.as_bytes())?;

    writer.flush()?;

    tauri_build::build();

    Ok(())
}
