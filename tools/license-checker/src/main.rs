mod diff;
mod npm;
mod rs;

use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};

#[derive(Debug)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let npm_deps = npm::collect_npm_deps(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../node_modules"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../package-lock.json"),
    )?;

    let rs_deps = rs::collect_rs_deps(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/Cargo.toml"),
    )?;

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");

    let old = path.parent().unwrap().join("NOTICE");
    let new = path.parent().unwrap().join("NOTICE-new");

    let mut writer = BufWriter::new(File::create(&new)?);

    writeln!(
        writer,
        r"THIRD-PARTY SOFTWARE NOTICES AND INFORMATION

This software includes the following third-party components.
The license terms for each of these components are provided later in this notice.
"
    )?;

    let mut licenses = std::collections::HashSet::new();
    npm_deps
        .into_iter()
        .chain(rs_deps.into_iter())
        .try_for_each(|dep| -> anyhow::Result<()> {
            writeln!(
                writer,
                r"---------------------------------------------------------
"
            )?;

            if dep.license.is_none() && dep.license_file.is_none() {
                return Err(anyhow::anyhow!(
                    "No license information found for {} {}",
                    dep.name,
                    dep.version
                ));
            }

            writeln!(
                writer,
                "{} {}{}",
                dep.name,
                dep.version,
                if let Some(license) = dep.license {
                    let license = if license == "Apache-2.0 OR LGPL-2.1-or-later OR MIT" {
                        "Apache-2.0 OR MIT".to_string()
                    } else {
                        license
                    };
                    license
                        .split("AND")
                        .flat_map(|token| {
                            token
                                .split("OR")
                                .map(|token| token.trim().trim_matches(&['(', ')']))
                        })
                        .for_each(|license| {
                            licenses.insert(license.to_string());
                        });
                    format!(" ({})", license)
                } else {
                    "".to_string()
                }
            )?;
            if let Some(rep) = dep.repository {
                writeln!(writer, "{}", rep)?;
            }

            if let Some(license_file) = dep.license_file {
                writeln!(
                    writer,
                    r"
---
            {}",
                    license_file
                )?;
            }

            Ok(())
        })?;

    writeln!(
        writer,
        r"---------------------------------------------------------
        
LICENSE TERMS"
    )?;

    let mut licenses = licenses.into_iter().collect::<Vec<_>>();
    licenses.sort();
    licenses
        .into_iter()
        .filter(|license| license != "Unlicense")
        .try_for_each(|license| -> anyhow::Result<()> {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("licenses")
                .join(&license);

            writeln!(
                writer,
                r"
---------------------------------------------------------
{}
---
",
                license
            )?;
            let mut file_content = String::new();
            if let Ok(mut reader) = fs::File::open(path).map(BufReader::new) {
                reader.read_to_string(&mut file_content)?;
            } else {
                return Err(anyhow::anyhow!("License file not found for {}", license));
            }
            writer.write_all(file_content.as_bytes())?;

            Ok(())
        })?;

    writeln!(
        writer,
        r"
---------------------------------------------------------"
    )?;

    writer.flush()?;

    let changed = if old.exists() {
        let mut old_license = String::new();
        fs::File::open(&old)
            .map(BufReader::new)?
            .read_to_string(&mut old_license)?;
        let old_license = old_license.replace("\r", "");

        let mut new_license = String::new();
        fs::File::open(&new)
            .map(BufReader::new)?
            .read_to_string(&mut new_license)?;
        let new_license = new_license.replace("\r", "");

        diff::show_diff(&old_license, &new_license)
    } else {
        false
    };

    std::fs::remove_file(&old)?;
    std::fs::rename(new, old)?;

    if changed {
        return Err(anyhow::anyhow!(
            "Some NOTICE files have been updated. Manuall check is required.",
        ));
    }

    Ok(())
}
