//! `config.toml`: optional; missing keys take defaults and unknown keys are ignored so
//! an older daemon still starts with a newer file. Both profiles read the same file; the
//! development profile applies its `[dev]` table on top of the base keys. Production
//! never parses `[dev]`, so a mistake there cannot stop the production daemon.

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
    /// Kept untyped and validated only under the development profile.
    dev: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct ApiSection {
    port: Option<u16>,
}

/// Keys the development profile overrides; production ignores this table. Keep
/// [`DEV_KEYS`] and the table in ARCHITECTURE.md in step with it.
#[derive(Debug, Default, Deserialize)]
struct DevSection {
    api_port: Option<u16>,
}

/// The keys `[dev]` supports; others get a warning under the development profile.
const DEV_KEYS: &[&str] = &["api_port"];

/// Reads `[dev]` for the development profile, with a warning line per unknown key.
fn dev_section(
    path: &Path,
    value: Option<toml::Value>,
) -> Result<(DevSection, Vec<String>), DaemonError> {
    let Some(value) = value else {
        return Ok((DevSection::default(), Vec::new()));
    };
    let warnings = value
        .as_table()
        .map(|table| {
            table
                .keys()
                .filter(|key| !DEV_KEYS.contains(&key.as_str()))
                .map(|key| {
                    format!(
                        "warning: {}: [dev] {key} is not a supported key; it is ignored",
                        path.display()
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let section = value.try_into().map_err(|e| DaemonError::Config {
        path: path.to_owned(),
        source: Box::new(e),
    })?;
    Ok((section, warnings))
}

impl Config {
    /// Reads the config file of `paths` for its profile. Warnings go to stderr.
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
        let (config, warnings) =
            Config::from_parts(path, text.as_deref(), paths.data_dir.clone(), paths.profile)?;
        for warning in warnings {
            eprintln!("{warning}");
        }
        Ok(config)
    }

    fn from_parts(
        path: &Path,
        text: Option<&str>,
        data_dir: PathBuf,
        profile: Profile,
    ) -> Result<(Config, Vec<String>), DaemonError> {
        let file: FileConfig = match text {
            Some(text) => toml::from_str(text).map_err(|e| DaemonError::Config {
                path: path.to_owned(),
                source: Box::new(e),
            })?,
            None => FileConfig::default(),
        };
        let base = file.api.port.unwrap_or(DEFAULT_API_PORT);
        let (dev, warnings) = match profile {
            Profile::Production => (DevSection::default(), Vec::new()),
            Profile::Dev => dev_section(path, file.dev)?,
        };
        if dev.api_port == Some(base) {
            return Err(DaemonError::DevPortClash { port: base });
        }
        let api_port = profile
            .api_port(base, dev.api_port)
            .ok_or(DaemonError::DevPort { base })?;
        let config = Config {
            api_port,
            data_dir,
            event_retention: almahrya_engine::DEFAULT_RETENTION,
        };
        Ok((config, warnings))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn parse_with_warnings(
        text: Option<&str>,
        profile: Profile,
    ) -> Result<(Config, Vec<String>), DaemonError> {
        Config::from_parts(Path::new("c.toml"), text, PathBuf::from("/d"), profile)
    }

    fn parse(text: Option<&str>) -> Result<Config, DaemonError> {
        parse_with_warnings(text, Profile::Production).map(|(config, _)| config)
    }

    fn parse_dev(text: Option<&str>) -> Result<Config, DaemonError> {
        parse_with_warnings(text, Profile::Dev).map(|(config, _)| config)
    }

    #[test]
    fn production_starts_with_a_malformed_dev_table() {
        let text = "[api]\nport = 5000\n[dev]\napi_port = \"x\"\n";
        assert_eq!(parse(Some(text)).unwrap().api_port, 5000);
        assert!(matches!(
            parse_dev(Some(text)),
            Err(DaemonError::Config { .. })
        ));
        let not_a_table = "dev = 3\n";
        assert!(parse(Some(not_a_table)).is_ok());
        assert!(matches!(
            parse_dev(Some(not_a_table)),
            Err(DaemonError::Config { .. })
        ));
    }

    #[test]
    fn dev_port_equal_to_the_base_port_is_an_error() {
        let text = "[api]\nport = 5000\n[dev]\napi_port = 5000\n";
        let err = parse_dev(Some(text)).unwrap_err();
        assert!(
            matches!(err, DaemonError::DevPortClash { port: 5000 }),
            "{err:?}"
        );
        assert!(err.to_string().contains("[dev] api_port"), "{err}");
        // The default base port counts too.
        assert!(matches!(
            parse_dev(Some("[dev]\napi_port = 47821\n")),
            Err(DaemonError::DevPortClash { port: 47821 })
        ));
        assert_eq!(parse(Some(text)).unwrap().api_port, 5000);
    }

    #[test]
    fn unknown_dev_keys_are_warned_about_under_dev_only() {
        let text = "[dev]\napi_port = 6000\ndata_dir = \"/x\"\nport = 1\n";
        let (config, warnings) = parse_with_warnings(Some(text), Profile::Dev).unwrap();
        assert_eq!(config.api_port, 6000);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("[dev] data_dir")));
        assert!(warnings.iter().any(|w| w.contains("[dev] port")));
        assert!(warnings.iter().all(|w| !w.contains('\n')));
        let (_, warnings) = parse_with_warnings(Some(text), Profile::Production).unwrap();
        assert!(warnings.is_empty());
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
