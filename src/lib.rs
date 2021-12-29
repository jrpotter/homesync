use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use yaml_rust::{Yaml, YamlLoader};

fn read_config(path: &PathBuf) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Err(err) => match err.kind() {
            // Ignore not found since we may try multiple paths.
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(err),
        },
        Ok(contents) => Ok(Some(contents)),
    }
}

fn find_config() -> Result<Vec<Yaml>, Box<dyn Error>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        paths.push(
            [&xdg_config_home, "homesync", "homesync.yml"]
                .iter()
                .collect(),
        );
        paths.push([&xdg_config_home, "homesync.yml"].iter().collect());
    }
    if let Ok(home) = env::var("HOME") {
        paths.push(
            [&home, ".config", "homesync", "homesync.yml"]
                .iter()
                .collect(),
        );
        paths.push([&home, ".homesync.yml"].iter().collect());
    }
    for path in paths {
        if let Ok(Some(contents)) = read_config(&path) {
            return Ok(YamlLoader::load_from_str(&contents)?);
        }
    }
    Err(Box::new(io::Error::new(
        io::ErrorKind::NotFound,
        "Could not find a homesync config.",
    )))
}

pub fn run(config: Option<String>) -> Result<(), Box<dyn Error>> {
    let _loaded = match config {
        Some(path) => {
            let contents = fs::read_to_string(path)?;
            YamlLoader::load_from_str(&contents)?
        }
        None => find_config()?,
    };
    Ok(())
}
