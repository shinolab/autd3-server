use std::{io::Write, path::Path};

mod npm;
mod rs;

fn main() -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    let mut file = std::fs::File::create(root.join("NOTICE"))?;
    {
        let mut writer = std::io::BufWriter::new(&mut file);
        writeln!(writer, "THIRD-PARTY SOFTWARE NOTICES AND INFORMATION")?;
        writeln!(writer)?;
        writeln!(
            writer,
            "This software includes the following third-party components."
        )?;
        writeln!(writer)?;
        writeln!(writer)?;
    }

    let npm_deps = npm::get_npm_deps(root.join("node_modules"), root.join("package-lock.json"))?;
    file.write_all(npm_deps.as_bytes())?;

    let rs_deps = rs::get_rs_deps(&root)?;
    file.write_all(rs_deps.as_bytes())?;

    Ok(())
}
