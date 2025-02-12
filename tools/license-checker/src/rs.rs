use std::{
    io::Read,
    path::{Path, PathBuf},
};

use crate::Dependency;
use cargo_license::{get_dependencies_from_cargo_lock, GetDependenciesOpt};
use cargo_metadata::MetadataCommand;

pub fn collect_rs_deps<P>(path: P) -> anyhow::Result<Vec<Dependency>>
where
    P: Into<PathBuf>,
{
    let mut cmd = MetadataCommand::new();

    let path: PathBuf = path.into();
    cmd.manifest_path(&path);

    let get_opts = GetDependenciesOpt {
        avoid_dev_deps: true,
        avoid_build_deps: true,
        direct_deps_only: false,
        root_only: false,
    };

    let license_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("license_file");

    get_dependencies_from_cargo_lock(cmd, get_opts)?
        .into_iter()
        .map(|dep| {
            let repo = dep.repository;
            let license_file = dep
                .license_file
                .map(|license| -> anyhow::Result<String> {
                    let path = license_dir.join(&dep.name);
                    if std::fs::File::open(&path).is_err() {
                        let paths = repo
                            .as_ref()
                            .unwrap()
                            .trim_start_matches("https://")
                            .split('/')
                            .collect::<Vec<_>>();
                        match paths[0] {
                            "github.com" => {
                                let url = format!(
                                    "https://raw.githubusercontent.com/{}/{}/refs/heads/main/{}",
                                    paths[1], paths[2], license
                                );
                                let body = reqwest::blocking::get(&url)?.text()?;
                                std::fs::create_dir_all(&license_dir)?;
                                std::fs::write(&path, &body)?;
                            }
                            _ => {
                                return Err(anyhow::anyhow!(
                                    "not supported repository: {}",
                                    repo.as_ref().unwrap()
                                ));
                            }
                        }
                    }
                    let file = std::fs::File::open(&path)?;
                    let mut reader = std::io::BufReader::new(file);
                    let mut license = String::new();
                    reader.read_to_string(&mut license)?;
                    Ok(license)
                })
                .transpose()?;
            Ok(Dependency {
                name: dep.name,
                version: dep.version.to_string(),
                license: dep.license,
                license_file: license_file,
                repository: repo,
            })
        })
        .collect()
}
