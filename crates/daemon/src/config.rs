//! `config.toml`: optional; missing keys take defaults and unknown keys are ignored so
//! an older daemon still starts with a newer file.

use crate::error::DaemonError;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_API_PORT: u16 = 47821;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub api_port: u16,
    pub data_dir: PathBuf,
    pub event_retention: u64,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    #[serde(default)]
    api: ApiSection,
}

#[derive(Debug, Default, Deserialize)]
struct ApiSection {
    port: Option<u16>,
}

impl Config {
    /// Reads the config file and data dir from their default locations or env overrides.
    pub fn load() -> Result<Config, DaemonError> {
        let path = almahrya_local::paths::config_file()?;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(DaemonError::io(
                    format!("could not read {}", path.display()),
                    e,
                ));
            }
        };
        Config::from_parts(&path, text.as_deref(), almahrya_local::paths::data_dir()?)
    }

    fn from_parts(
        path: &Path,
        text: Option<&str>,
        data_dir: PathBuf,
    ) -> Result<Config, DaemonError> {
        let file: FileConfig = match text {
            Some(text) => toml::from_str(text).map_err(|e| DaemonError::Config {
                path: path.to_owned(),
                source: Box::new(e),
            })?,
            None => FileConfig::default(),
        };
        Ok(Config {
            api_port: file.api.port.unwrap_or(DEFAULT_API_PORT),
            data_dir,
            event_retention: almahrya_engine::DEFAULT_RETENTION,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn parse(text: Option<&str>) -> Result<Config, DaemonError> {
        Config::from_parts(Path::new("c.toml"), text, PathBuf::from("/d"))
    }

    #[test]
    fn missing_file_gives_defaults() {
        let config = parse(None).unwrap();
        assert_eq!(
            config,
            Config {
                api_port: 47821,
                data_dir: PathBuf::from("/d"),
                event_retention: 10_000,
            }
        );
    }

    #[test]
    fn reads_api_port_and_ignores_unknown_keys() {
        let config = parse(Some(
            "top = 1\n[api]\nport = 5000\nextra = \"x\"\n[other]\nk = true\n",
        ))
        .unwrap();
        assert_eq!(config.api_port, 5000);
    }

    #[test]
    fn malformed_file_is_an_error() {
        assert!(matches!(
            parse(Some("[api]\nport = \"no\"")),
            Err(DaemonError::Config { .. })
        ));
    }
}
