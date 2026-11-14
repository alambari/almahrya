//! `config.toml`: optional; missing keys take defaults and unknown keys are ignored so
//! an older daemon still starts with a newer file. Both profiles read the same file; the
//! development profile applies its `[dev]` table on top of the base keys.

use crate::error::DaemonError;
use almahrya_local::paths::Paths;
use almahrya_local::profile::Profile;
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
    #[serde(default)]
    dev: DevSection,
}

#[derive(Debug, Default, Deserialize)]
struct ApiSection {
    port: Option<u16>,
}

/// Keys the development profile overrides; production ignores this table.
#[derive(Debug, Default, Deserialize)]
struct DevSection {
    api_port: Option<u16>,
}

impl Config {
    /// Reads the config file of `paths` for its profile.
    pub fn load(paths: &Paths) -> Result<Config, DaemonError> {
        let path = &paths.config_file;
        let text = match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(DaemonError::io(
                    format!("could not read {}", path.display()),
                    e,
                ));
            }
        };
        Config::from_parts(path, text.as_deref(), paths.data_dir.clone(), paths.profile)
    }

    fn from_parts(
        path: &Path,
        text: Option<&str>,
        data_dir: PathBuf,
        profile: Profile,
    ) -> Result<Config, DaemonError> {
        let file: FileConfig = match text {
            Some(text) => toml::from_str(text).map_err(|e| DaemonError::Config {
                path: path.to_owned(),
                source: Box::new(e),
            })?,
            None => FileConfig::default(),
        };
        let base = file.api.port.unwrap_or(DEFAULT_API_PORT);
        let api_port = profile
            .api_port(base, file.dev.api_port)
            .ok_or(DaemonError::DevPort { base })?;
        Ok(Config {
            api_port,
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
        Config::from_parts(
            Path::new("c.toml"),
            text,
            PathBuf::from("/d"),
            Profile::Production,
        )
    }

    fn parse_dev(text: Option<&str>) -> Result<Config, DaemonError> {
        Config::from_parts(Path::new("c.toml"), text, PathBuf::from("/d"), Profile::Dev)
    }

    #[test]
    fn dev_port_is_base_plus_one_unless_the_dev_table_sets_it() {
        assert_eq!(parse_dev(None).unwrap().api_port, 47822);
        assert_eq!(
            parse_dev(Some("[api]\nport = 5000\n")).unwrap().api_port,
            5001
        );
        let both = "[api]\nport = 5000\n[dev]\napi_port = 6000\n";
        assert_eq!(parse_dev(Some(both)).unwrap().api_port, 6000);
        // Production never reads the dev table.
        assert_eq!(parse(Some(both)).unwrap().api_port, 5000);
    }

    #[test]
    fn dev_port_past_the_last_port_is_an_error() {
        assert!(matches!(
            parse_dev(Some("[api]\nport = 65535\n")),
            Err(DaemonError::DevPort { base: 65535 })
        ));
        assert_eq!(
            parse(Some("[api]\nport = 65535\n")).unwrap().api_port,
            65535
        );
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
