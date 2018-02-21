use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use url::Url;
use url_serde;
use serde_json;

#[derive(Debug, Deserialize)]
struct SbtPortFile {
    #[serde(with = "url_serde")]
    uri: Url,
}

pub fn lookup_from(start: &Path) -> Result<Option<PathBuf>, io::Error> {
    match find_port_file(start) {
        None => Ok(None),
        Some(port_file) => {
            debug!("Found port file {:?}", port_file);
            let file = File::open(port_file)?;
            let s: SbtPortFile = serde_json::from_reader(file)?;
            match s.uri.scheme() {
                "local" => {
                    let sock = s.uri.path();

                    Ok(Some(PathBuf::from(sock)))
                }
                sch => Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Unknown protocol: {}", sch),
                )),
            }
        }
    }
}

fn find_port_file(start: &Path) -> Option<PathBuf> {
    let mut current_path = if start.is_file() {
        start.parent()
    } else {
        Some(start)
    };

    while let Some(path) = current_path {
        let mut port_path = PathBuf::new();
        port_path.push(path);
        port_path.push("project");
        port_path.push("target");
        port_path.push("active.json");

        trace!("Testing for port file {:?}", port_path);

        if port_path.exists() && !port_path.is_dir() {
            info!("Found a (probably) running sbt server for project {:?}", path);
            return Some(port_path);
        } else {
            current_path = path.parent();
        }
    }

    debug!("No port file found (startup point: {:?})", start);
    None
}
