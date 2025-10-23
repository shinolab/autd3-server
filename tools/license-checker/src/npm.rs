use std::{
    collections::HashMap,
    fs,
    io::Write,
    io::{BufReader, Read},
    path::Path,
};

use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct PackageLockModuleJson {
    pub dev: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct PackageLockJson {
    pub packages: HashMap<String, PackageLockModuleJson>,
}

#[derive(Deserialize, Debug)]
struct PackageJson {
    pub name: String,
    pub version: String,
    pub repository: serde_json::Value,
    pub license: String,
}

fn collect_npm_deps<P1: AsRef<Path>, P2: AsRef<Path>>(
    node_modules_path: P1,
    package_lock_json_path: P2,
) -> anyhow::Result<Vec<PackageJson>> {
    let mut package_lock_json = String::new();
    fs::File::open(package_lock_json_path)
        .map(BufReader::new)?
        .read_to_string(&mut package_lock_json)?;

    let dev_packages =
        if let Ok(package_lock) = serde_json::from_str::<PackageLockJson>(&package_lock_json) {
            package_lock
                .packages
                .into_iter()
                .filter_map(|(name, module)| {
                    if module.dev.unwrap_or(false) {
                        Some(name.replace("node_modules/", ""))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

    Ok(glob::glob(&format!(
        "{}/{}",
        node_modules_path.as_ref().to_str().unwrap(),
        "**/package.json"
    ))?
    .map(|entry| -> anyhow::Result<Option<PackageJson>> {
        let entry = entry?;
        let mut file_content = String::new();
        fs::File::open(&entry)
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;

        if let Ok(package) = serde_json::from_str::<PackageJson>(&file_content) {
            if dev_packages.contains(&package.name) {
                return Ok(None);
            }
            Ok(Some(package))
        } else {
            eprintln!("failed to parse package.json: {}", entry.display());
            Ok(None)
        }
    })
    .collect::<anyhow::Result<Vec<_>>>()?
    .into_iter()
    .filter_map(std::convert::identity)
    .collect())
}

pub fn get_npm_deps<P1: AsRef<Path>, P2: AsRef<Path>>(
    node_modules_path: P1,
    package_lock_json_path: P2,
) -> anyhow::Result<String> {
    let deps = collect_npm_deps(node_modules_path, package_lock_json_path)?;

    let mut writer = std::io::BufWriter::new(Vec::new());

    for dep in deps {
        writeln!(writer)?;
        writeln!(
            writer,
            "---------------------------------------------------------"
        )?;
        writeln!(writer)?;
        writeln!(writer, "{} {} ({})", dep.name, dep.version, dep.license)?;
        let repo = match dep.repository {
            serde_json::Value::String(rep) => Some(rep),
            serde_json::Value::Object(map) => {
                if let Some(rep) = map.get("url") {
                    rep.as_str()
                        .map(|s| s.trim_start_matches("git+").to_owned())
                } else {
                    anyhow::bail!("invalid repository type");
                }
            }
            _ => anyhow::bail!("invalid repository type"),
        };
        if let Some(repo) = repo {
            writeln!(writer, "{}", repo)?;
        }
        writeln!(writer)?;
        writeln!(
            writer,
            "---------------------------------------------------------"
        )?;
    }

    Ok(String::from_utf8(writer.into_inner()?)?)
}
