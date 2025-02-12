use std::{
    collections::HashMap,
    fs,
    io::{BufReader, Read},
    path::Path,
};

use crate::Dependency;

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

pub fn collect_npm_deps<P1, P2>(
    node_modules_path: P1,
    package_lock_json_path: P2,
) -> anyhow::Result<Vec<Dependency>>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
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
    .map(|entry| -> anyhow::Result<Option<Dependency>> {
        let entry = entry?;
        let mut file_content = String::new();
        fs::File::open(&entry)
            .map(BufReader::new)?
            .read_to_string(&mut file_content)?;

        if let Ok(package) = serde_json::from_str::<PackageJson>(&file_content) {
            if dev_packages.contains(&package.name) {
                return Ok(None);
            }
            Ok(Some(Dependency {
                name: package.name,
                version: package.version,
                license: Some(package.license),
                license_file: None,
                repository: match package.repository {
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
                },
            }))
        } else {
            Ok(None)
        }
    })
    .collect::<anyhow::Result<Vec<_>>>()?
    .into_iter()
    .filter_map(std::convert::identity)
    .collect())
}
